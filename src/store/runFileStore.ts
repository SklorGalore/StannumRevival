// Run files (.stnrun): Run plays one back. The format and the runner live
// in stannum-core (run_file.rs); stannum-cli runs the same files without the
// app. Run files are made from the history (historyStore.ts) or with Save as
// run file (studyFileStore.ts).
import { create } from "zustand";
import { api, pickFile, RUN_FILE_FILTERS, toStannumError } from "../lib/analysis/api";
import type { RunReport, Step } from "../lib/analysis/commands.gen";
import { fileName } from "../lib/study/recent";
import { useStudy } from "./studyStore";
import { caseChangeOk, useWorkspace } from "./workspaceStore";

interface RunFileState {
  /** The run file running now, if any. */
  running: string | null;
  /** How the last run went, until dismissed. */
  report: RunReport | null;
  error: string | null;
  /** Run a run file (asking for one when no path is given), then show what it left. */
  run: (path?: string, opts?: { confirmed?: boolean }) => Promise<RunReport | null>;
  cancel: () => void;
  dismiss: () => void;
}

/**
 * The case on screen as run file steps: open it, attach its side files, set
 * its study edits.
 */
export function startingPoint(): Step[] {
  const summary = useWorkspace.getState().caseSummary;
  if (!summary) return [];
  const steps: Step[] = [{ command: "open_case", args: { path: summary.path } }];
  if (summary.dc_grids) steps.push({ command: "load_dc_grids", args: { path: summary.dc_grids.path } });
  if (summary.coordinates) steps.push({ command: "load_coordinates", args: { path: summary.coordinates.path } });
  const edits = useStudy.getState().entries.map((e) => e.edit);
  if (edits.length > 0) steps.push({ command: "set_study_edits", args: { edits } });
  return steps;
}

export const useRunFile = create<RunFileState>((set, get) => ({
  running: null,
  report: null,
  error: null,

  run: async (path, opts) => {
    if (get().running) return null;
    const w = useWorkspace.getState();
    // A run file usually opens its own case: ask about unsaved work first.
    if (!opts?.confirmed && !caseChangeOk(w.caseSummary)) return null;
    const chosen = path ?? (await pickFile(RUN_FILE_FILTERS));
    if (!chosen) return null;
    set({ running: chosen, report: null, error: null });
    let report: RunReport | null = null;
    try {
      report = await api.runFile(chosen);
    } catch (e) {
      // The file couldn't be read, or a line is wrong: nothing ran.
      set({ running: null, error: toStannumError(e).message });
      return null;
    }
    set({ running: null, report });
    await adopt(report);
    return report;
  },

  cancel: () => void api.cancelRunFile(),
  dismiss: () => set({ report: null, error: null }),
}));

/** Show whatever case and study the run file left behind. */
async function adopt(report: RunReport) {
  if (report.outcomes.length === 0) return;
  const summary = await api.caseSummary();
  useWorkspace.setState((s) => ({
    caseSummary: summary,
    caseVersion: s.caseVersion + 1,
    openError: null,
    solveReport: null,
    shown: null,
    selection: null,
    drawRequest: null,
  }));
  if (!summary) return;
  // The new case version cleared the study; list the run's edits instead.
  try {
    const edits = await api.studyEdits();
    useStudy.setState({ entries: edits.map((edit) => ({ edit })) });
  } catch {
    // The case has no power flow model, so no edits either.
  }
  // Show results when the run solved: the app solves again from the run's
  // solution with its own settings, so every view has a report.
  const solved = report.outcomes.some((o) => o.command === "solve_power_flow" && !o.error);
  if (solved && summary.power.available) useWorkspace.getState().requestSolve();
}

/** "case9_study.stnrun: 6 steps in 0.4 s" and the lines that failed. */
export function describeRun(report: RunReport): { tone: "ok" | "warning"; title: string; lines: string[] } {
  const name = fileName(report.path);
  const ran = report.outcomes.length;
  const failed = report.outcomes.filter((o) => o.error && o.error.kind !== "cancelled");
  const lines = failed.map((o) => `Line ${o.line}, ${o.command}: ${o.error!.message}`);
  if (report.cancelled) return { tone: "warning", title: `Cancelled ${name} after ${ran} of ${report.steps} steps.`, lines };
  if (failed.length > 0) {
    const notRun = report.steps - ran;
    return {
      tone: "warning",
      title: `${name} stopped: ${failed.length === 1 ? "a line" : `${failed.length} lines`} failed${notRun > 0 ? `, ${notRun} not run` : ""}.`,
      lines,
    };
  }
  const took = report.seconds < 1 ? `${Math.max(1, Math.round(report.seconds * 1000))} ms` : `${report.seconds.toFixed(1)} s`;
  return { tone: "ok", title: `Ran ${name}: ${ran} step${ran === 1 ? "" : "s"} in ${took}.`, lines };
}
