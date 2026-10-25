import { create } from "zustand";
import { api, CASE_FILTERS, pickFile } from "../lib/analysis/api";
import type { CaseSummary, SolveReport } from "../lib/analysis/types";

export type ModuleId = "diagram" | "power" | "topology" | "diff";

/** One bus or piece of equipment selected across every module. */
export interface SharedSelection {
  /** Shared element id (see lib/elements.ts), such as "bus:101". */
  element: string;
  /** The module the user picked it in (the others follow quietly), or "search" when found with Ctrl+K. */
  from: ModuleId | "search";
}

interface WorkspaceState {
  module: ModuleId;
  setModule: (m: ModuleId) => void;
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
  openCase: (path?: string) => Promise<void>;
  closeCase: () => Promise<void>;
}

const LINKED_KEY = "stannum-linked-selection";

function loadLinked(): boolean {
  try {
    return localStorage.getItem(LINKED_KEY) === "1";
  } catch {
    return false;
  }
}

export const useWorkspace = create<WorkspaceState>((set, get) => ({
  module: "power",
  setModule: (module) => set({ module }),
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
    set({ linked, selection: null });
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
  requestSolve: () => set((s) => ({ solveRequest: s.solveRequest + 1, module: "power" })),
  paletteOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  drawRequest: null,
  drawAround: (element, from) => {
    get().select(element, from);
    set((s) => ({ drawRequest: { element, n: (s.drawRequest?.n ?? 0) + 1 }, module: "diagram" }));
  },

  openCase: async (path) => {
    if (get().opening) return;
    const chosen = path ?? (await pickFile(CASE_FILTERS));
    if (!chosen) return;
    set({ opening: true, openError: null });
    try {
      const summary = await api.openCase(chosen);
      set((s) => ({
        caseSummary: summary,
        caseVersion: s.caseVersion + 1,
        solveReport: null,
        selection: null,
        drawRequest: null,
        opening: false,
        // A MATPOWER file has no topology, and a RAW file outside rs-power's
        // support may still have one: move to a module that can use it.
        module:
          s.module === "power" && !summary.power.available && summary.topology.available
            ? "topology"
            : s.module,
      }));
    } catch (e) {
      set({ opening: false, openError: (e as Error).message });
    }
  },

  closeCase: async () => {
    await api.closeCase();
    set((s) => ({ caseSummary: null, caseVersion: s.caseVersion + 1, openError: null, solveReport: null, selection: null, drawRequest: null }));
  },
}));
