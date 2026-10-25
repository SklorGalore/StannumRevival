// Shapes returned by the Rust commands in src-tauri/src/analysis.rs.
// Field names mirror the Rust structs (snake_case).

export interface ModuleStatus<T> {
  available: boolean;
  error: string | null;
  info: T | null;
}

export interface CaseCounts {
  base_mva: number;
  buses: number;
  generators: number;
  branches: number;
  dc_lines: number;
}

export interface RawWarning {
  line: number;
  message: string;
}

export interface CaseSummary {
  path: string;
  name: string;
  format: "matpower" | "raw";
  power: ModuleStatus<{ counts: CaseCounts; warnings: RawWarning[] }>;
  topology: ModuleStatus<{
    revision: number;
    buses: number;
    equipment: number;
    titles: string[];
  }>;
  diffable: boolean;
}

// ---- Power flow -----------------------------------------------------------

export type ModelChoice = "ac" | "dc";

export interface SolveSettings {
  model: ModelChoice;
  start: "case" | "flat";
  tolerance: number;
  max_iterations: number;
  enforce_q_limits: boolean;
  /** Two-terminal DC lines: converter equations, or MATPOWER's lossless pair. */
  dc_model: "detailed" | "simplified";
  /** Step DC converter taps (detailed model only). */
  adjust_dc_taps: boolean;
}

export interface BusRow {
  id: number;
  name: string;
  kind: "PQ" | "PV" | "Ref" | "Isolated";
  base_kv: number;
  vm: number;
  va: number;
  vmin: number;
  vmax: number;
  pd: number;
  qd: number;
  area: number;
  violation: number;
}

export interface GeneratorRow {
  index: number;
  /** Shared element id, such as "generator:101:1". */
  element: string;
  machine_id: string;
  bus: number;
  bus_name: string;
  online: boolean;
  pg: number;
  qg: number;
  qmin: number;
  qmax: number;
  vg: number;
  pmax: number;
  q_violation: number;
}

export interface BranchRow {
  index: number;
  /** Shared element id, such as "line:101:102:1". */
  element: string;
  circuit: string;
  /** 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise. */
  winding: number;
  from: number;
  to: number;
  online: boolean;
  is_transformer: boolean;
  pf: number;
  qf: number;
  pt: number;
  qt: number;
  loss_mw: number;
  rate_a: number;
  loading_pct: number | null;
}

/** A solved DC line, from its sending (rectifier) bus to its receiving (inverter) bus. */
export interface DcLineRow {
  index: number;
  /** Shared element id, such as "dcline:101:205:DC1". */
  element: string;
  kind: DcLineKind;
  from: number;
  to: number;
  from_name: string;
  to_name: string;
  online: boolean;
  /** MW taken from the AC system at the sending end. */
  pf: number;
  /** MW delivered to the AC system at the receiving end. */
  pt: number;
  /** MVAr injected at each end; negative means absorbed. */
  qf: number;
  qt: number;
  vf: number;
  vt: number;
  loss_mw: number;
  qminf: number;
  qmaxf: number;
  qmint: number;
  qmaxt: number;
  q_violation: number;
  /** Converter state from the detailed LCC model, when it was used. */
  detail?: LccDetail | null;
}

/** Operating point of a two-terminal DC line under the detailed model. */
export interface LccDetail {
  /** Which converter holds the DC current. */
  control: "rectifier" | "inverter" | "angle_limited" | "blocked";
  /** Low DC voltage switched a power order to current control. */
  reduced_voltage: boolean;
  current_ka: number;
  rectifier_kv: number;
  inverter_kv: number;
  alpha: number;
  gamma: number;
  rectifier_tap: number;
  inverter_tap: number;
  line_loss_mw: number;
  converter_loss_mw: number;
}

export interface LimitSummary {
  voltage_violations: number;
  worst_voltage: [number, number] | null;
  reactive_violations: number;
  worst_reactive: [number, number] | null;
  branch_overloads: number;
  worst_branch: [number, number, number] | null;
}

export interface SolveReport {
  status: string;
  converged: boolean;
  model: ModelChoice;
  iterations: number;
  q_limit_passes: number;
  /** Passes that stepped DC converter taps. */
  dc_tap_passes?: number;
  residual_pu: number;
  residual_mva: number;
  elapsed_ms: number;
  total_generation_mw: number;
  total_load_mw: number;
  total_losses_mw: number;
  limits: LimitSummary;
  buses: BusRow[];
  generators: GeneratorRow[];
  branches: BranchRow[];
  /** Missing from older backends. */
  dc_lines?: DcLineRow[];
}

export interface InjectionColumn {
  column: number;
  bus: number;
  bus_name: string;
  kind: "p" | "q";
}

export interface SensitivitySummary {
  model: ModelChoice;
  status: string;
  elapsed_ms: number;
  states: number;
  injections: InjectionColumn[];
  branches: number;
  quantities: { key: string; title: string }[];
  explanation: string;
  column_explanation: string;
}

export interface SensitivityResponse {
  index: number;
  label: string;
  value: number;
  offline: boolean;
}

export interface ResponseTable {
  units: string;
  title: string;
  rows: SensitivityResponse[];
}

// ---- Topology (nodal) -----------------------------------------------------

export interface TopoDiagnostic {
  severity: "error" | "warning" | "info";
  section: string;
  line: number;
  message: string;
}
export interface SourceRecord {
  line: number;
  section: string;
  fields: Record<string, string>;
  raw_lines: string[];
}
export interface TopoBus {
  number: number;
  name: string;
  base_kv: number;
  bus_type: number;
  area: number | null;
  zone: number | null;
  voltage_pu: number | null;
  source: SourceRecord;
}
export interface Equipment {
  id: string;
  kind: "line" | "transformer" | "switch";
  name: string;
  circuit: string;
  buses: number[];
  status: number;
  active_terminals: boolean[];
  source: SourceRecord;
}
export interface Injection {
  id: string;
  bus: number;
  status: number;
  p_mw: number;
  q_mvar: number;
  source: SourceRecord;
}
export interface TopoCase {
  revision: number;
  base_mva: number;
  frequency_hz: number;
  zero_impedance_threshold: number;
  titles: string[];
  encoding: string;
  buses: TopoBus[];
  equipment: Equipment[];
  loads: Injection[];
  generators: Injection[];
  diagnostics: TopoDiagnostic[];
  skipped_sections: Record<string, number>;
}
export interface Connection {
  id: string;
  equipment_id: string;
  source: string;
  target: string;
  winding: number | null;
}
export interface GraphNode {
  id: string;
  label: string;
  bus: number | null;
  equipment_id: string | null;
  base_kv: number | null;
  member_buses: number[];
}
export interface GraphData {
  nodes: GraphNode[];
  edges: Connection[];
  zero_impedance_groups: number[][];
  merge_zero_impedance: boolean;
  absorbed_equipment_ids: string[];
}
export interface Island {
  id: number;
  buses: number[];
  synthetic_nodes: string[];
}
export interface Degree {
  bus: number;
  degree: number;
  island: number;
}
export interface Analysis {
  active_buses: number;
  original_active_buses: number;
  absorbed_equipment: number;
  excluded_buses: number;
  active_equipment: number;
  excluded_equipment: number;
  synthetic_junctions: number;
  islands: Island[];
  degrees: Degree[];
  bridges: Connection[];
  articulation_buses: number[];
  synthetic_articulations: string[];
}
export interface TopoPath {
  buses: number[];
  equipment_ids: string[];
  node_ids: string[];
  connection_ids: string[];
  hops: number;
}
export interface TopologySnapshot {
  case: TopoCase;
  analysis: Analysis;
  graph: GraphData;
}

// ---- Case diff ------------------------------------------------------------

export type DiffStatus = "added" | "removed" | "changed" | "unchanged";

export interface DiffSummary {
  baseline: { name: string; revision: number };
  comparison: { name: string; revision: number };
  complete: boolean;
  differing: number;
  sections: {
    name: string;
    counts: Record<DiffStatus, number>;
  }[];
  diagnostics: {
    side: "baseline" | "comparison";
    file: string;
    section: string;
    line: number;
    message: string;
    raw_lines: string[];
  }[];
}

export interface DiffRecordRow {
  index: number;
  key: string;
  name: string;
  status: DiffStatus;
  /** Shared element id, for buses, branches, transformers, switches, loads and generators. */
  element: string | null;
}

export interface DiffRecordDetail {
  section: string;
  key: string;
  status: DiffStatus;
  fields: {
    name: string;
    before: string | null;
    after: string | null;
    status: DiffStatus;
  }[];
  before_lines: string[] | null;
  after_lines: string[] | null;
}

// ---- Diagram --------------------------------------------------------------

export interface NetworkBus {
  id: number;
  name: string;
  base_kv: number;
  area: number;
  kind: "PQ" | "PV" | "Ref" | "Isolated";
  pd: number;
  qd: number;
  gs: number;
  bs: number;
  /** Internal star point of a three-winding transformer. */
  star: boolean;
}
export interface NetworkGenerator {
  index: number;
  element: string;
  bus: number;
  online: boolean;
  pmax: number;
}
export interface NetworkBranch {
  index: number;
  element: string;
  circuit: string;
  winding: number;
  from: number;
  to: number;
  online: boolean;
  transformer: boolean;
  rate_a: number;
  tap: number;
  shift: number;
}
/** A two-terminal DC line, from its rectifier bus to its inverter bus. */
export interface NetworkDcLine {
  index: number;
  element: string;
  name: string;
  from: number;
  to: number;
  online: boolean;
  /** Scheduled MW sent from the rectifier; the power flow holds it fixed. */
  mw: number;
  /** Which kind of DC line. A multi-terminal system gives one per scheduled
   *  converter, each to the converter holding the DC voltage. Older backends
   *  leave it out, which means two-terminal. */
  kind?: DcLineKind;
}
export type DcLineKind = "two_terminal" | "vsc" | "multi_terminal";
export interface CaseNetwork {
  base_mva: number;
  buses: NetworkBus[];
  generators: NetworkGenerator[];
  branches: NetworkBranch[];
  dc_lines: NetworkDcLine[];
}
