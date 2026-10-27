// The open case's study: edits (outages, generator output, bus load) that
// every solve uses. The list lives here; stannum-core applies it.
import { create } from "zustand";
import { api, DEFAULT_SOLVE_SETTINGS } from "../lib/analysis/api";
import type { SolveReport, SolveSettings, StudyEdit } from "../lib/analysis/types";
import { useWorkspace } from "./workspaceStore";

/** What an edit replaced, for showing "was …" beside it. */
export interface StudyEntry {
  edit: StudyEdit;
  was?: string;
}

/** The thing an edit changes, so a newer edit to it replaces the older one. */
export function editTarget(e: StudyEdit): string {
  return e.kind === "load" ? `load:${e.bus}` : `${e.kind}:${e.element}`;
}

/** Add an edit, replacing an earlier one to the same thing. Putting something
 * back in service just drops its outage. */
export function withEdit(entries: StudyEntry[], entry: StudyEntry): StudyEntry[] {
  const target = editTarget(entry.edit);
  const earlier = entries.find((e) => editTarget(e.edit) === target);
  const rest = entries.filter((e) => e !== earlier);
  if (entry.edit.kind === "status" && entry.edit.online && earlier) return rest;
  // "Was" stays the file's value, not the previous edit's.
  return [...rest, earlier?.was ? { ...entry, was: earlier.was } : entry];
}

/** A solve kept for comparing, with the edits it was solved with. */
export interface SavedSolve {
  id: number;
  name: string;
  report: SolveReport;
  entries: StudyEntry[];
}

/** A side of a comparison: a saved solve, or whatever is solved now. */
export type CompareSide = number | "current";

interface StudyState {
  /** Power flow's solve settings, kept here so a study file can save them. */
  settings: SolveSettings;
  setSettings: (settings: SolveSettings) => void;
  saved: SavedSolve[];
  /** Put back saved solves read from a study file, numbering them afresh. */
  restoreSolves: (solves: Omit<SavedSolve, "id">[], compare: { a: CompareSide; b: CompareSide } | null) => void;
  saveSolve: (report: SolveReport, name: string) => void;
  renameSolve: (id: number, name: string) => void;
  deleteSolve: (id: number) => void;
  /** Which two solves to compare, before and after. */
  compare: { a: CompareSide; b: CompareSide } | null;
  setCompare: (c: { a: CompareSide; b: CompareSide } | null) => void;
  entries: StudyEntry[];
  /** Bumps on every change the backend accepted, so Power flow can re-solve. */
  revision: number;
  error: string | null;
  busy: boolean;
  /** Replace the edits; on a refusal nothing changes and `error` says why. */
  setEntries: (entries: StudyEntry[]) => Promise<boolean>;
  add: (entry: StudyEntry) => Promise<boolean>;
  remove: (index: number) => Promise<boolean>;
  clear: () => Promise<boolean>;
  /** Asks Case diff to compare the working case with its file. */
  diffRequest: number;
  showInDiff: () => void;
  /** Asks Power flow to show the study tab. */
  showRequest: number;
  show: () => void;
}

let nextSolve = 1;

export const useStudy = create<StudyState>((set, get) => ({
  settings: DEFAULT_SOLVE_SETTINGS,
  setSettings: (settings) => set({ settings }),
  saved: [],
  restoreSolves: (solves, compare) => {
    // A compare side names a solve by its place in the list it came with.
    const restored = solves.map((x) => ({ ...x, id: nextSolve++ }));
    const side = (c: CompareSide): CompareSide | null => (c === "current" ? c : restored[c]?.id ?? null);
    const a = compare && side(compare.a);
    const b = compare && side(compare.b);
    set({ saved: restored, compare: a != null && b != null ? { a, b } : null });
  },
  saveSolve: (report, name) =>
    set((s) => {
      const solve = { id: nextSolve++, name, report, entries: s.entries };
      const saved = [...s.saved, solve];
      // The first save sets up "this solve against now"; later ones compare with the one before.
      const compare = s.compare ?? { a: solve.id, b: "current" as const };
      return { saved, compare };
    }),
  renameSolve: (id, name) => set((s) => ({ saved: s.saved.map((x) => (x.id === id ? { ...x, name } : x)) })),
  deleteSolve: (id) =>
    set((s) => ({
      saved: s.saved.filter((x) => x.id !== id),
      compare: s.compare && (s.compare.a === id || s.compare.b === id) ? null : s.compare,
    })),
  compare: null,
  setCompare: (compare) => set({ compare }),
  entries: [],
  revision: 0,
  error: null,
  busy: false,
  setEntries: async (entries) => {
    const version = useWorkspace.getState().caseVersion;
    set({ busy: true, error: null });
    try {
      await api.setStudyEdits(entries.map((e) => e.edit));
      if (version !== useWorkspace.getState().caseVersion) return false;
      set((s) => ({ entries, revision: s.revision + 1, busy: false }));
      return true;
    } catch (e) {
      set({ busy: false, error: (e as Error).message });
      return false;
    }
  },
  add: (entry) => get().setEntries(withEdit(get().entries, entry)),
  remove: (index) => get().setEntries(get().entries.filter((_, i) => i !== index)),
  clear: () => get().setEntries([]),
  diffRequest: 0,
  showInDiff: () => {
    set((s) => ({ diffRequest: s.diffRequest + 1 }));
    useWorkspace.getState().setModule("diff");
  },
  showRequest: 0,
  show: () => {
    set((s) => ({ showRequest: s.showRequest + 1 }));
    useWorkspace.getState().setModule("power");
  },
}));

// A new case starts a new study.
let lastVersion = useWorkspace.getState().caseVersion;
useWorkspace.subscribe((s) => {
  if (s.caseVersion === lastVersion) return;
  lastVersion = s.caseVersion;
  // Edits made in Records keep the study: the backend applies it again.
  if (s.caseVersion === s.editedVersion) return;
  useStudy.setState((st) => ({ entries: [], saved: [], compare: null, error: null, revision: st.revision + 1 }));
});
