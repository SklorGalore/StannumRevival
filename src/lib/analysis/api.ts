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
  DiffRecordDetail,
  DiffRecordRow,
  DiffSummary,
  ElementDetail,
  FileFingerprint,
  GraphData,
  RecordBook,
  RecordEdit,
  RecordTable,
  Analysis,
  ResponseTable,
  SensitivitySummary,
  SolveReport,
  SolveSettings,
  StudyEdit,
  TopoPath,
  TopologySnapshot,
} from "./types";
import { decodeSolveReport, type WireSolveReport } from "./wire";

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
};

/**
 * Every backend call goes through here: Tauri rejects with the Rust error
 * string, normalized to Error, and each call is recorded in the log.
 */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const meta = LOG_LABELS[command] ?? { label: command.replace(/_/g, " "), quiet: true };
  const end = useLog.getState().begin(meta.label);
  try {
    const result = await invoke<T>(command, args);
    await catchUpLog();
    end({ done: meta.done, quiet: meta.quiet });
    return result;
  } catch (e) {
    const error = new Error(typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
    await catchUpLog();
    // Stopping on request is not a failure.
    if (error.message.startsWith("Solve cancelled")) end({ cancelled: true });
    else end({ error: error.message });
    throw error;
  }
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
export const RAW_FILTERS = [{ name: "PSS/E RAW", extensions: ["raw", "RAW"] }];

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
  await call<void>("save_diagram", { path, content });
  return true;
}

export const api = {
  openCase: (path: string) => call<CaseSummary>("open_case", { path }),
  closeCase: () => call<void>("close_case"),
  caseSummary: () => call<CaseSummary | null>("case_summary"),

  solve: async (settings: SolveSettings): Promise<SolveReport> =>
    decodeSolveReport(await call<WireSolveReport>("solve_power_flow", { settings })),
  /** Stops the solve in progress; it keeps the previous results. */
  cancelSolve: () => invoke<void>("cancel_solve"),
  setStudyEdits: (edits: StudyEdit[]) => call<void>("set_study_edits", { edits }),
  loadDcGrids: (path: string) => call<DcGridsSummary>("load_dc_grids", { path }),
  clearDcGrids: () => call<void>("clear_dc_grids"),
  sensitivities: (settings: SolveSettings) =>
    call<SensitivitySummary>("calculate_sensitivities", { settings }),
  responses: (column: number, quantity: string, engineering: boolean) =>
    call<ResponseTable>("sensitivity_responses", { column, quantity, engineering }),
  exportMatpower: (path: string, solved: boolean) =>
    call<void>("export_matpower", { path, solved }),

  runContingencies: (settings: ContingencySettings) =>
    call<ContingencySummary>("run_contingencies", { settings }),
  // Polled while a run is going, so they skip the log.
  cancelContingencies: () => invoke<void>("cancel_contingencies"),
  contingencyProgress: () => invoke<ContingencyProgress>("contingency_progress"),
  contingencyRows: (offset: number, limit: number) =>
    call<ContingencyRow[]>("contingency_rows", { offset, limit }),
  contingencyDetail: (rank: number) => call<ContingencyDetail>("contingency_detail", { rank }),
  tripAndCompare: (element: string, limitPct: number) => call<ContingencyDetail>("trip_and_compare", { element, limitPct }),
  /** A named result ("base", "study", "contingency 3", "trip line:1-2-1") as a full solve report. */
  resultReport: async (name: string): Promise<SolveReport> =>
    decodeSolveReport(await call<WireSolveReport>("result_report", { name })),

  diagramNetwork: () => call<CaseNetwork>("diagram_network"),
  elementDetail: (element: string) => call<ElementDetail>("element_detail", { element }),
  caseRecords: () => call<RecordBook>("case_records"),
  recordTable: (section: string) => call<RecordTable>("record_table", { section }),
  applyRecordEdits: (edits: RecordEdit[]) => call<CaseSummary>("apply_record_edits", { edits }),
  saveCaseAs: (path: string) => call<CaseSummary>("save_case_as", { path }),

  readTextFile: (path: string) => call<string>("load_diagram", { path }),
  writeTextFile: (path: string, content: string) => call<void>("save_diagram", { path, content }),
  fileFingerprint: (path: string) => call<FileFingerprint>("file_fingerprint", { path }),
  studyRelativePath: (study: string, casePath: string) =>
    call<string | null>("study_relative_path", { study, case: casePath }),
  studyLocateCase: (study: string, absolute: string, relative: string | null) =>
    call<string | null>("study_locate_case", { study, absolute, relative }),

  topologySnapshot: () => call<TopologySnapshot>("topology_snapshot"),
  topologySetMerge: (enabled: boolean) =>
    call<{ analysis: Analysis; graph: GraphData }>("topology_set_merge", { enabled }),
  shortestPath: (from: number, to: number) =>
    call<TopoPath | null>("topology_shortest_path", { from, to }),

  diffCompare: (baseline: string, comparison: string) =>
    call<DiffSummary>("diff_compare", { baseline, comparison }),
  diffCompareWorking: () => call<DiffSummary>("diff_compare_working"),
  diffSectionRecords: (section: string) =>
    call<DiffRecordRow[]>("diff_section_records", { section }),
  diffRecordDetail: (section: string, index: number) =>
    call<DiffRecordDetail | null>("diff_record_detail", { section, index }),
};
