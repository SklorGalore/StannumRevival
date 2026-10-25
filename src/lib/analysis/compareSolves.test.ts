import { describe, expect, it } from "vitest";
import { compareSolves } from "./compareSolves";
import type { BranchRow, BusRow, SolveReport } from "./types";

const bus = (id: number, vm: number, violation = 0): BusRow => ({
  id, name: "", kind: "PQ", base_kv: 138, vm, va: 0, vmin: 0.95, vmax: 1.05, pd: 0, qd: 0, area: 1, violation,
});
const branch = (index: number, pf: number, loading: number | null, online = true): BranchRow => ({
  index, element: `line:${index}:${index + 1}:1`, circuit: "1", winding: 0, from: index, to: index + 1, online,
  is_transformer: false, pf, qf: 0, pt: -pf, qt: 0, loss_mw: 0, rate_a: 100, loading_pct: loading,
});
const report = (buses: BusRow[], branches: BranchRow[], losses: number): SolveReport =>
  ({
    status: "Converged", converged: true, model: "ac", iterations: 3, q_limit_passes: 0, residual_pu: 0, residual_mva: 0, elapsed_ms: 1,
    total_generation_mw: 100 + losses, total_load_mw: 100, total_losses_mw: losses,
    limits: {
      voltage_violations: buses.filter((b) => b.violation > 0).length, reactive_violations: 0,
      branch_overloads: branches.filter((b) => (b.loading_pct ?? 0) > 100).length,
    },
    buses, generators: [], branches,
  }) as unknown as SolveReport;

describe("compareSolves", () => {
  const a = report([bus(1, 1.0), bus(2, 0.99)], [branch(1, 50, 50), branch(2, 30, 30)], 2);
  const b = report([bus(1, 1.0), bus(2, 0.94, 0.01)], [branch(1, 0, 0, false), branch(2, 110, 110)], 3);
  const c = compareSolves(a, b);

  it("ranks branches by how much their flow moved", () => {
    expect(c.branches.map((x) => [x.index, x.dp])).toEqual([[2, 80], [1, -50]]);
    expect(c.branches[1]).toMatchObject({ onlineA: true, onlineB: false });
  });

  it("ranks buses by voltage change", () => {
    expect(c.buses[0]).toMatchObject({ id: 2 });
    expect(c.buses[0].dvm).toBeCloseTo(-0.05);
  });

  it("lists violations that appeared and cleared", () => {
    expect(c.appeared.map((v) => v.label)).toEqual(["Bus 2 voltage", "2 → 3 (1) loading"]);
    expect(c.cleared).toEqual([]);
    expect(compareSolves(b, a).cleared).toHaveLength(2);
  });

  it("sums the totals", () => {
    expect(c.totals.find((t) => t.label === "Losses")).toMatchObject({ a: 2, b: 3 });
    expect(c.totals.find((t) => t.label === "Limit violations")).toMatchObject({ a: 0, b: 2 });
  });
});
