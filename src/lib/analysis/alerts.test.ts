import { describe, expect, it } from "vitest";
import { alertCounts, alertsFor } from "./alerts";
import type { BranchRow, BusRow, GeneratorRow, SolveReport } from "./types";

const bus = (id: number, vm: number, violation = 0, kind: BusRow["kind"] = "PQ"): BusRow => ({
  id, name: `B${id}`, kind, base_kv: 138, vm, va: 0, vmin: 0.95, vmax: 1.05, pd: 0, qd: 0, area: 1, violation, load_parts: null,
});
const branch = (index: number, loading: number | null, online = true): BranchRow => ({
  index, element: `line:${index}:${index + 1}:1`, circuit: "1", winding: 0, from: index, to: index + 1, online,
  is_transformer: false, tap: 1, shift_deg: 0, pf: 0, qf: 0, pt: 0, qt: 0, loss_mw: 0, rate_a: 100, loading_pct: loading,
});
const gen = (bus: number, qg: number, q_violation: number): GeneratorRow => ({
  index: 0, element: `generator:${bus}:1`, bus, bus_name: "", machine_id: "1", online: true, pg: 0, pmax: 100, qg, qmin: -50, qmax: 50, q_violation, vg: 1,
});
const report = (buses: BusRow[], branches: BranchRow[], generators: GeneratorRow[] = []): SolveReport =>
  ({ buses, branches, generators }) as unknown as SolveReport;

describe("alertsFor", () => {
  it("splits critical (past a limit) from marginal (within the band), critical first", () => {
    const a = alertsFor(
      report(
        [bus(1, 1.0), bus(2, 0.93, 0.02), bus(3, 0.958), bus(4, 1.045), bus(5, 0.5, 0, "Isolated")],
        [branch(1, 50), branch(2, 104.2), branch(3, 96), branch(4, 99, false), branch(5, null)],
        [gen(7, 60, 10)]
      )
    );
    expect(a.map((x) => [x.level, x.element])).toEqual([
      ["critical", "line:2:3:1"],
      ["critical", "bus:2"],
      ["critical", "generator:7:1"],
      ["marginal", "line:3:4:1"],
      ["marginal", "bus:4"],
      ["marginal", "bus:3"],
    ]);
    expect(a[0]).toMatchObject({ condition: "Overload", value: "104.2 %", names: "B2 – B3" });
    expect(a[1]).toMatchObject({ condition: "Under voltage", value: "0.930 pu", limit: "0.950 pu" });
    expect(a[2]).toMatchObject({ condition: "Above Q max", limit: "50.0 Mvar" });
    expect(alertCounts(a)).toEqual({ critical: 3, marginal: 3 });
  });

  it("follows the thresholds", () => {
    const r = report([bus(1, 0.97)], [branch(1, 90)]);
    expect(alertsFor(r)).toEqual([]);
    expect(alertsFor(r, { loadingPct: 85, voltageBandPu: 0.03 }).map((x) => x.element)).toEqual(["line:1:2:1", "bus:1"]);
  });

  it("counts exactly 100 % as marginal, as the overview counts overloads past 100 %", () => {
    expect(alertsFor(report([], [branch(1, 100)]))[0].level).toBe("marginal");
  });
});
