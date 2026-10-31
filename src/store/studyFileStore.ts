// Saving the work as a run file, and opening cases, run files and old study
// files. Save writes a run file (.stnrun) that opens the case, attaches its
// side files, sets the study edits and solves with the app's settings.
// Study files (.stannum) are no longer written; old ones still open, and
// their saved solves and drawing come back for the session (see
// lib/study/studyFile.ts).
import { create } from "zustand";
import { api, CASE_FILTERS, COORDINATE_FILTERS, DC_GRID_FILTERS, pickFile, pickSavePath, RAW_FILTERS, RUN_FILE_FILTERS, STN_FILTERS, STNB_FILTERS, toStannumError } from "../lib/analysis/api";
import type { Step } from "../lib/analysis/commands.gen";
import { addRecent, fileName } from "../lib/study/recent";
import { confirmDialog } from "../lib/tauri/commands";
import { describeChange, parseStudyFile, STUDY_EXTENSION, STUDY_FILTERS, type StudyFile } from "../lib/study/studyFile";
import { useContingencies } from "./contingencyStore";
import { useDiagramStore } from "./diagramStore";
import { startingPoint, useRunFile } from "./runFileStore";
import { useStudy, type StudyEntry } from "./studyStore";
import { caseChangeOk, onBeforeCaseChange, useWorkspace } from "./workspaceStore";

/** The Open dialog takes a case, a run file or an old study file. */
export const OPEN_FILTERS = [
  { name: "Case or run file", extensions: ["raw", "RAW", "m", "stn", "stnb", "stnrun", "stannum"] },
  ...CASE_FILTERS.slice(1),
  ...RUN_FILE_FILTERS,
  { name: "Stannum study (older versions)", extensions: ["stannum"] },
];

const RUN_EXTENSION = /\.stnrun$/i;

/** Something the user should know after opening a study or exporting. */
export interface StudyNotice {
  tone: "warning" | "info";
  title: string;
  lines: string[];
}

interface StudyFileState {
  /** The run file this work was last saved to, if any. */
  path: string | null;
  /** Edits or solve settings changed since the work was last saved. */
  dirty: boolean;
  busy: "saving" | "opening" | null;
  error: string | null;
  notice: StudyNotice | null;
  dismissNotice: () => void;
  /**
   * Save the work as a run file: to the one saved last, or asking where when
   * there is none (or `as` is set). Resolves true once saved.
   */
  save: (opts?: { as?: boolean }) => Promise<boolean>;
  /** Open an old study file (asking for one when no path is given), with its case. */
  open: (path?: string, opts?: { confirmed?: boolean }) => Promise<boolean>;
  /** Open a case, run a run file or open an old study, asking for one when no path is given. */
  openFile: (path?: string) => Promise<boolean>;
  /**
   * Write the case, with its Records and study edits, as a PSS/E RAW file of
   * `revision` (asking where), then say what the file couldn't keep.
   */
  exportRaw: (revision: 33 | 35) => Promise<boolean>;
  /** Write the case, or with `solved` the latest solve, as a MATPOWER file. */
  exportMatpower: (solved: boolean) => Promise<boolean>;
  /** Write the case as a Stannum case file (.stn, or .stnb when `binary`; format preview). */
  exportStn: (binary: boolean) => Promise<boolean>;
}

/** Whether there is anything a run file would keep: study edits. */
function hasWork(): boolean {
  return useStudy.getState().entries.length > 0;
}

/** Asked before the case is replaced or closed. */
async function unsavedStudyOk(): Promise<boolean> {
  const { path, dirty } = useStudyFile.getState();
  if (!dirty || (!path && !hasWork())) return true;
  const what = path ? `${fileName(path)} has changes that aren't saved.` : "The study edits aren't saved to a run file.";
  return confirmDialog(`${what} Discard them?`);
}

/** Where a side file (the case, a DC grid file) is now, asking the user when it can't be found. */
async function locate(study: string, absolute: string, relative: string | null, what: string, filters: typeof CASE_FILTERS) {
  const found = await api.studyLocateCase(study, absolute, relative);
  if (found) return { path: found, moved: found !== absolute };
  const ok = await confirmDialog(`The study's ${what}, ${fileName(absolute)}, isn't at\n${absolute}\nor beside the study. Find it yourself?`);
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

/** The run file for the work on screen: open the case, attach its side files, set the edits, solve. */
export function workSteps(): Step[] {
  const steps = startingPoint();
  if (steps.length === 0) return steps;
  // Settings equal to the defaults are left out when the file is written.
  steps.push({ command: "solve_power_flow", args: { settings: useStudy.getState().settings } });
  return steps;
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
    const base = summary.name.replace(/\.(raw|m|stnb?)$/i, "");
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

  async exportStn(binary) {
    const summary = useWorkspace.getState().caseSummary;
    if (!summary) return false;
    const base = summary.name.replace(/\.(raw|m|stnb?)$/i, "");
    const path = await pickSavePath(`${base}.${binary ? "stnb" : "stn"}`, binary ? STNB_FILTERS : STN_FILTERS);
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
    // A run file opens the case from its file, so Records edits have to be in one first.
    const recordEdits = summary.record_edits ?? 0;
    if (recordEdits > 0) {
      const ok = await confirmDialog(
        `${summary.name} has ${recordEdits} edited value${recordEdits === 1 ? "" : "s"} from Records that aren't in a file yet. ` +
          "A run file opens its case from the file, so save the edited case first. Save it now?"
      );
      if (!ok || !(await w.saveCaseAs())) return false;
    }
    const base = summary.name.replace(/\.(raw|m|stnb?)$/i, "");
    const path = (!opts?.as && get().path) || (await pickSavePath(`${base}.stnrun`, RUN_FILE_FILTERS));
    if (!path) return false;
    set({ busy: "saving", error: null });
    try {
      await api.saveRunFile(path, workSteps());
      set({ path, dirty: false, busy: null });
      addRecent(path, "run");
      return true;
    } catch (e) {
      set({ busy: null, error: `Couldn't save the run file: ${toStannumError(e).message}` });
      return false;
    }
  },

  async openFile(given) {
    if (get().busy || useWorkspace.getState().opening) return false;
    if (!(await caseChangeOk(useWorkspace.getState().caseSummary))) return false;
    const path = given ?? (await pickFile(OPEN_FILTERS));
    if (!path) return false;
    if (RUN_EXTENSION.test(path)) {
      const report = await useRunFile.getState().run(path, { confirmed: true });
      if (report) addRecent(path, "run");
      return !!report;
    }
    return STUDY_EXTENSION.test(path)
      ? get().open(path, { confirmed: true })
      : useWorkspace.getState().openCase(path, { confirmed: true });
  },

  async open(given, opts) {
    if (get().busy) return false;
    if (!opts?.confirmed && !(await caseChangeOk(useWorkspace.getState().caseSummary))) return false;
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
    // Study files are read, never written: point at what replaces them.
    const retired =
      "Study files are no longer saved. Save as run file keeps the case, its side files, the edits and the solve settings" +
      (file.diagram ? "; save the drawing from Diagram's File menu." : ".");
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
      useContingencies.getState().setChoice({ rating: "a", baseRating: "a", buses: false, pairs: false, skipZeroImpedance: false, zeroImpedancePu: 0.0001, source: "n1", listsPath: null, monitorFromLists: false, monitorsPath: null, ...file.contingencies });
      const diagram = useDiagramStore.getState();
      if (file.diagram) {
        diagram.loadDiagramFromJSON(file.diagram);
        diagram.setFilePath(null);
      }
      lines.push(retired);
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
    // The study's edits aren't in a run file yet, so they count as unsaved.
    set({
      path: null,
      dirty: hasWork(),
      busy: null,
      notice: { tone: warning ? "warning" : "info", title: `Opened ${fileName(path)}`, lines },
    });
    addRecent(path, "study");
    return true;
  },
}));

// A change to what the run file keeps marks the work as changed. Saved
// solves, the drawing and contingency settings aren't in it.
const touch = () => {
  if (!restoring && useWorkspace.getState().caseSummary && !useStudyFile.getState().dirty) useStudyFile.setState({ dirty: true });
};
useStudy.subscribe((s, prev) => {
  if (s.entries !== prev.entries || s.settings !== prev.settings) touch();
});

// A different case (or none) ends the study; edits made in Records keep it.
let lastVersion = useWorkspace.getState().caseVersion;
useWorkspace.subscribe((s) => {
  if (s.caseVersion === lastVersion) return;
  lastVersion = s.caseVersion;
  if (s.caseVersion === s.editedVersion) return;
  if (!restoring) useStudyFile.setState({ path: null, dirty: false, notice: null, error: null });
});

onBeforeCaseChange(unsavedStudyOk);
