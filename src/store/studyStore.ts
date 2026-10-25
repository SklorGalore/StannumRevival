// The open case's study: edits (outages, generator output, bus load) that
// every solve uses. The list lives here; stannum-core applies it.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import type { StudyEdit } from "../lib/analysis/types";
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

interface StudyState {
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
  /** Asks Power flow to show the study tab. */
  showRequest: number;
  show: () => void;
}

export const useStudy = create<StudyState>((set, get) => ({
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
  useStudy.setState((st) => ({ entries: [], error: null, revision: st.revision + 1 }));
});
