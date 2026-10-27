// Power flow results for symbols linked to the open case.
import { useShownReport, useWorkspace } from "../../store/workspaceStore";
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

/**
 * Results to draw (the result every view shows), or null when hidden,
 * unsolved, or for a different case.
 */
export function useResults(): Results | null {
  const report = useShownReport();
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

const drawnMax = new WeakMap<object, WeakMap<Results, number>>();

/**
 * Largest active power flow on a branch in this drawing, in MW, to weigh
 * unrated branches against: the case's largest flow would make every wire in
 * a local drawing look light.
 */
export function useDrawnMaxFlow(results: Results | null): number {
  const edges = useDiagramStore((s) => s.edges);
  if (!results) return 0;
  let byResults = drawnMax.get(edges);
  if (!byResults) drawnMax.set(edges, (byResults = new WeakMap()));
  let max = byResults.get(results);
  if (max === undefined) {
    max = 0;
    for (const e of edges) {
      const ref = (e.data as { caseRef?: { kind: string; id: number } } | undefined)?.caseRef;
      const row = ref?.kind === "branch" ? results.branches.get(ref.id) : undefined;
      if (row) max = Math.max(max, Math.abs(row.pf), Math.abs(row.pt));
    }
    byResults.set(results, max);
  }
  return max;
}
