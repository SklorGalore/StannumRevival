// Power flow results for symbols linked to the open case.
import { useWorkspace } from "../../store/workspaceStore";
import { useDiagramStore } from "../../store/diagramStore";
import type { BranchRow, BusRow, GeneratorRow, SolveReport } from "../analysis/types";

export interface Results {
  ac: boolean;
  buses: Map<number, BusRow>;
  generators: Map<number, GeneratorRow>;
  branches: Map<number, BranchRow>;
}

const indexed = new WeakMap<SolveReport, Results>();

function index(report: SolveReport): Results {
  let r = indexed.get(report);
  if (!r) {
    r = {
      ac: report.model === "ac",
      buses: new Map(report.buses.map((b) => [b.id, b])),
      generators: new Map(report.generators.map((g) => [g.index, g])),
      branches: new Map(report.branches.map((b) => [b.index, b])),
    };
    indexed.set(report, r);
  }
  return r;
}

/** Results to draw, or null when hidden, unsolved, or for a different case. */
export function useResults(): Results | null {
  const report = useWorkspace((s) => s.solveReport);
  const openName = useWorkspace((s) => s.caseSummary?.name);
  const caseName = useDiagramStore((s) => s.metadata.caseName);
  const show = useDiagramStore((s) => s.showResults);
  if (!show || !report || !caseName || caseName !== openName) return null;
  return index(report);
}

/** Flow along a drawn branch, read at the end the drawing starts from. */
export function branchFlow(row: BranchRow, reversed?: boolean) {
  return reversed ? { p: row.pt, q: row.qt } : { p: row.pf, q: row.qf };
}

export const mw = (v: number) => `${Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1)} MW`;
export const mvar = (v: number) => `${Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1)} Mvar`;

/** Loading bands used on branches and transformers. */
export function loadingColor(pct: number | null | undefined): string | null {
  if (pct == null) return null;
  if (pct >= 100) return "#dc2626";
  if (pct >= 80) return "#d97706";
  return null;
}

export const RESULT_TEXT = "#0f766e";
