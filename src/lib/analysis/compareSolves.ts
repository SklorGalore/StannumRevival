// What changed between two solves of the same case (a study's "before" and
// "after"): totals, the branches and buses that moved most, and the limit
// violations that appeared or cleared.
import type { BranchRow, BusRow, SolveReport } from "./types";

export interface BranchChange {
  element: string;
  index: number;
  from: number;
  to: number;
  circuit: string;
  /** Active power at the from end, MW, before and after. */
  pA: number;
  pB: number;
  dp: number;
  loadingA: number | null;
  loadingB: number | null;
  onlineA: boolean;
  onlineB: boolean;
}

export interface BusChange {
  id: number;
  name: string;
  vmA: number;
  vmB: number;
  dvm: number;
  vaA: number;
  vaB: number;
}

export interface Violation {
  element: string;
  label: string;
  /** How far past the limit, in the check's units. */
  amount: string;
}

export interface SolveComparison {
  totals: { label: string; a: number; b: number; unit: string }[];
  branches: BranchChange[];
  buses: BusChange[];
  appeared: Violation[];
  cleared: Violation[];
}

const overloaded = (b: BranchRow) => b.online && (b.loading_pct ?? 0) > 100;
const outside = (b: BusRow) => b.violation > 0;

function violations(r: SolveReport): Map<string, Violation> {
  const out = new Map<string, Violation>();
  for (const b of r.buses)
    if (outside(b)) out.set(`bus:${b.id}`, { element: `bus:${b.id}`, label: `Bus ${b.id} voltage`, amount: `${b.vm.toFixed(4)} p.u.` });
  for (const b of r.branches)
    if (overloaded(b))
      out.set(b.element + (b.winding ? `#${b.winding}` : ""), {
        element: b.element,
        label: `${b.from} → ${b.to} (${b.circuit}) loading`,
        amount: `${(b.loading_pct ?? 0).toFixed(1)}%`,
      });
  for (const g of r.generators)
    if (g.online && g.q_violation > 0)
      out.set(`q:${g.element}`, { element: g.element, label: `Generator ${g.machine_id} at bus ${g.bus} Q`, amount: `${g.q_violation.toFixed(1)} Mvar past` });
  return out;
}

/** Compare solve B against solve A. Rows are matched by branch row and bus number. */
export function compareSolves(a: SolveReport, b: SolveReport): SolveComparison {
  const branchesB = new Map(b.branches.map((r) => [r.index, r]));
  const branches: BranchChange[] = [];
  for (const ra of a.branches) {
    const rb = branchesB.get(ra.index);
    if (!rb) continue;
    branches.push({
      element: ra.element,
      index: ra.index,
      from: ra.from,
      to: ra.to,
      circuit: ra.winding ? `${ra.circuit} · W${ra.winding}` : ra.circuit,
      pA: ra.pf,
      pB: rb.pf,
      dp: rb.pf - ra.pf,
      loadingA: ra.loading_pct,
      loadingB: rb.loading_pct,
      onlineA: ra.online,
      onlineB: rb.online,
    });
  }
  branches.sort((x, y) => Math.abs(y.dp) - Math.abs(x.dp) || x.index - y.index);

  const busesB = new Map(b.buses.map((r) => [r.id, r]));
  const buses: BusChange[] = [];
  for (const ra of a.buses) {
    const rb = busesB.get(ra.id);
    if (!rb) continue;
    buses.push({ id: ra.id, name: ra.name.trim(), vmA: ra.vm, vmB: rb.vm, dvm: rb.vm - ra.vm, vaA: ra.va, vaB: rb.va });
  }
  buses.sort((x, y) => Math.abs(y.dvm) - Math.abs(x.dvm) || x.id - y.id);

  const va = violations(a);
  const vb = violations(b);
  const count = (r: SolveReport) => r.limits.voltage_violations + r.limits.reactive_violations + r.limits.branch_overloads;
  return {
    totals: [
      { label: "Generation", a: a.total_generation_mw, b: b.total_generation_mw, unit: "MW" },
      { label: "Load", a: a.total_load_mw, b: b.total_load_mw, unit: "MW" },
      { label: "Losses", a: a.total_losses_mw, b: b.total_losses_mw, unit: "MW" },
      { label: "Limit violations", a: count(a), b: count(b), unit: "" },
    ],
    branches,
    buses,
    appeared: [...vb].filter(([k]) => !va.has(k)).map(([, v]) => v),
    cleared: [...va].filter(([k]) => !vb.has(k)).map(([, v]) => v),
  };
}
