import { describe, expect, it } from "vitest";
import { buildMap } from "./model";
import { arrowSize, mapOverlay } from "./overlay";
import type { BranchRow, BusCoordinates, BusRow, CaseNetwork, NetworkBus, SolveReport } from "../analysis/types";
import type { LayerMarks } from "../diagram/layers";

const bus = (id: number, base_kv: number): NetworkBus => ({ id, name: `B${id}`, base_kv, area: 1, zone: 1, owner: 1, kind: "PQ", pd: 0, qd: 0, gs: 0, bs: 0, star: false });
const element = (from: number, to: number) => `line:${from}:${to}:1`;
const branch = (from: number, to: number) => ({
  index: 0,
  element: element(from, to),
  circuit: "1",
  winding: 0,
  from,
  to,
  online: true,
  transformer: false,
  rate_a: 100,
  tap: 1,
  shift: 0,
});

// 1 and 2 share a substation; 3 and 4 stand alone.
const net: CaseNetwork = {
  base_mva: 100,
  buses: [bus(1, 345), bus(2, 138), bus(3, 138), bus(4, 138)],
  generators: [],
  branches: [branch(1, 2), branch(2, 3), branch(3, 4)],
  dc_lines: [],
};
const coords: BusCoordinates = {
  bus: [1, 2, 3, 4],
  latitude: [30, 30, 31, 32],
  longitude: [-97, -97, -96, -95],
  source: ["file", "file", "file", "file"],
  substation: ["North", "North", null, null],
  summary: {} as BusCoordinates["summary"],
};
const model = buildMap(net, coords);

const row = (from: number, to: number, pf: number, online = true, loading_pct: number | null = 50): BranchRow => ({
  index: 0,
  element: element(from, to),
  circuit: "1",
  winding: 0,
  from,
  to,
  online,
  is_transformer: false,
  tap: 1,
  shift_deg: 0,
  pf,
  qf: 0,
  pt: -pf,
  qt: 0,
  loss_mw: 0,
  rate_a: 100,
  loading_pct,
});
const busRow = (id: number, vm: number): BusRow => ({ id, name: "", kind: "PQ", base_kv: 138, vm, va: 0, vmin: 0.95, vmax: 1.05, pd: 0, qd: 0, area: 1, violation: 0, load_parts: null });
const report = {
  buses: [busRow(1, 1.01), busRow(2, 0.99), busRow(3, 0.97), busRow(4, 1.0)],
  branches: [row(1, 2, 10), row(2, 3, -80), row(3, 4, 0, false)],
  dc_lines: [],
} as unknown as SolveReport;

const empty = (): LayerMarks => ({ buses: new Map(), elements: new Map(), nodes: new Map() });

describe("mapOverlay", () => {
  it("gives a site the marks of its buses, highest voltage first", () => {
    const marks = empty();
    marks.buses.set(2, { color: "#222222", note: "second" });
    marks.buses.set(1, { halo: "#111111", note: "first" });
    const o = mapOverlay(model, marks, null, false);
    const north = o.sites[model.busSite.get(1)!];
    expect(north.color).toBe("#222222");
    expect(north.halo).toBe("#111111");
    expect(north.notes).toEqual(["Bus 1: first", "Bus 2: second"]);
  });

  it("fades a site only when every bus in it is faded", () => {
    const marks = empty();
    marks.buses.set(1, { dim: true });
    expect(mapOverlay(model, marks, null, false).sites[model.busSite.get(1)!].dim).toBe(false);
    marks.buses.set(2, { dim: true });
    expect(mapOverlay(model, marks, null, false).sites[model.busSite.get(1)!].dim).toBe(true);
  });

  it("marks lines by element id", () => {
    const marks = empty();
    marks.elements.set(element(2, 3), { color: "#dc2626", bold: true, note: "overloaded" });
    const o = mapOverlay(model, marks, null, false);
    const line = model.lines.findIndex((l) => l.element === element(2, 3));
    expect(o.lines[line]).toMatchObject({ color: "#dc2626", bold: true, notes: ["overloaded"] });
  });

  it("marks a site for a marked branch inside it", () => {
    const marks = empty();
    marks.elements.set(element(1, 2), { halo: "#dc2626", dim: true, note: "taken out" });
    const north = mapOverlay(model, marks, null, false).sites[model.busSite.get(1)!];
    expect(north.halo).toBe("#dc2626");
    expect(north.notes).toEqual(["Line 1–2 circuit 1: taken out"]);
  });

  it("takes each line's status from the shown result", () => {
    const o = mapOverlay(model, empty(), report, false);
    const out = model.lines.findIndex((l) => l.element === element(3, 4));
    expect(o.lines[out].online).toBe(false);
    // Flows only when asked for.
    expect(o.lines.every((l) => l.mw === null)).toBe(true);
  });

  it("gives flows with their direction from the line's from bus", () => {
    const o = mapOverlay(model, empty(), report, true);
    const i = model.lines.findIndex((l) => l.element === element(2, 3));
    expect(o.lines[i].mw).toBe(-80);
    expect(o.lines[i].notes[0]).toBe("80.0 MW from bus 3 to bus 2 · 50% of rate A");
    expect(o.maxMw).toBe(80);
    const site3 = o.sites[model.busSite.get(3)!];
    expect(site3.notes).toEqual(["0.970 p.u."]);
  });
});

describe("arrowSize", () => {
  it("grows with the share of the heaviest flow", () => {
    expect(arrowSize(0, 100)).toBeCloseTo(0.8);
    expect(arrowSize(100, 100)).toBeCloseTo(1.6);
    expect(arrowSize(25, 100)).toBeCloseTo(1.2);
  });
});
