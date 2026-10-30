import { describe, expect, it } from "vitest";
import { paletteElements, searchElements } from "./search";
import type { CaseNetwork } from "./analysis/types";

const bus = (id: number, name: string) => ({ id, name, base_kv: 138, area: 1, zone: 1, owner: 1, kind: "PQ" as const, pd: 0, qd: 0, gs: 0, bs: 0, star: false });
const branch = (index: number, from: number, to: number, circuit = "1") => ({
  index, element: `line:${Math.min(from, to)}:${Math.max(from, to)}:${circuit}`, circuit, winding: 0, from, to,
  online: true, transformer: false, rate_a: 100, tap: 1, shift: 0,
});
const net: CaseNetwork = {
  base_mva: 100,
  buses: [bus(1, "North_138"), bus(14, "Central_138"), bus(149, "Lakeside"), { ...bus(900, ""), star: true }],
  generators: [{ index: 0, element: "generator:149:1", bus: 149, online: true, pmax: 200 }],
  branches: [branch(0, 1, 14), branch(1, 14, 149), branch(2, 14, 149, "2")],
  dc_lines: [],
};
const items = paletteElements(net);

describe("palette search", () => {
  it("lists buses, branches and generators but not star points", () => {
    expect(items.map((i) => i.element)).toEqual([
      "bus:1", "bus:14", "bus:149", "line:1:14:1", "line:14:149:1", "line:14:149:2", "generator:149:1",
    ]);
  });

  it("puts the bus with that exact number first, then what connects to it", () => {
    const hits = searchElements(items, "149").map((i) => i.element);
    expect(hits[0]).toBe("bus:149");
    expect(hits.slice(1)).toEqual(["line:14:149:1", "line:14:149:2", "generator:149:1"]);
  });

  it("matches names and needs every word", () => {
    expect(searchElements(items, "lakeside")[0].element).toBe("bus:149");
    expect(searchElements(items, "central gen")).toEqual([]);
    expect(searchElements(items, "line 14 ckt 2").map((i) => i.element)).toEqual(["line:14:149:2"]);
  });

  it("lists a three-winding transformer once, by its windings' buses", () => {
    const winding = (index: number, from: number, w: number) => ({
      ...branch(index, from, 900), element: "transformer:1:14:149:T", circuit: "T", winding: w, transformer: true,
    });
    const three = paletteElements({ ...net, branches: [winding(3, 14, 1), winding(4, 1, 2), winding(5, 149, 3)] });
    const t = three.filter((i) => i.element.startsWith("transformer"));
    expect(t).toHaveLength(1);
    expect(t[0].title).toBe("Transformer 14–1–149 circuit T");
    expect(t[0].detail).not.toContain("900");
    expect(searchElements(three, "149").map((i) => i.element)).toContain("transformer:1:14:149:T");
  });

  it("returns nothing for an empty query", () => {
    expect(searchElements(items, "  ")).toEqual([]);
  });
});
