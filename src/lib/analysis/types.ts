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
  /** The branch or generator the warning's record describes, when known. */
  element?: string;
}

export interface CaseSummary {
  path: string;
  name: string;
  format: "matpower" | "raw";
  power: ModuleStatus<{ counts: CaseCounts; warnings: RawWarning[] }>;
  topology: ModuleStatus<{
    /** RAW revision nodal read; 0 when built from the power flow model. */
    revision: number;
    /** Built from the power flow model (MATPOWER, RAW other than revision 35). */
    derived?: boolean;
    buses: number;
    equipment: number;
    titles: string[];
  }>;
  diffable: boolean;
  /** The attached Stannum DC grid file, if any. Missing from older backends. */
  dc_grids?: DcGridsSummary | null;
}

// ---- VSC DC grids ---------------------------------------------------------

export interface DcGridsSummary {
  path: string;
  name: string;
  grids: { name: string; base_kv: number; poles: number; buses: number; lines: number; converters: number }[];
}

export interface VscGridConverterRow {
  name: string;
  ac_bus: number;
  ac_bus_name: string;
  dc_bus: number;
  in_service: boolean;
  dc_control: "slack" | "power" | "droop";
  ac_control: "reactive" | "voltage" | "reactive_limit";
  /** Injected into the AC grid at the system bus. */
  ps_mw: number;
  qs_mvar: number;
  us_pu: number;
  us_deg: number;
  /** Leaving the converter towards the AC side. */
  pc_mw: number;
  qc_mvar: number;
  uc_pu: number;
  uc_deg: number;
  current_pu: number;
  current_ka: number;
  loss_mw: number;
  /** Injected into the DC grid. */
  pdc_mw: number;
  udc_pu: number;
  udc_kv: number;
  q_min_mvar: number | null;
  q_max_mvar: number | null;
  p_limited: boolean;
  outside_limits: boolean;
}

export interface VscGridBusRow {
  number: number;
  name: string;
  v_pu: number;
  kv: number;
  injection_mw: number;
}

export interface VscGridLineRow {
  from: number;
  to: number;
  name: string;
  in_service: boolean;
  current_ka: number;
  from_mw: number;
  to_mw: number;
  loss_mw: number;
}

export interface VscGridReport {
  name: string;
  base_kv: number;
  poles: number;
  line_loss_mw: number;
  converter_loss_mw: number;
  converters: VscGridConverterRow[];
  buses: VscGridBusRow[];
  lines: VscGridLineRow[];
}

export interface VscGridsReport {
  file: string;
  converged: boolean;
  iterations: number;
  ac_iterations: number;
  last_change_mva: number;
  grids: VscGridReport[];
}

// ---- Power flow -----------------------------------------------------------

export type ModelChoice = "ac" | "dc";

export interface SolveSettings {
  model: ModelChoice;
  /** "last" starts from the last converged AC solve, with its controls where
   * they ended, and falls back to the case voltages ("case") without one. */
  start: "case" | "flat" | "last";
  /** Largest mismatch allowed at any bus, in MVA on every case's base. */
  tolerance_mva: number;
  max_iterations: number;
  enforce_q_limits: boolean;
  /** Two-terminal DC lines: converter equations, or MATPOWER's lossless pair. */
  dc_model: "detailed" | "simplified";
  /** Step DC converter taps (detailed model only). */
  adjust_dc_taps: boolean;
  /** Switch shunts in voltage control (AC only). */
  switch_shunts: boolean;
  /** Step transformer taps in voltage control (AC only). */
  adjust_transformer_taps: boolean;
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
  /** Load drawn at the solved voltage, all parts together. */
  pd: number;
  qd: number;
  /** The load split by how it varies with voltage, when any of it does. */
  load_parts?: LoadParts | null;
  area: number;
  violation: number;
}

/** A bus's load at the solved voltage by part, in MW and Mvar:
 * [constant power, constant current, constant impedance]. */
export interface LoadParts {
  p: [number, number, number];
  q: [number, number, number];
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
  /** Converter state from the detailed VSC model, when it was used. */
  vsc_detail?: VscDetail | null;
  /** This row's converter from the detailed multi-terminal model. */
  mtdc_detail?: MtdcDetail | null;
}

export type MtdcControl = "voltage" | "order" | "angle_limited" | "blocked";

/** One converter of a multi-terminal DC system, on the row linking it to the voltage converter. */
export interface MtdcDetail {
  system: string;
  control: MtdcControl;
  /** Low DC voltage switched the power orders to current orders. */
  reduced_voltage: boolean;
  /** Whether the converter is the row's sending end. */
  at_from: boolean;
  rectifier: boolean;
  current_ka: number;
  /** DC voltage across the converter, kV. */
  dc_kv: number;
  /** Firing (rectifier) or extinction (inverter) angle, degrees. */
  angle: number;
  tap: number;
}

/** The DC network of a multi-terminal system under the detailed model. */
export interface DcSystemReport {
  name: string;
  reduced_voltage: boolean;
  line_loss_mw: number;
  converter_loss_mw: number;
  converters: DcConverterRow[];
  buses: DcBusRow[];
  links: DcLinkRow[];
}

export interface DcConverterRow {
  bus: number;
  bus_name: string;
  dc_bus: number;
  control: MtdcControl;
  rectifier: boolean;
  current_ka: number;
  dc_kv: number;
  angle: number;
  tap: number;
  /** MW drawn from the AC bus; negative means delivered. */
  p_mw: number;
  /** MVAr absorbed at the AC bus. */
  q_mvar: number;
  loss_mw: number;
}

export interface DcBusRow {
  number: number;
  name: string;
  kv: number;
}

export interface DcLinkRow {
  from: number;
  to: number;
  circuit: string;
  current_ka: number;
  from_mw: number;
  to_mw: number;
  loss_mw: number;
}

/** Operating point of a VSC DC line under the detailed model. */
export interface VscDetail {
  /** DC current from the sending to the receiving end, kA. */
  current_ka: number;
  from_kv: number;
  to_kv: number;
  from_loss_mw: number;
  to_loss_mw: number;
  line_loss_mw: number;
  /** Fraction of the power order kept after MVA or current limits. */
  order_scale: number;
  from_limited: boolean;
  to_limited: boolean;
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
  /** Passes that reduced VSC output to converter MVA or current limits. */
  vsc_limit_passes?: number;
  /** Passes that switched shunts or stepped transformer taps. */
  voltage_control_passes?: number;
  /** Switched shunts that ended away from their starting value. */
  shunts_switched?: number;
  /** Transformer taps that ended away from their starting ratio. */
  taps_moved?: number;
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
  /** DC networks of the multi-terminal systems (detailed model). */
  dc_systems?: DcSystemReport[];
  /** VSC DC grids from an attached DC grid file. */
  vsc_grids?: VscGridsReport | null;
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

/** One change a study makes to the open case (see stannum-core study.rs). */
export type StudyEdit =
  | { kind: "status"; element: string; online: boolean }
  | { kind: "generation"; element: string; mw: number }
  | { kind: "load"; bus: number; mw: number; mvar: number };

// ---- Element inspector ----------------------------------------------------

/** One value from the file; text where it isn't a number (a type, "not rated"). */
export interface DetailField {
  label: string;
  value: number | string;
  unit: string;
}

/** One element as the file describes it (see stannum-core inspect.rs). */
export interface ElementDetail {
  element: string;
  kind: string;
  buses: { number: number; name: string; base_kv: number }[];
  /** In service in the file, before study edits. */
  online: boolean;
  /** Parameters, one group per winding for a three-winding transformer. */
  data: { title: string; fields: DetailField[] }[];
  limits: DetailField[];
  /** A bus's branches, generators and DC lines. */
  connected: string[];
  note: string | null;
}

// ---------------------------------------------------------------------------
// Contingency analysis (stannum-core contingency.rs)
// ---------------------------------------------------------------------------

/** Linear screen then AC of the worst, or AC for every outage. */
export type ContingencyMethod = "screen" | "ac";

export interface ContingencySettings {
  /** Element ids to take out one at a time; absent means every in-service branch and/or generator. */
  elements?: string[] | null;
  branches: boolean;
  generators: boolean;
  method: ContingencyMethod;
  ac_count: number;
  limit_pct: number;
  solve: SolveSettings;
}

export interface ContingencyOverload {
  element: string;
  from: number;
  to: number;
  pct: number;
  base_pct: number;
  mw: number;
  base_mw: number;
}

export interface ContingencyVoltage {
  bus: number;
  name: string;
  vm: number;
  base_vm: number;
  vmin: number;
  vmax: number;
  outside: number;
}

export interface ContingencyRow {
  rank: number;
  element: string;
  kind: "branch" | "generator";
  checked: "ac" | "linear";
  converged: boolean | null;
  iterations: number | null;
  worst: ContingencyOverload | null;
  overloads: number;
  screened_pct: number | null;
  note: string | null;
  worst_voltage: ContingencyVoltage | null;
  voltage_violations: number;
  cut_off_buses: number;
  lost_load_mw: number;
  lost_generation_mw: number;
  severity: number;
}

export interface ContingencySummary {
  method: ContingencyMethod | null;
  limit_pct: number;
  total: number;
  already_out: number;
  ac_checked: number;
  with_overloads: number;
  with_voltage_problems: number;
  not_converged: number;
  islanding: number;
  screen_ms: number;
  ac_ms: number;
  elapsed_ms: number;
  cancelled: boolean;
  worst: ContingencyRow | null;
}

export interface ContingencyBranchChange {
  element: string;
  from: number;
  to: number;
  mw: number;
  base_mw: number;
  pct: number | null;
  base_pct: number | null;
  tripped: boolean;
}

export interface ContingencyBusChange {
  bus: number;
  vm: number;
  base_vm: number;
  vmin: number;
  vmax: number;
}

export interface ContingencyDetail {
  row: ContingencyRow;
  overloads: ContingencyOverload[];
  voltage_problems: ContingencyVoltage[];
  branches: ContingencyBranchChange[];
  buses: ContingencyBusChange[];
  cut_off: number[];
}

export interface ContingencyProgress {
  phase: "idle" | "screening" | "ac";
  done: number;
  total: number;
}
