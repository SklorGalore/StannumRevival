import { create } from "zustand";
import { api, CASE_FILTERS, pickFile } from "../lib/analysis/api";
import type { CaseSummary, SolveReport } from "../lib/analysis/types";

export type ModuleId = "diagram" | "power" | "topology" | "diff";

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
  openCase: (path?: string) => Promise<void>;
  closeCase: () => Promise<void>;
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
    set((s) => ({ caseSummary: null, caseVersion: s.caseVersion + 1, openError: null, solveReport: null }));
  },
}));
