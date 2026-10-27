import { describe, expect, it } from "vitest";
import { elementResults, loadingTone } from "./inspect";
import type { BranchRow, BusRow, GeneratorRow, SolveReport } from "./analysis/types";

const bus = (id: number, vm: number, extra: Partial<BusRow> = {}): BusRow => ({
  id, name: "", kind: "PQ", base_kv: 230, vm, va: -3, vmin: 0.95, vmax: 1.05, pd: 50, qd: 10, area: 1,
  violation: vm > 1.05 ? vm - 1.05 : vm < 0.95 ? 0.95 - vm : 0, load_parts: null, ...extra,
});
const branch = (extra: Partial<BranchRow>): BranchRow => ({
  index: 0, element: "line:1:2:1", circuit: "1", winding: 0, from: 1, to: 2, online: true, is_transformer: false,
  pf: 120, qf: 20, pt: -118, qt: -15, loss_mw: 2, rate_a: 100, loading_pct: 121.7, ...extra,
});
const gen = (extra: Partial<GeneratorRow>): GeneratorRow => ({
  index: 0, element: "generator:1:1", machine_id: "1", bus: 1, bus_name: "", online: true, pg: 80, qg: 40,
  qmin: -20, qmax: 30, vg: 1.02, pmax: 100, q_violation: 10, ...extra,
});
const report = (r: Partial<SolveReport>): SolveReport => ({ buses: [], generators: [], branches: [], ...r }) as SolveReport;

describe("elementResults", () => {
  it("flags an overloaded line", () => {
    const r = elementResults(report({ branches: [branch({})] }), "line:1:2:1")!;
    expect(r.headline).toBe("121.7% of rate A");
    expect(r.tone).toBe("bad");
    expect(r.loading).toBeCloseTo(121.7);
    expect(r.groups[0].rows.find((x) => x.label === "Loading")).toMatchObject({ value: "121.7", tone: "bad", unit: "% of 100 MVA" });
  });

  it("shows each winding of a three-winding transformer and its worst loading", () => {
    const id = "transformer:1:2:3:T1";
    const rows = [1, 2, 3].map((w) => branch({ element: id, winding: w, loading_pct: 30 * w, is_transformer: true }));
    const r = elementResults(report({ branches: rows }), id)!;
    expect(r.groups.map((g) => g.title)).toEqual(["Winding 1", "Winding 2", "Winding 3"]);
    expect(r.loading).toBe(90);
    expect(r.tone).toBe("ok");
  });

  it("splits a bus's load by how it varies with voltage", () => {
    const parts = { p: [20, 18, 12] as [number, number, number], q: [5, 3, 2] as [number, number, number] };
    const r = elementResults(report({ buses: [bus(7, 0.98, { load_parts: parts })] }), "load:7:1")!;
    expect(r.groups.map((g) => g.title)).toEqual(["Bus 7", "Load by type"]);
    expect(r.groups[1].rows.find((x) => x.label === "Constant current" && x.unit === "MW")).toMatchObject({ value: "18.0" });
    expect(elementResults(report({ buses: [bus(7, 0.98)] }), "bus:7")!.groups).toHaveLength(1);
  });

  it("says when a branch is out of service", () => {
    const r = elementResults(report({ branches: [branch({ online: false })] }), "line:1:2:1")!;
    expect(r).toMatchObject({ offline: true, headline: "Out of service", groups: [] });
  });

  it("compares a bus voltage with its limits", () => {
    const r = elementResults(report({ buses: [bus(7, 1.062)] }), "bus:7")!;
    expect(r.headline).toBe("1.062 p.u., above 1.05");
    expect(r.tone).toBe("bad");
    expect(r.groups[0].rows.find((x) => x.unit === "kV")?.value).toBe((1.062 * 230).toFixed(2));
  });

  it("shows a load through its bus", () => {
    const r = elementResults(report({ buses: [bus(7, 1)] }), "load:7:1")!;
    expect(r.groups[0].title).toBe("Bus 7");
    expect(r.tone).toBe("ok");
  });

  it("flags a generator outside its reactive range", () => {
    const r = elementResults(report({ generators: [gen({})] }), "generator:1:1")!;
    expect(r.tone).toBe("bad");
    expect(r.headline).toBe("40.0 Mvar, outside -20.0 to 30.0");
  });

  it("returns null for elements the solve doesn't list", () => {
    expect(elementResults(report({}), "line:8:9:1")).toBeNull();
    expect(elementResults(report({}), "nonsense")).toBeNull();
  });

  it("grades loading", () => {
    expect([loadingTone(null), loadingTone(50), loadingTone(95), loadingTone(101)]).toEqual(["ok", "ok", "warn", "bad"]);
  });
});
