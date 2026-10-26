import { useLog } from "./logStore";
import { create } from "zustand";
import { api, CASE_FILTERS, pickFile, pickSavePath } from "../lib/analysis/api";
import type { CaseSummary, RecordEdit, SolveReport } from "../lib/analysis/types";

export type ModuleId = "diagram" | "power" | "records" | "diff";

/** One bus or piece of equipment selected across every module. */
export interface SharedSelection {
  /** Shared element id (see lib/elements.ts), such as "bus:101". */
  element: string;
  /** The module the user picked it in (the others follow quietly), or "search" when found with Ctrl+K. */
  from: ModuleId | "search";
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
  setSolveReport: (r: SolveReport | null) => void;
  /** Opt-in: when off, each module keeps its own selection. */
  linked: boolean;
  setLinked: (on: boolean) => void;
  selection: SharedSelection | null;
  /** Select an element everywhere (only while linked); null clears the selection. */
  select: (element: string | null, from: ModuleId) => void;
  /** Show an element in every module, linked or not (a search result). */
  focus: (element: string) => void;
  /** Incremented to ask Power flow to solve with its current settings. */
  solveRequest: number;
  requestSolve: () => void;
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
  openCase: (path?: string) => Promise<void>;
  closeCase: () => Promise<void>;
}

const LINKED_KEY = "stannum-linked-selection";

/** Before the case is replaced: ask when it has edits that aren't saved. */
function unsavedEditsOk(summary: CaseSummary | null): boolean {
  const n = summary?.record_edits ?? 0;
  if (n === 0 || typeof window === "undefined" || typeof window.confirm !== "function") return true;
  return window.confirm(`${summary!.name} has ${n} edited value${n === 1 ? "" : "s"} that aren't saved. Discard ${n === 1 ? "it" : "them"}?`);
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
  setSolveReport: (solveReport) => set({ solveReport }),
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
  requestSolve: () => set((s) => ({ solveRequest: s.solveRequest + 1, ...showing(s, "power") })),
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
      drawRequest: null,
    }));
  },
  saveCaseAs: async () => {
    const summary = get().caseSummary;
    if (!summary) return false;
    const matpower = summary.format === "matpower";
    const base = summary.name.replace(/\.(raw|m)$/i, "");
    const path = await pickSavePath(`${base}${base.endsWith("-edited") ? "" : "-edited"}.${matpower ? "m" : "raw"}`, [
      matpower ? { name: "MATPOWER", extensions: ["m"] } : { name: "PSS/E RAW", extensions: ["raw", "RAW"] },
    ]);
    if (!path) return false;
    const saved = await api.saveCaseAs(path);
    set({ caseSummary: saved });
    return true;
  },

  openCase: async (path) => {
    if (get().opening) return;
    if (!unsavedEditsOk(get().caseSummary)) return;
    const chosen = path ?? (await pickFile(CASE_FILTERS));
    if (!chosen) return;
    set({ opening: true, openError: null });
    // The log's badge counts what this case raised, not every case so far.
    useLog.getState().newCase();
    try {
      const summary = await api.openCase(chosen);
      set((s) => ({
        caseSummary: summary,
        caseVersion: s.caseVersion + 1,
        solveReport: null,
        selection: null,
        drawRequest: null,
        opening: false,
        // A RAW file outside rs-power's support may still have a topology:
        // move to the Diagram, which draws it as a network.
        module:
          s.module === "power" && !summary.power.available && summary.topology.available
            ? "diagram"
            : s.module,
      }));
    } catch (e) {
      set({ opening: false, openError: (e as Error).message });
    }
  },

  closeCase: async () => {
    if (!unsavedEditsOk(get().caseSummary)) return;
    await api.closeCase();
    useLog.getState().newCase();
    set((s) => ({ caseSummary: null, caseVersion: s.caseVersion + 1, openError: null, solveReport: null, selection: null, drawRequest: null }));
  },
}));
