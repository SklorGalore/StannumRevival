import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
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
  TopoPath,
  TopologySnapshot,
} from "./types";

/** Tauri rejects with the Rust error string; normalize to Error. */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw new Error(typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
  }
}

export const DEFAULT_SOLVE_SETTINGS: SolveSettings = {
  model: "ac",
  start: "case",
  tolerance: 1e-8,
  max_iterations: 10,
  enforce_q_limits: false,
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
