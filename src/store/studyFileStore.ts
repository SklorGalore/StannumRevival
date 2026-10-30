// Saving and opening study files (.stannum): the open case's study edits,
// saved solves, solve and contingency settings, drawing and layout, kept
// beside a reference to the case file. See lib/study/studyFile.ts.
import { create } from "zustand";
import { api, CASE_FILTERS, COORDINATE_FILTERS, DC_GRID_FILTERS, pickFile, pickSavePath, RAW_FILTERS, STN_FILTERS, toStannumError } from "../lib/analysis/api";
import { addRecent, fileName } from "../lib/study/recent";
import {
  compareToFile,
  describeChange,
  parseStudyFile,
  serializeStudyFile,
  STUDY_EXTENSION,
  STUDY_FILTERS,
  STUDY_FORMAT,
  STUDY_VERSION,
  type StudyFile,
} from "../lib/study/studyFile";
import { useContingencies } from "./contingencyStore";
import { useDiagramStore } from "./diagramStore";
import { useStudy, type StudyEntry } from "./studyStore";
import { caseChangeOk, onBeforeCaseChange, useWorkspace } from "./workspaceStore";

/** The Open dialog takes a case or a study. */
export const OPEN_FILTERS = [
  { name: "Case or study", extensions: ["raw", "RAW", "m", "stn", "stannum"] },
  ...CASE_FILTERS.slice(1),
  ...STUDY_FILTERS,
];

/** Something the user should know after opening a study. */
export interface StudyNotice {
  tone: "warning" | "info";
  title: string;
  lines: string[];
}

interface StudyFileState {
  /** The study file the open case belongs to, if it was saved or opened as one. */
  path: string | null;
  /** Changes since the study was last saved or opened. */
  dirty: boolean;
  busy: "saving" | "opening" | null;
  error: string | null;
  notice: StudyNotice | null;
  dismissNotice: () => void;
  /** Save to the study's file, or ask where when there is none (or `as` is set). Resolves true once saved. */
  save: (opts?: { as?: boolean }) => Promise<boolean>;
  /** Open a study (asking for one when no path is given), with its case. */
  open: (path?: string, opts?: { confirmed?: boolean }) => Promise<boolean>;
  /** Open a case or a study, asking for either when no path is given. */
  openFile: (path?: string) => Promise<boolean>;
  /**
   * Write the case, with its Records and study edits, as a PSS/E RAW file of
   * `revision` (asking where), then say what the file couldn't keep.
   */
  exportRaw: (revision: 33 | 35) => Promise<boolean>;
  /** Write the case, or with `solved` the latest solve, as a MATPOWER file. */
  exportMatpower: (solved: boolean) => Promise<boolean>;
  /** Write the case as a Stannum case file (.stn, format preview). */
  exportStn: () => Promise<boolean>;
}

/** Whether there is anything worth keeping: edits, saved solves or a drawing. */
function hasWork(): boolean {
  const study = useStudy.getState();
  return study.entries.length > 0 || study.saved.length > 0;
}

/** Asked before the case is replaced or closed. */
function unsavedStudyOk(): boolean {
  const { path, dirty } = useStudyFile.getState();
  if (!dirty || (!path && !hasWork())) return true;
  if (typeof window === "undefined" || typeof window.confirm !== "function") return true;
  const what = path ? `${fileName(path)} has changes that aren't saved.` : "This study's edits and saved solves aren't saved to a study file.";
  return window.confirm(`${what} Discard them?`);
}

/** Where a side file (the case, a DC grid file) is now, asking the user when it can't be found. */
async function locate(study: string, absolute: string, relative: string | null, what: string, filters: typeof CASE_FILTERS) {
  const found = await api.studyLocateCase(study, absolute, relative);
  if (found) return { path: found, moved: found !== absolute };
  const ok = window.confirm(`The study's ${what}, ${fileName(absolute)}, isn't at\n${absolute}\nor beside the study. Find it yourself?`);
  if (!ok) return null;
  const picked = await pickFile(filters);
  return picked ? { path: picked, moved: true } : null;
}

/** Apply study edits; if the whole list is refused, keep the ones the case accepts. */
async function restoreEdits(entries: StudyEntry[]): Promise<string[]> {
  const study = useStudy.getState();
  if (entries.length === 0 || (await study.setEntries(entries))) return [];
  const kept: StudyEntry[] = [];
  const dropped: string[] = [];
  for (const entry of entries) {
    try {
      await api.setStudyEdits([...kept, entry].map((e) => e.edit));
      kept.push(entry);
    } catch (e) {
      dropped.push((e as Error).message);
    }
  }
  await study.setEntries(kept);
  return dropped;
}

/** Everything the study keeps, as of now. */
async function collect(studyPath: string): Promise<StudyFile> {
  const w = useWorkspace.getState();
  const summary = w.caseSummary!;
  const study = useStudy.getState();
  const diagram = useDiagramStore.getState();
  const [fingerprint, relative] = await Promise.all([
    api.fileFingerprint(summary.path),
    api.studyRelativePath(studyPath, summary.path),
  ]);
  const grids = summary.dc_grids;
  // Only an attached file is saved; a case's own substation places come with it.
  const coordinates = summary.coordinates?.path ? { path: summary.coordinates.path } : null;
  return {
    format: STUDY_FORMAT,
    version: STUDY_VERSION,
    app: __APP_VERSION__,
    savedAt: new Date().toISOString(),
    case: { path: summary.path, relative, name: summary.name, format: summary.format, fingerprint },
    dcGrids: grids ? { path: grids.path, relative: await api.studyRelativePath(studyPath, grids.path) } : null,
    coordinates: coordinates
      ? { path: coordinates.path, relative: await api.studyRelativePath(studyPath, coordinates.path) }
      : null,
    solveSettings: study.settings,
    edits: study.entries,
    solves: study.saved.map(({ name, entries, report }) => ({ name, entries, report })),
    compare: compareToFile(study.compare, study.saved.map((s) => s.id)),
    contingencies: useContingencies.getState().choice,
    diagram:
      diagram.nodes.length > 0
        ? { metadata: diagram.metadata, nodes: diagram.nodes, edges: diagram.edges, viewport: diagram.viewport }
        : null,
    view: { module: w.module, split: w.split, splitRatio: w.splitRatio },
  };
}

// While a study is being put back, its own changes don't count as edits.
let restoring = false;

export const useStudyFile = create<StudyFileState>((set, get) => ({
  path: null,
  dirty: false,
  busy: null,
  error: null,
  notice: null,
  dismissNotice: () => set({ notice: null }),

  async exportRaw(revision) {
    const summary = useWorkspace.getState().caseSummary;
    if (!summary) return false;
    const base = summary.name.replace(/\.(raw|m|stn)$/i, "");
    const path = await pickSavePath(`${base}_v${revision}.raw`, RAW_FILTERS);
    if (!path) return false;
    try {
      const done = await api.exportRaw(path, revision);
      set({
        error: null,
        notice: {
          // Notes say what the file couldn't keep, or that it came from the power flow model.
          tone: done.notes.length ? "warning" : "info",
          title: `Exported ${fileName(path)} as PSS/E RAW ${revision}: ${done.records.toLocaleString()} records`,
          lines: done.notes.length ? done.notes : ["Every record of the case is in the file."],
        },
      });
      return true;
    } catch (e) {
      set({ notice: null, error: `Couldn't export ${fileName(path)}: ${toStannumError(e).message}` });
      return false;
    }
  },

  async exportMatpower(solved) {
    const summary = useWorkspace.getState().caseSummary;
    if (!summary) return false;
    // MATPOWER function names start with a letter and hold only word characters.
    const stem = summary.name.replace(/\.[^.]+$/, "").replace(/[^A-Za-z0-9_]/g, "_");
    const name = /^[A-Za-z]/.test(stem) ? stem : `case_${stem}`;
    const path = await pickSavePath(`${name}${solved ? "_solved" : ""}.m`, [{ name: "MATPOWER", extensions: ["m"] }]);
    if (!path) return false;
    try {
      await api.exportMatpower(path, solved);
      set({ error: null, notice: { tone: "info", title: `Exported ${fileName(path)} as MATPOWER${solved ? " with the latest solve" : ""}`, lines: [] } });
      return true;
    } catch (e) {
      set({ notice: null, error: `Couldn't export ${fileName(path)}: ${toStannumError(e).message}` });
      return false;
    }
  },

  async exportStn() {
    const summary = useWorkspace.getState().caseSummary;
    if (!summary) return false;
    const base = summary.name.replace(/\.(raw|m|stn)$/i, "");
    const path = await pickSavePath(`${base}.stn`, STN_FILTERS);
    if (!path) return false;
    try {
      const done = await api.exportStn(path);
      set({
        error: null,
        notice: {
          tone: "info",
          title: `Exported ${fileName(path)} as a Stannum case: ${done.records.toLocaleString()} records`,
          lines: done.notes.length ? done.notes : ["Every record of the case is in the file."],
        },
      });
      return true;
    } catch (e) {
      set({ notice: null, error: `Couldn't export ${fileName(path)}: ${toStannumError(e).message}` });
      return false;
    }
  },

  async save(opts) {
    const w = useWorkspace.getState();
    const summary = w.caseSummary;
    if (!summary || get().busy) return false;
    // A study points at a case file, so Records edits have to be in one first.
    const recordEdits = summary.record_edits ?? 0;
    if (recordEdits > 0) {
      const ok = window.confirm(
        `${summary.name} has ${recordEdits} edited value${recordEdits === 1 ? "" : "s"} from Records that aren't in a file yet. ` +
          "A study refers to its case file, so save the edited case first. Save it now?"
      );
      if (!ok || !(await w.saveCaseAs())) return false;
    }
    const base = summary.name.replace(/\.(raw|m|stn)$/i, "");
    const path = (!opts?.as && get().path) || (await pickSavePath(`${base}.stannum`, STUDY_FILTERS));
    if (!path) return false;
    set({ busy: "saving", error: null });
    try {
      const file = await collect(path);
      await api.writeTextFile(path, serializeStudyFile(file));
      set({ path, dirty: false, busy: null });
      addRecent(path, "study");
      return true;
    } catch (e) {
      set({ busy: null, error: `Couldn't save the study: ${(e as Error).message}` });
      return false;
    }
  },

  async openFile(given) {
    if (get().busy || useWorkspace.getState().opening) return false;
    if (!caseChangeOk(useWorkspace.getState().caseSummary)) return false;
    const path = given ?? (await pickFile(OPEN_FILTERS));
    if (!path) return false;
    return STUDY_EXTENSION.test(path)
      ? get().open(path, { confirmed: true })
      : useWorkspace.getState().openCase(path, { confirmed: true });
  },

  async open(given, opts) {
    if (get().busy) return false;
    if (!opts?.confirmed && !caseChangeOk(useWorkspace.getState().caseSummary)) return false;
    const path = given ?? (await pickFile(STUDY_FILTERS));
    if (!path) return false;
    set({ busy: "opening", error: null, notice: null });
    const fail = (message: string) => {
      set({ busy: null, error: message });
      return false;
    };
    let file: StudyFile;
    try {
      file = await parseStudyFile(await api.readTextFile(path));
    } catch (e) {
      return fail(`Couldn't open ${fileName(path)}: ${(e as Error).message}`);
    }

    const lines: string[] = [];
    let warning = false;
    const where = await locate(path, file.case.path, file.case.relative, "case", CASE_FILTERS);
    if (!where) return fail(`${fileName(path)} wasn't opened: its case file couldn't be found.`);
    if (where.moved) lines.push(`The case was found at ${where.path}.`);
    try {
      const change = describeChange(file.case.fingerprint, await api.fileFingerprint(where.path));
      if (change) {
        warning = true;
        lines.push(
          `${fileName(where.path)} has changed since the study was saved on ${new Date(file.savedAt).toLocaleString()}: ${change}. ` +
            "Saved solves show results for the case as it was then."
        );
      }
    } catch (e) {
      return fail(`Couldn't read the case: ${(e as Error).message}`);
    }

    restoring = true;
    try {
      if (!(await useWorkspace.getState().openCase(where.path, { confirmed: true }))) {
        return fail(useWorkspace.getState().openError ?? "The case couldn't be opened.");
      }
      if (file.dcGrids) {
        const grids = await locate(path, file.dcGrids.path, file.dcGrids.relative, "DC grid file", DC_GRID_FILTERS);
        if (grids) {
          try {
            const attached = await api.loadDcGrids(grids.path);
            useWorkspace.setState((s) => ({ caseSummary: s.caseSummary && { ...s.caseSummary, dc_grids: attached } }));
          } catch (e) {
            warning = true;
            lines.push(`The DC grid file couldn't be attached: ${(e as Error).message}`);
          }
        } else {
          warning = true;
          lines.push("The DC grid file wasn't attached.");
        }
      }
      if (file.coordinates) {
        const found = await locate(path, file.coordinates.path, file.coordinates.relative, "bus coordinates file", COORDINATE_FILTERS);
        if (found) {
          try {
            const attached = await api.loadCoordinates(found.path);
            useWorkspace.setState((s) => ({ caseSummary: s.caseSummary && { ...s.caseSummary, coordinates: attached } }));
          } catch (e) {
            warning = true;
            lines.push(`The bus coordinates file couldn't be attached: ${(e as Error).message}`);
          }
        } else {
          warning = true;
          lines.push("The bus coordinates file wasn't attached.");
        }
      }
      const study = useStudy.getState();
      study.setSettings({ ...study.settings, ...file.solveSettings });
      const dropped = await restoreEdits(file.edits);
      if (dropped.length) {
        warning = true;
        lines.push(`${dropped.length} edit${dropped.length === 1 ? "" : "s"} no longer match the case and were left out: ${dropped.join(" ")}`);
      }
      useStudy.getState().restoreSolves(file.solves, file.compare);
      useContingencies.getState().setChoice(file.contingencies);
      const diagram = useDiagramStore.getState();
      if (file.diagram) {
        diagram.loadDiagramFromJSON(file.diagram);
        diagram.setFilePath(null);
      }
      // Results aren't kept between sessions (saved solves are): solve again.
      const w = useWorkspace.getState();
      if (w.caseSummary?.power.available) w.requestSolve();
      // Then put the modules back where they were.
      w.setSplit(null);
      useWorkspace.setState({ module: file.view.module, splitRatio: file.view.splitRatio });
      if (file.view.split && file.view.split !== file.view.module) w.setSplit(file.view.split);
    } finally {
      restoring = false;
    }
    set({
      path,
      dirty: false,
      busy: null,
      notice: lines.length ? { tone: warning ? "warning" : "info", title: `Opened ${fileName(path)}`, lines } : null,
    });
    addRecent(path, "study");
    return true;
  },
}));

// Any change to the work marks the study as changed.
const touch = () => {
  if (!restoring && useWorkspace.getState().caseSummary && !useStudyFile.getState().dirty) useStudyFile.setState({ dirty: true });
};
useStudy.subscribe((s, prev) => {
  if (s.entries !== prev.entries || s.saved !== prev.saved || s.settings !== prev.settings || s.compare !== prev.compare) touch();
});
useContingencies.subscribe((s, prev) => s.choice !== prev.choice && touch());
// The drawing counts once it has edits of its own (results drawn on it don't).
useDiagramStore.subscribe((s, prev) => s.isDirty && !prev.isDirty && touch());

// A different case (or none) ends the study; edits made in Records keep it.
let lastVersion = useWorkspace.getState().caseVersion;
useWorkspace.subscribe((s) => {
  if (s.caseVersion === lastVersion) return;
  lastVersion = s.caseVersion;
  if (s.caseVersion === s.editedVersion) return;
  if (!restoring) useStudyFile.setState({ path: null, dirty: false, notice: null, error: null });
});

onBeforeCaseChange(unsavedStudyOk);
