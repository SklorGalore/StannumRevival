import { useLog } from "./logStore";
import { create } from "zustand";
import { api, CASE_FILTERS, COORDINATE_FILTERS, pickFile, pickSavePath } from "../lib/analysis/api";
import type { CaseSummary, RecordEdit, SolveReport } from "../lib/analysis/types";
import { addRecent } from "../lib/study/recent";
import { confirmDialog } from "../lib/tauri/commands";

/** Power flow tabs other parts of the app can open. */
export type PowerTabRequest = "warnings" | "vsc" | "contingencies" | "transfers" | "study";

export type ModuleId = "diagram" | "power" | "records" | "diff";

/** One bus or piece of equipment selected across every module. */
export interface SharedSelection {
  /** Shared element id (see lib/elements.ts), such as "bus:101". */
  element: string;
  /** The module the user picked it in (the others follow quietly), or "search" when found with Ctrl+K. */
  from: ModuleId | "search";
}

/**
 * A result other than the latest solve, shown by every view in its place:
 * one outage of the contingency run, or the last "Trip and compare". Named
 * as stannum-core names it (see crates/stannum-core/src/results.rs).
 */
export interface ShownResult {
  /** The core's name, such as "contingency 3" or "trip line:1-2-1". */
  name: string;
  /** How the case bar and the tables name it, such as "Outage #3". */
  label: string;
  /** The element the result takes out. */
  element: string;
  /** Null while it loads; views keep showing the latest solve until then. */
  report: SolveReport | null;
  error: string | null;
}

/** The core's name for the latest solve: with study edits or without. */
export function latestResultName(studyEdits: number): "base" | "study" {
  return studyEdits > 0 ? "study" : "base";
}

/** The module that goes beside another when split view opens without a choice. */
export const SPLIT_PARTNER: Record<ModuleId, ModuleId> = { diagram: "power", power: "diagram", records: "diagram", diff: "diagram" };

/** Bring a module into view: switch to it, unless split view already shows it. */
export function showing(s: { module: ModuleId; split: ModuleId | null }, m: ModuleId): { module: ModuleId } {
  return { module: s.split === m ? s.module : m };
}

interface WorkspaceState {
  /** The module in view, or on the left in split view. */
  module: ModuleId;
  /** Bring a module into view (a module already on the right stays there). */
  setModule: (m: ModuleId) => void;
  /** Swap the two sides of split view. */
  swapSplit: () => void;
  /** The module on the right in split view, or null for one module at a time. */
  split: ModuleId | null;
  /** Open split view with a module on the right (linking selection while it is open), or close it. */
  setSplit: (m: ModuleId | null) => void;
  /** The left pane's share of the width. */
  splitRatio: number;
  setSplitRatio: (r: number) => void;
  /** Whether opening split view turned linking on, so closing it turns it back off. */
  splitLinked: boolean;
  caseSummary: CaseSummary | null;
  /** Increments on every open/close so modules can drop stale results. */
  caseVersion: number;
  opening: boolean;
  openError: string | null;
  /** Latest power flow solve of the open case, shared by Power flow and Diagram. */
  solveReport: SolveReport | null;
  /** A new solve: every view goes back to showing it. */
  setSolveReport: (r: SolveReport | null) => void;
  /** A solve made outside the window (an attached script), shown as the latest. */
  adoptSolve: (r: SolveReport) => void;
  /** Bumps on every adopted solve, so Power flow knows its results are current. */
  adoptedSolves: number;
  /**
   * The result every view shows instead of the latest solve, or null for the
   * latest solve. Read it through useShownReport().
   */
  shown: ShownResult | null;
  /** Show a named result in every view (fetched from the core once). */
  showResult: (result: { name: string; label: string; element: string }) => Promise<void>;
  /** Go back to showing the latest solve. */
  showLatest: () => void;
  /** Opt-in: when off, each module keeps its own selection. */
  linked: boolean;
  setLinked: (on: boolean) => void;
  selection: SharedSelection | null;
  /** Select an element everywhere (only while linked); null clears the selection. */
  select: (element: string | null, from: ModuleId) => void;
  /** Show an element in every module, linked or not (a search result). */
  focus: (element: string) => void;
  /** Incremented to ask Power flow to solve with its current settings, without leaving the current view. */
  solveRequest: number;
  requestSolve: () => void;
  /** Incremented to ask Power flow to calculate sensitivities (toolbar, menu, Ctrl+K). */
  sensitivityRequest: number;
  requestSensitivities: () => void;
  /** Asks Power flow to open one of its tabs (toolbar, menu, Ctrl+K). */
  powerTabRequest: { tab: PowerTabRequest; n: number } | null;
  showPowerTab: (tab: PowerTabRequest) => void;
  /** The Ctrl+K search and command palette. */
  paletteOpen: boolean;
  setPaletteOpen: (open: boolean) => void;
  /** A request to draw a one-line around an element; n tells repeat requests apart. */
  drawRequest: { element: string; n: number } | null;
  /** Open the diagram drawn around an element (selecting it too while linked). */
  drawAround: (element: string, from: ModuleId) => void;
  /**
   * The caseVersion that applying edits in Records produced: the same file,
   * changed, so a view can keep its place (and the study its edits).
   */
  editedVersion: number;
  /** Write cell edits from Records into the case; every module reloads it. Throws when an edit is refused. */
  applyRecordEdits: (edits: RecordEdit[]) => Promise<void>;
  /** Save the edited case to a new file, which then becomes the open case. */
  saveCaseAs: () => Promise<boolean>;
  /**
   * Open a case (asking for one when no path is given). Resolves true once it
   * is open. `confirmed` skips the unsaved-work questions, for a caller that
   * has already asked them.
   */
  openCase: (path?: string, opts?: { confirmed?: boolean }) => Promise<boolean>;
  /** Open one of the sample cases built into Stannum, by id (see `api.sampleCases`). */
  openSample: (id: string) => Promise<boolean>;
  closeCase: () => Promise<boolean>;
  /** Attach a bus coordinates CSV for the map (asking for one when no path is given). */
  attachCoordinates: (path?: string) => Promise<boolean>;
  detachCoordinates: () => Promise<void>;
}

const LINKED_KEY = "stannum-linked-selection";

/**
 * Questions to ask before the open case is replaced or closed, such as
 * "the study has unsaved changes". Each returns false to stop.
 */
const beforeCaseChange: (() => Promise<boolean>)[] = [];
export function onBeforeCaseChange(check: () => Promise<boolean>): () => void {
  beforeCaseChange.push(check);
  return () => {
    const i = beforeCaseChange.indexOf(check);
    if (i >= 0) beforeCaseChange.splice(i, 1);
  };
}

/** Ask every question in turn; the first "no" stops. */
export async function caseChangeOk(summary: CaseSummary | null): Promise<boolean> {
  if (!(await unsavedEditsOk(summary))) return false;
  for (const check of [...beforeCaseChange]) if (!(await check())) return false;
  return true;
}

/** Before the case is replaced: ask when it has edits that aren't saved. */
async function unsavedEditsOk(summary: CaseSummary | null): Promise<boolean> {
  const n = summary?.record_edits ?? 0;
  if (n === 0) return true;
  return confirmDialog(`${summary!.name} has ${n} edited value${n === 1 ? "" : "s"} that aren't saved. Discard ${n === 1 ? "it" : "them"}?`);
}

function loadLinked(): boolean {
  try {
    return localStorage.getItem(LINKED_KEY) === "1";
  } catch {
    return false;
  }
}

export const useWorkspace = create<WorkspaceState>((set, get) => ({
  module: "power",
  setModule: (module) => set((s) => showing(s, module)),
  swapSplit: () => set((s) => (s.split ? { module: s.split, split: s.module } : {})),
  split: null,
  splitLinked: false,
  setSplit: (split) => {
    const s = get();
    if (split === null) {
      if (s.splitLinked) set({ linked: false, selection: null });
      set({ split: null, splitLinked: false });
      return;
    }
    if (split === s.module) return;
    // Side by side is where linked selection pays off, so it is on while split.
    if (!s.split && !s.linked) set({ linked: true, splitLinked: true });
    set({ split });
  },
  splitRatio: 0.5,
  setSplitRatio: (r) => set({ splitRatio: Math.min(0.8, Math.max(0.2, r)) }),
  caseSummary: null,
  caseVersion: 0,
  opening: false,
  openError: null,
  solveReport: null,
  setSolveReport: (solveReport) => set({ solveReport, shown: null }),
  adoptSolve: (solveReport) => set((s) => ({ solveReport, shown: null, adoptedSolves: s.adoptedSolves + 1 })),
  adoptedSolves: 0,
  shown: null,
  showResult: async ({ name, label, element }) => {
    if (get().shown?.name === name) return;
    const version = get().caseVersion;
    const solved = get().solveReport;
    set({ shown: { name, label, element, report: null, error: null } });
    // Drop the answer if the case, the solve or the choice moved on meanwhile.
    const current = () => {
      const s = get();
      return s.caseVersion === version && s.solveReport === solved && s.shown?.name === name;
    };
    try {
      const report = await api.resultReport(name);
      if (current()) set((s) => ({ shown: s.shown && { ...s.shown, report } }));
    } catch (e) {
      if (current()) set((s) => ({ shown: s.shown && { ...s.shown, error: (e as Error).message } }));
    }
  },
  showLatest: () => {
    if (get().shown) set({ shown: null });
  },
  linked: loadLinked(),
  setLinked: (linked) => {
    try {
      localStorage.setItem(LINKED_KEY, linked ? "1" : "0");
    } catch {
      // Storage may be unavailable; the choice still holds for this session.
    }
    set({ linked, selection: null, splitLinked: false });
  },
  selection: null,
  select: (element, from) => {
    const current = get().selection;
    if (!get().linked) {
      // Picking something by hand replaces a search result.
      if (current?.from === "search") set({ selection: null });
      return;
    }
    if (element === null) {
      if (current) set({ selection: null });
      return;
    }
    if (current?.element === element) return;
    set({ selection: { element, from } });
  },

  focus: (element) => set({ selection: { element, from: "search" } }),
  solveRequest: 0,
  // Solving keeps the current view: the diagram, tables and inspector update
  // in place. Power flow comes into view only if the solve fails.
  requestSolve: () => set((s) => ({ solveRequest: s.solveRequest + 1 })),
  sensitivityRequest: 0,
  requestSensitivities: () => set((s) => ({ sensitivityRequest: s.sensitivityRequest + 1, ...showing(s, "power") })),
  powerTabRequest: null,
  showPowerTab: (tab) => set((s) => ({ powerTabRequest: { tab, n: (s.powerTabRequest?.n ?? 0) + 1 }, ...showing(s, "power") })),
  paletteOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  drawRequest: null,
  drawAround: (element, from) => {
    get().select(element, from);
    set((s) => ({ drawRequest: { element, n: (s.drawRequest?.n ?? 0) + 1 }, ...showing(s, "diagram") }));
  },

  editedVersion: -1,
  applyRecordEdits: async (edits) => {
    const summary = await api.applyRecordEdits(edits);
    set((s) => ({
      caseSummary: summary,
      caseVersion: s.caseVersion + 1,
      editedVersion: s.caseVersion + 1,
      // Results describe the case before the edits.
      solveReport: null,
      shown: null,
      drawRequest: null,
    }));
  },
  saveCaseAs: async () => {
    const summary = get().caseSummary;
    if (!summary) return false;
    const matpower = summary.format === "matpower";
    const stn = summary.format === "stn";
    const binary = /\.stnb$/i.test(summary.name);
    const base = summary.name.replace(/\.(raw|m|stnb?)$/i, "");
    // RAW and Stannum cases save as either; the extension picks the format.
    // A Stannum case keeps its form; otherwise binary is offered first.
    const raw = { name: "PSS/E RAW", extensions: ["raw", "RAW"] };
    const stnFilter = { name: "Stannum case (preview)", extensions: ["stn"] };
    const stnbFilter = { name: "Binary Stannum case (preview)", extensions: ["stnb"] };
    const path = await pickSavePath(
      `${base}${base.endsWith("-edited") ? "" : "-edited"}.${matpower ? "m" : binary ? "stnb" : stn ? "stn" : "raw"}`,
      matpower
        ? [{ name: "MATPOWER", extensions: ["m"] }]
        : binary
          ? [stnbFilter, stnFilter, raw]
          : stn
            ? [stnFilter, stnbFilter, raw]
            : [raw, stnbFilter, stnFilter]
    );
    if (!path) return false;
    const saved = await api.saveCaseAs(path);
    set({ caseSummary: saved });
    return true;
  },

  openCase: async (path, opts) => {
    if (get().opening) return false;
    if (!opts?.confirmed && !(await caseChangeOk(get().caseSummary))) return false;
    const chosen = path ?? (await pickFile(CASE_FILTERS));
    if (!chosen) return false;
    const summary = await opened(set, () => api.openCase(chosen));
    if (summary) addRecent(summary.path, "case");
    return !!summary;
  },

  openSample: async (id) => {
    if (get().opening) return false;
    if (!(await caseChangeOk(get().caseSummary))) return false;
    // Samples aren't added to Recent: the empty screen always lists them.
    return !!(await opened(set, async () => api.openSample(id, await sampleFolder())));
  },

  attachCoordinates: async (path) => {
    const chosen = path ?? (await pickFile(COORDINATE_FILTERS));
    if (!chosen) return false;
    try {
      const coordinates = await api.loadCoordinates(chosen);
      set((s) => ({ caseSummary: s.caseSummary && { ...s.caseSummary, coordinates } }));
      return true;
    } catch {
      // The log already shows why.
      return false;
    }
  },

  detachCoordinates: async () => {
    await api.clearCoordinates();
    set((s) => ({ caseSummary: s.caseSummary && { ...s.caseSummary, coordinates: null } }));
  },

  closeCase: async () => {
    if (!(await caseChangeOk(get().caseSummary))) return false;
    await api.closeCase();
    useLog.getState().newCase();
    set((s) => ({ caseSummary: null, caseVersion: s.caseVersion + 1, openError: null, solveReport: null, shown: null, selection: null, drawRequest: null }));
    return true;
  },
}));

/**
 * Open a case with `open`, replacing the one open now, and reset what
 * belonged to the old case. Resolves to the new case's summary, or null when
 * it couldn't be opened (`openError` says why).
 */
async function opened(
  set: (fn: (s: WorkspaceState) => Partial<WorkspaceState>) => void,
  open: () => Promise<CaseSummary>
): Promise<CaseSummary | null> {
  set(() => ({ opening: true, openError: null }));
  // The log's badge counts what this case raised, not every case so far.
  useLog.getState().newCase();
  try {
    const summary = await open();
    set((s) => ({
      caseSummary: summary,
      caseVersion: s.caseVersion + 1,
      solveReport: null,
      shown: null,
      selection: null,
      drawRequest: null,
      opening: false,
      // A RAW file outside rs-power's support may still have a topology:
      // move to the Diagram, which draws it as a network.
      module:
        s.module === "power" && !summary.power.available && summary.topology.available ? "diagram" : s.module,
    }));
    return summary;
  } catch (e) {
    set(() => ({ opening: false, openError: (e as Error).message }));
    return null;
  }
}

/**
 * Where the desktop app keeps its copies of the sample cases: a "samples"
 * folder in its local data folder. Undefined outside the app (and if the
 * folder can't be found), so the core uses the temp folder.
 */
async function sampleFolder(): Promise<string | undefined> {
  try {
    const { isTauri } = await import("@tauri-apps/api/core");
    if (!isTauri()) return undefined;
    const { appLocalDataDir, join } = await import("@tauri-apps/api/path");
    return await join(await appLocalDataDir(), "samples");
  } catch {
    return undefined;
  }
}

/**
 * The solve report every view shows: the shown result once it has loaded,
 * else the latest solve.
 */
export function shownReport(s: Pick<WorkspaceState, "shown" | "solveReport">): SolveReport | null {
  return s.shown?.report ?? s.solveReport;
}

export function useShownReport(): SolveReport | null {
  return useWorkspace(shownReport);
}
