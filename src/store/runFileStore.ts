// Run files (.stnrun): Record keeps the commands the app sends while you
// work and saves them as a run file; Run plays one back. The format and the
// runner live in stannum-core (run_file.rs); stannum-cli runs the same files
// without the app.
import { create } from "zustand";
import { api, pickFile, pickSavePath, RUN_FILE_FILTERS, toStannumError, watchCoreCalls } from "../lib/analysis/api";
import { ACTION_COMMANDS, type CoreCommand, type RunReport, type Step } from "../lib/analysis/commands.gen";
import { fileName } from "../lib/study/recent";
import { useStudy } from "./studyStore";
import { caseChangeOk, useWorkspace } from "./workspaceStore";

const ACTIONS = new Set<CoreCommand>(ACTION_COMMANDS);

/** Setters: calling one again with the same arguments changes nothing. */
const SETTERS = new Set<CoreCommand>(["set_study_edits", "topology_set_merge"]);

/**
 * Add one call to a recording. Only actions are kept, `case` is left out
 * (a run file works on whatever case it opened), and a setter called again
 * with the same arguments as last time is skipped.
 */
export function recordStep(steps: Step[], command: CoreCommand, args: Record<string, unknown>): Step[] {
  if (!ACTIONS.has(command)) return steps;
  const { case: _case, ...rest } = args;
  if (SETTERS.has(command)) {
    const last = [...steps].reverse().find((s) => s.command === command);
    if (last && JSON.stringify(last.args) === JSON.stringify(rest)) return steps;
  }
  return [...steps, { command, args: rest }];
}

interface RunFileState {
  recording: boolean;
  steps: Step[];
  /** The run file running now, if any. */
  running: string | null;
  /** How the last run went, until dismissed. */
  report: RunReport | null;
  error: string | null;
  startRecording: () => void;
  /** Ask where to save the recording, save it and stop. False if cancelled. */
  saveRecording: () => Promise<boolean>;
  discardRecording: () => void;
  /** Run a run file (asking for one when no path is given), then show what it left. */
  run: (path?: string) => Promise<RunReport | null>;
  cancel: () => void;
  dismiss: () => void;
}

let unwatch: (() => void) | null = null;

/**
 * The case on screen as the first steps, so the run file starts where the
 * recording did: open it, attach its side files, set its study edits.
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
  recording: false,
  steps: [],
  running: null,
  report: null,
  error: null,

  startRecording: () => {
    unwatch?.();
    unwatch = watchCoreCalls((command, args) => {
      if (get().recording) set((s) => ({ steps: recordStep(s.steps, command, args) }));
    });
    set({ recording: true, steps: startingPoint(), report: null, error: null });
  },

  saveRecording: async () => {
    const { steps } = get();
    const summary = useWorkspace.getState().caseSummary;
    const base = summary ? summary.name.replace(/\.(raw|m)$/i, "") : "recording";
    const path = await pickSavePath(`${base}.stnrun`, RUN_FILE_FILTERS);
    if (!path) return false;
    try {
      await api.saveRunFile(path, steps);
    } catch (e) {
      set({ error: toStannumError(e).message });
      return false;
    }
    get().discardRecording();
    set({ error: null });
    return true;
  },

  discardRecording: () => {
    unwatch?.();
    unwatch = null;
    set({ recording: false, steps: [] });
  },

  run: async (path) => {
    if (get().running) return null;
    const w = useWorkspace.getState();
    // A run file usually opens its own case: ask about unsaved work first.
    if (!caseChangeOk(w.caseSummary)) return null;
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
