import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { catchUpLog, useLog } from "../../store/logStore";
import type {
  CaseNetwork,
  CaseSummary,
  DiffRecordDetail,
  DiffRecordRow,
  DiffSummary,
  GraphData,
  Analysis,
  ResponseTable,
  SensitivitySummary,
  SolveReport,
  SolveSettings,
  StudyEdit,
  TopoPath,
  TopologySnapshot,
} from "./types";

/** How each command reads in the log. Quiet ones only log when they fail. */
const LOG_LABELS: Record<string, { label: string; done?: string; quiet?: boolean }> = {
  open_case: { label: "Opening case", done: "Opened case" },
  close_case: { label: "Closing case", done: "Closed case" },
  solve_power_flow: { label: "Solving power flow", done: "Solved power flow" },
  calculate_sensitivities: { label: "Calculating sensitivities", done: "Calculated sensitivities" },
  export_matpower: { label: "Exporting MATPOWER case", done: "Exported MATPOWER case" },
  topology_snapshot: { label: "Analysing topology", done: "Analysed topology" },
  topology_set_merge: { label: "Rebuilding topology", done: "Rebuilt topology" },
  set_study_edits: { label: "Applying study edits", done: "Applied study edits" },
  diff_compare: { label: "Comparing cases", done: "Compared cases" },
  save_diagram: { label: "Saving file", done: "Saved file" },
  diagram_network: { label: "Reading the case network", quiet: true },
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
    end({ error: error.message });
    throw error;
  }
}

export const DEFAULT_SOLVE_SETTINGS: SolveSettings = {
  model: "ac",
  start: "case",
  // 2 MVA on a 100 MVA base.
  tolerance: 0.02,
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

  solve: (settings: SolveSettings) => call<SolveReport>("solve_power_flow", { settings }),
  setStudyEdits: (edits: StudyEdit[]) => call<void>("set_study_edits", { edits }),
  sensitivities: (settings: SolveSettings) =>
    call<SensitivitySummary>("calculate_sensitivities", { settings }),
  responses: (column: number, quantity: string, engineering: boolean) =>
    call<ResponseTable>("sensitivity_responses", { column, quantity, engineering }),
  exportMatpower: (path: string, solved: boolean) =>
    call<void>("export_matpower", { path, solved }),

  diagramNetwork: () => call<CaseNetwork>("diagram_network"),

  topologySnapshot: () => call<TopologySnapshot>("topology_snapshot"),
  topologySetMerge: (enabled: boolean) =>
    call<{ analysis: Analysis; graph: GraphData }>("topology_set_merge", { enabled }),
  shortestPath: (from: number, to: number) =>
    call<TopoPath | null>("topology_shortest_path", { from, to }),

  diffCompare: (baseline: string, comparison: string) =>
    call<DiffSummary>("diff_compare", { baseline, comparison }),
  diffSectionRecords: (section: string) =>
    call<DiffRecordRow[]>("diff_section_records", { section }),
  diffRecordDetail: (section: string, index: number) =>
    call<DiffRecordDetail | null>("diff_record_detail", { section, index }),
};
