import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { catchUpLog, useLog } from "../../store/logStore";
import type {
  CaseNetwork,
  CaseSummary,
  ContingencyDetail,
  ContingencyProgress,
  ContingencyRow,
  ContingencySettings,
  ContingencySummary,
  DcGridsSummary,
  CoordinatesSummary,
  BusCoordinates,
  DiffRecordDetail,
  DiffRecordRow,
  DiffSummary,
  ElementDetail,
  FileFingerprint,
  RecordBook,
  RecordEdit,
  RecordTable,
  ResponseTable,
  SensitivitySummary,
  SolveReport,
  SolveSettings,
  StudyEdit,
  SourceRecord,
  TopoCase,
  TopoPath,
} from "./types";
import { decodeSolveReport } from "./wire";
import { decodeTopology, decodeView } from "./topologyWire";
import type { CoreArgs, CoreCommand, CoreError, CoreResult, RunReport, Step } from "./commands.gen";

export type CoreErrorKind = CoreError["kind"];

/** How each command reads in the log. Quiet ones only log when they fail. */
const LOG_LABELS: Record<string, { label: string; done?: string; quiet?: boolean }> = {
  open_case: { label: "Opening case", done: "Opened case" },
  close_case: { label: "Closing case", done: "Closed case" },
  solve_power_flow: { label: "Solving power flow", done: "Solved power flow" },
  calculate_sensitivities: { label: "Calculating sensitivities", done: "Calculated sensitivities" },
  export_matpower: { label: "Exporting MATPOWER case", done: "Exported MATPOWER case" },
  topology_snapshot: { label: "Analysing topology", done: "Analysed topology" },
  topology_set_merge: { label: "Rebuilding topology", done: "Rebuilt topology" },
  run_contingencies: { label: "Running contingencies", done: "Ran contingencies" },
  contingency_detail: { label: "Opening a contingency", quiet: true },
  trip_and_compare: { label: "Tripping and comparing", done: "Tripped and compared" },
  result_report: { label: "Reading a result", quiet: true },
  set_study_edits: { label: "Applying study edits", done: "Applied study edits" },
  load_diagram: { label: "Reading file", quiet: true },
  file_fingerprint: { label: "Checking the case file", quiet: true },
  load_dc_grids: { label: "Reading DC grid file", done: "Attached DC grid file" },
  clear_dc_grids: { label: "Detaching DC grid file", done: "Detached DC grid file" },
  load_coordinates: { label: "Reading bus coordinates", done: "Attached bus coordinates" },
  clear_coordinates: { label: "Detaching bus coordinates", done: "Detached bus coordinates" },
  bus_coordinates: { label: "Placing buses", quiet: true },
  diff_compare: { label: "Comparing cases", done: "Compared cases" },
  diff_compare_working: { label: "Comparing the working case with its file", done: "Compared the working case with its file" },
  save_diagram: { label: "Saving file", done: "Saved file" },
  diagram_network: { label: "Reading the case network", quiet: true },
  element_detail: { label: "Reading an element", quiet: true },
  case_records: { label: "Reading the case's records", quiet: true },
  record_table: { label: "Reading records", quiet: true },
  apply_record_edits: { label: "Applying record edits", done: "Applied record edits" },
  save_case_as: { label: "Saving case", done: "Saved case" },
  topology_shortest_path: { label: "Finding a path", quiet: true },
  topology_source: { label: "Reading the record", quiet: true },
  run_file: { label: "Running run file", done: "Ran run file" },
  save_run_file: { label: "Saving run file", done: "Saved run file" },
  study_edits: { label: "Reading study edits", quiet: true },
};

/** Arguments for a core command, left out when it takes none. */
type ArgsFor<C extends CoreCommand> = Record<string, never> extends CoreArgs<C>
  ? [args?: CoreArgs<C>]
  : [args: CoreArgs<C>];

/**
 * What a failed backend call rejects with: the core's error kind (see
 * CoreError in commands.gen.ts) and its message.
 */
export class StannumError extends Error {
  readonly kind: CoreErrorKind | "shell";
  constructor(kind: CoreErrorKind | "shell", message: string) {
    super(message);
    this.name = "StannumError";
    this.kind = kind;
  }
}

/** Is this a failure of the given kind (such as "cancelled")? */
export function isErrorKind(e: unknown, kind: CoreErrorKind): boolean {
  return e instanceof StannumError && e.kind === kind;
}

/** Whatever a backend call rejected with, as a StannumError. */
export function toStannumError(e: unknown): StannumError {
  if (e instanceof StannumError) return e;
  if (e && typeof e === "object" && "kind" in e && "message" in e) {
    const { kind, message } = e as CoreError;
    return new StannumError(kind, String(message));
  }
  return new StannumError("shell", typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
}

/**
 * Run one of the core's commands (stannum_core::commands). They all travel
 * through one Tauri command, so the core's list is the only list, and the
 * types come from its schemas (commands.gen.ts).
 */
export async function invokeCore<C extends CoreCommand>(command: C, ...[args]: ArgsFor<C>): Promise<CoreResult<C>> {
  let result: CoreResult<C>;
  try {
    result = await invoke<CoreResult<C>>("stannum_command", { command, args: args ?? {} });
  } catch (e) {
    throw toStannumError(e);
  }
  for (const watcher of watchers) watcher(command, (args ?? {}) as Record<string, unknown>);
  return result;
}

/** Told about every core command that worked, with its arguments (Record uses it). */
export type CoreCallWatcher = (command: CoreCommand, args: Record<string, unknown>) => void;
const watchers = new Set<CoreCallWatcher>();

/** Follow every core command that works; the answer stops following. */
export function watchCoreCalls(watcher: CoreCallWatcher): () => void {
  watchers.add(watcher);
  return () => watchers.delete(watcher);
}

/** Every backend call goes through here and is recorded in the log. */
async function logged<T>(command: string, run: () => Promise<T>): Promise<T> {
  const meta = LOG_LABELS[command] ?? { label: command.replace(/_/g, " "), quiet: true };
  const end = useLog.getState().begin(meta.label);
  try {
    const result = await run();
    await catchUpLog();
    end({ done: meta.done, quiet: meta.quiet });
    return result;
  } catch (e) {
    const error = toStannumError(e);
    await catchUpLog();
    // Stopping on request is not a failure.
    if (error.kind === "cancelled") end({ cancelled: true });
    else end({ error: error.message });
    throw error;
  }
}

/** A core command, logged. */
function call<C extends CoreCommand>(command: C, ...args: ArgsFor<C>): Promise<CoreResult<C>> {
  return logged(command, () => invokeCore(command, ...args));
}

/** A command the desktop shell answers itself, logged. */
function shell<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return logged(command, () => invoke<T>(command, args));
}

export const DEFAULT_SOLVE_SETTINGS: SolveSettings = {
  model: "ac",
  start: "last",
  // The same MW and Mvar on every case, whatever its base.
  tolerance_mva: 2,
  max_iterations: 10,
  enforce_q_limits: false,
  dc_model: "detailed",
  adjust_dc_taps: true,
  switch_shunts: true,
  adjust_transformer_taps: true,
};

export const CASE_FILTERS = [
  { name: "Power system case", extensions: ["raw", "RAW", "m"] },
  { name: "PSS/E RAW", extensions: ["raw", "RAW"] },
  { name: "MATPOWER", extensions: ["m"] },
];
export const DC_GRID_FILTERS = [{ name: "Stannum DC grids", extensions: ["json"] }];
export const COORDINATE_FILTERS = [{ name: "Bus coordinates (CSV)", extensions: ["csv", "CSV", "txt"] }];
export const RAW_FILTERS = [{ name: "PSS/E RAW", extensions: ["raw", "RAW"] }];
export const RUN_FILE_FILTERS = [{ name: "Stannum run file", extensions: ["stnrun"] }];
export const RUN_FILE_EXTENSION = /\.stnrun$/i;

export async function pickFile(
  filters: { name: string; extensions: string[] }[]
): Promise<string | null> {
  const result = await open({ multiple: false, filters });
  if (!result) return null;
  return typeof result === "string" ? result : (result as string[])[0] ?? null;
}

export async function pickSavePath(
  defaultPath: string,
  filters: { name: string; extensions: string[] }[]
): Promise<string | null> {
  return (await save({ defaultPath, filters })) ?? null;
}

/** Write a text export (CSV/JSON) chosen through the save dialog. */
export async function saveTextExport(
  defaultName: string,
  content: string,
  filter: { name: string; extensions: string[] }
): Promise<boolean> {
  const path = await pickSavePath(defaultName, [filter]);
  if (!path) return false;
  await shell<void>("save_diagram", { path, content });
  return true;
}

export const api = {
  openCase: (path: string): Promise<CaseSummary> => call("open_case", { path }),
  closeCase: () => call("close_case"),
  caseSummary: (): Promise<CaseSummary | null> => call("case_summary"),

  solve: async (settings: SolveSettings): Promise<SolveReport> =>
    decodeSolveReport(await call("solve_power_flow", { settings })),
  /** Stops the solve in progress; it keeps the previous results. */
  cancelSolve: () => invokeCore("cancel_solve"),
  setStudyEdits: (edits: StudyEdit[]) => call("set_study_edits", { edits }),
  loadDcGrids: (path: string): Promise<DcGridsSummary> => call("load_dc_grids", { path }),
  clearDcGrids: () => call("clear_dc_grids"),
  loadCoordinates: (path: string): Promise<CoordinatesSummary> => call("load_coordinates", { path }),
  clearCoordinates: () => call("clear_coordinates"),
  busCoordinates: (): Promise<BusCoordinates> => call("bus_coordinates"),
  sensitivities: (settings: SolveSettings): Promise<SensitivitySummary> =>
    call("calculate_sensitivities", { settings }),
  responses: (column: number, quantity: string, engineering: boolean): Promise<ResponseTable> =>
    call("sensitivity_responses", { column, quantity, engineering }),
  exportMatpower: (path: string, solved: boolean) =>
    call("export_matpower", { path, solved }),

  runContingencies: (settings: ContingencySettings): Promise<ContingencySummary> =>
    call("run_contingencies", { settings }),
  // Polled while a run is going, so they skip the log.
  cancelContingencies: () => invokeCore("cancel_contingencies"),
  contingencyProgress: (): Promise<ContingencyProgress> => invokeCore("contingency_progress"),
  contingencyRows: (offset: number, limit: number): Promise<ContingencyRow[]> =>
    call("contingency_rows", { offset, limit }),
  contingencyDetail: (rank: number): Promise<ContingencyDetail> => call("contingency_detail", { rank }),
  tripAndCompare: (element: string, limitPct: number): Promise<ContingencyDetail> => call("trip_and_compare", { element, limitPct }),
  /** A named result ("base", "study", "contingency 3", "trip line:1-2-1") as a full solve report. */
  resultReport: async (name: string): Promise<SolveReport> =>
    decodeSolveReport(await call("result_report", { name })),

  diagramNetwork: (): Promise<CaseNetwork> => call("diagram_network"),
  elementDetail: (element: string): Promise<ElementDetail> => call("element_detail", { element }),
  caseRecords: (): Promise<RecordBook> => call("case_records"),
  recordTable: (section: string): Promise<RecordTable> => call("record_table", { section }),
  applyRecordEdits: (edits: RecordEdit[]): Promise<CaseSummary> => call("apply_record_edits", { edits }),
  saveCaseAs: (path: string): Promise<CaseSummary> => call("save_case_as", { path }),

  readTextFile: (path: string) => shell<string>("load_diagram", { path }),
  writeTextFile: (path: string, content: string) => shell<void>("save_diagram", { path, content }),
  fileFingerprint: (path: string): Promise<FileFingerprint> => call("file_fingerprint", { path }),
  studyRelativePath: (study: string, casePath: string): Promise<string | null> =>
    call("study_relative_path", { study, casePath }),
  studyLocateCase: (study: string, absolute: string, relative: string | null): Promise<string | null> =>
    call("study_locate_case", { study, absolute, relative }),

  topologySnapshot: () => call("topology_snapshot").then(decodeTopology),
  /** Merge or split zero-impedance branches; `c` is the snapshot's case. */
  topologySetMerge: (enabled: boolean, c: TopoCase) =>
    call("topology_set_merge", { enabled }).then((w) => decodeView(w, c)),
  topologySource: (element: string): Promise<SourceRecord | null> => call("topology_source", { element }),
  shortestPath: (from: number, to: number): Promise<TopoPath | null> =>
    call("topology_shortest_path", { from, to }),

  /** Run a run file's lines in order; the report says how each went. */
  runFile: (path: string): Promise<RunReport> => call("run_file", { path }),
  /** Stops the run file in progress before its next line. */
  cancelRunFile: () => invokeCore("cancel_run_file"),
  saveRunFile: (path: string, steps: Step[]) => call("save_run_file", { path, steps }),
  studyEdits: (): Promise<StudyEdit[]> => call("study_edits"),

  diffCompare: (baseline: string, comparison: string): Promise<DiffSummary> =>
    call("diff_compare", { baseline, comparison }),
  diffCompareWorking: (): Promise<DiffSummary> => call("diff_compare_working"),
  diffSectionRecords: (section: string): Promise<DiffRecordRow[]> =>
    call("diff_section_records", { section }),
  diffRecordDetail: (section: string, index: number): Promise<DiffRecordDetail | null> =>
    call("diff_record_detail", { section, index }),
};
