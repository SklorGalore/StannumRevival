import { describe, expect, it } from "vitest";
import { buildMap, neighbourhood, siteElement, siteLabel } from "./model";
import type { BusCoordinates, CaseNetwork, NetworkBranch, NetworkBus } from "../analysis/types";

const bus = (id: number, base_kv: number, star = false): NetworkBus => ({
  id,
  name: `B${id}`,
  base_kv,
  area: 1,
  kind: "PQ",
  pd: 0,
  qd: 0,
  gs: 0,
  bs: 0,
  star,
});
const branch = (from: number, to: number, transformer = false): NetworkBranch => ({
  index: 0,
  element: `${transformer ? "transformer" : "line"}:${Math.min(from, to)}:${Math.max(from, to)}:1`,
  circuit: "1",
  winding: 0,
  from,
  to,
  online: true,
  transformer,
  rate_a: 0,
  tap: 1,
  shift: 0,
});

// 1 and 2 at one substation (a transformer), 3 elsewhere, 4 estimated onto 3's
// point, 5 estimated somewhere new, 6 with no place.
const net: CaseNetwork = {
  base_mva: 100,
  buses: [bus(1, 345), bus(2, 138), bus(3, 138), bus(4, 69), bus(5, 69), bus(6, 13.8)],
  generators: [],
  branches: [branch(1, 2, true), branch(2, 3), branch(3, 4, true), branch(4, 5)],
  dc_lines: [],
};
const coords: BusCoordinates = {
  bus: [1, 2, 3, 4, 5, 6],
  latitude: [30, 30.01, 31, 31, 31.5, null],
  longitude: [-97, -97.01, -96, -96, -95.5, null],
  source: ["file", "file", "file", "estimated", "estimated", "none"],
  substation: ["North", "North", null, null, null, null],
  summary: {} as BusCoordinates["summary"],
};

describe("buildMap", () => {
  const map = buildMap(net, coords);

  it("gathers buses into sites by substation name, then by point", () => {
    expect(map.sites.map((s) => s.buses)).toEqual([[1, 2], [3, 4], [5]]);
    expect(map.sites[0].kv).toBe(345);
    expect(map.sites[0].substation).toBe("North");
    // The first bus of a named substation sets its point.
    expect([map.sites[0].lat, map.sites[0].lon]).toEqual([30, -97]);
  });

  it("keeps two substations at one point apart", () => {
    const twin: BusCoordinates = { ...coords, substation: ["North", "South", null, null, null, null], latitude: [30, 30, 31, 31, 31.5, null], longitude: [-97, -97, -96, -96, -95.5, null] };
    expect(buildMap(net, twin).sites.map((s) => s.buses)).toEqual([[1], [2], [3, 4], [5]]);
  });

  it("marks a site estimated only when nothing in it came from the file", () => {
    expect(map.sites.map((s) => s.estimated)).toEqual([false, false, true]);
  });

  it("draws branches between sites, not inside one", () => {
    expect(map.lines.map((l) => [l.from, l.to, l.a, l.b, l.kv])).toEqual([
      [2, 3, 0, 1, 138],
      [4, 5, 1, 2, 69],
    ]);
  });

  it("keeps branches inside a site, for layers that mark them", () => {
    expect(map.inner).toEqual([
      { element: "transformer:1:2:1", winding: 0, site: 0 },
      { element: "transformer:3:4:1", winding: 0, site: 1 },
    ]);
  });

  it("counts unplaced buses and spans the placed ones", () => {
    expect(map.unplaced).toBe(1);
    expect(map.bounds).toEqual([-97, 30, -95.5, 31.5]);
  });

  it("puts a three-winding star point between its buses", () => {
    const withStar: CaseNetwork = {
      ...net,
      buses: [...net.buses, bus(900, 1, true)],
      branches: [...net.branches, branch(1, 900, true), branch(3, 900, true), branch(5, 900, true)],
    };
    const m = buildMap(withStar, coords);
    const site = m.sites[m.busSite.get(900)!];
    expect(site.lat).toBeCloseTo((30 + 31 + 31.5) / 3);
    expect(site.estimated).toBe(true);
  });

  it("names a site by substation, else by its top bus", () => {
    expect(siteLabel(map.sites[0], net)).toBe("North");
    expect(siteLabel(map.sites[1], net)).toBe("B3 (bus 3)");
    expect(siteElement(map.sites[1])).toBe("bus:3");
  });
});

describe("neighbourhood", () => {
  it("walks the given number of hops", () => {
    expect([...neighbourhood(net, 3, 1)].sort()).toEqual([2, 3, 4]);
    expect([...neighbourhood(net, 3, 2)].sort()).toEqual([1, 2, 3, 4, 5]);
  });

  it("crosses a star point without spending a hop", () => {
    const withStar: CaseNetwork = {
      ...net,
      buses: [...net.buses, bus(900, 1, true)],
      branches: [branch(1, 900, true), branch(6, 900, true)],
    };
    expect([...neighbourhood(withStar, 1, 1)].sort((a, b) => a - b)).toEqual([1, 6, 900]);
  });
});
