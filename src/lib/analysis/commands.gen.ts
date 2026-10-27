// Generated from the core's command schemas (stannum_core::commands::schemas).
// Don't edit: run `pwsh scripts/update-schemas.ps1` after changing a command.

export type AnalysisWire = {
  absorbed_equipment: number;
  active_buses: number;
  active_equipment: number;
  articulation_buses: number[];
  /** By position in the connection table. */
  bridges: number[];
  degrees: DegreeColumns;
  excluded_buses: number;
  excluded_equipment: number;
  /** Each island's id and junctions; its buses come from `degrees`. */
  islands: IslandWire[];
  original_active_buses: number;
  synthetic_articulations: string[];
  synthetic_junctions: number;
};

/** A branch's flow after one outage, for drawing the change on a diagram. */
export type BranchChange = {
  base_mw: number;
  base_pct: number | null;
  element: string;
  from: number;
  mw: number;
  /** Percent of rate A, after and before; `None` when unrated. */
  pct: number | null;
  to: number;
  tripped: boolean;
};

/** BranchRow rows as columns: one array per field, each `count` long. */
export type BranchRowColumns = {
  circuit: string[];
  count: number;
  element: string[];
  from: number[];
  index: number[];
  is_transformer: boolean[];
  loading_pct: (number | null)[];
  loss_mw: number[];
  online: boolean[];
  pf: number[];
  pt: number[];
  qf: number[];
  qt: number[];
  rate_a: number[];
  to: number[];
  winding: number[];
};

export type BusChange = {
  base_vm: number;
  bus: number;
  vm: number;
  vmax: number;
  vmin: number;
};

export type BusColumns = {
  area: (number | null)[];
  base_kv: number[];
  bus_type: number[];
  line: number[];
  name: string[];
  number: number[];
  section: number[];
  voltage_pu: (number | null)[];
  zone: (number | null)[];
};

/** Where each bus goes, as columns in the case's bus order. */
export type BusCoordinates = {
  bus: number[];
  /** Null for a bus with no place. */
  latitude: (number | null)[];
  longitude: (number | null)[];
  /** "file", "estimated" or "none". */
  source: ("file" | "estimated" | "none")[];
  /** The substation the file names, if any. */
  substation: (string | null)[];
  summary: CoordinatesSummary;
};

export type BusRef = {
  base_kv: number;
  name: string;
  number: number;
};

/** BusRow rows as columns: one array per field, each `count` long. */
export type BusRowColumns = {
  area: number[];
  base_kv: number[];
  count: number;
  id: number[];
  kind: ("PQ" | "PV" | "Ref" | "Isolated")[];
  load_parts: (LoadParts | null)[];
  name: string[];
  pd: number[];
  qd: number[];
  va: number[];
  violation: number[];
  vm: number[];
  vmax: number[];
  vmin: number[];
};

export type CaseCounts = {
  base_mva: number;
  branches: number;
  buses: number;
  dc_lines: number;
  generators: number;
};

export type CaseFormat = "matpower" | "raw";

/**
 * Names one open case in commands and scripts. Ids are never reused within
 * a workspace, so a stale id is refused instead of reaching another case.
 */
export type CaseId = number;

export type CaseInfo = {
  name: string;
  revision: number;
};

export type CaseNetwork = {
  base_mva: number;
  branches: NetworkBranch[];
  buses: NetworkBus[];
  dc_lines: NetworkDcLine[];
  generators: NetworkGenerator[];
};

export type CaseSummary = {
  /** The attached bus coordinates file, if any. */
  coordinates: CoordinatesSummary | null;
  /** The attached DC grid file, if any. */
  dc_grids: DcGridsSummary | null;
  /** RAW files can be used as a comparison baseline. */
  diffable: boolean;
  format: CaseFormat;
  /** The handle commands take to name this case. */
  id: CaseId;
  name: string;
  path: string;
  power: ModuleStatusOfPowerInfo;
  /** Cells edited in Records and not yet saved to a file. */
  record_edits: number;
  topology: ModuleStatusOfTopologyInfo;
};

export type CaseWire = {
  base_mva: number;
  buses: BusColumns;
  diagnostics: Diagnostic[];
  encoding: string;
  equipment: EquipmentColumns;
  frequency_hz: number;
  generators: InjectionColumns;
  loads: InjectionColumns;
  revision: number;
  /** Section names, which the tables refer to by position. */
  sections: string[];
  skipped_sections: Record<string, number>;
  titles: string[];
  zero_impedance_threshold: number;
};

export type ColumnKind = "integer" | "number" | "text";

/** One command, as `list_commands` describes it. */
export type CommandInfo = {
  /** What it does, from its doc comment. */
  doc: string;
  name: string;
  /** Whether it takes a `case` argument. */
  takes_case: boolean;
};

/** The answer to `list_commands`. */
export type CommandList = {
  api_version: number;
  commands: CommandInfo[];
};

/**
 * One outage in full: every overload and voltage problem, and the flow and
 * voltage changes worth drawing.
 */
export type ContingencyDetail = {
  /**
   * Branches whose flow moved by at least half a MW, or that are over the
   * limit or tripped.
   */
  branches: BranchChange[];
  /**
   * AC only: buses whose voltage moved by at least 0.001 p.u., or that
   * are outside their limits.
   */
  buses: BusChange[];
  /** Buses split off by the outage. */
  cut_off: number[];
  overloads: Overload[];
  row: ContingencyRow;
  voltage_problems: VoltageProblem[];
};

/** One outage's outcome, as a row of the ranked results. */
export type ContingencyRow = {
  /** "ac" when solved in AC, "linear" when only screened. */
  checked: "ac" | "linear";
  /** AC only: whether the solve converged, and in how many iterations. */
  converged: boolean | null;
  /** Buses split off from the rest of the network, and what they carried. */
  cut_off_buses: number;
  /** The element taken out. */
  element: string;
  iterations: number | null;
  /** "branch" or "generator". */
  kind: "branch" | "generator";
  lost_generation_mw: number;
  lost_load_mw: number;
  /** AC only: why a solve failed, in a sentence. */
  note: string | null;
  /** Branches over the limit that are new or more than a point worse. */
  overloads: number;
  /** Position in the ranked list, from 1. */
  rank: number;
  /** The `worst` branch's loading from the linear screen. */
  screened_pct: number | null;
  /**
   * Sort key, higher is worse: the worst new or worse overload, or a bus
   * voltage problem scored on the same scale; infinite when the AC solve
   * failed.
   */
  severity: number;
  voltage_violations: number;
  /**
   * The most loaded branch that is over its limit and new or worse than
   * before the outage; otherwise the most loaded branch.
   */
  worst: Overload | null;
  /** AC only: the worst bus outside its limits that is new or worse. */
  worst_voltage: VoltageProblem | null;
};

/** What to run. */
export type ContingencySettings = {
  /** How many of the worst screened outages get an AC solve. */
  ac_count?: number;
  branches?: boolean;
  /**
   * Branch and generator ids to take out, one at a time. When absent,
   * every in-service branch and/or generator, as chosen below.
   */
  elements?: string[] | null;
  generators?: boolean;
  /** Loading, as a percent of rate A, above which a branch is overloaded. */
  limit_pct?: number;
  method?: Method;
  solve: SolveSettings;
};

export type ContingencySummary = {
  ac_checked: number;
  ac_ms: number;
  already_out: number;
  /** Stopped early: outages after the stop keep their screened result. */
  cancelled: boolean;
  elapsed_ms: number;
  islanding: number;
  limit_pct: number;
  method: Method | null;
  not_converged: number;
  screen_ms: number;
  /**
   * Outages run, and in-service checks skipped because the element was
   * already out.
   */
  total: number;
  with_overloads: number;
  with_voltage_problems: number;
  worst: ContingencyRow | null;
};

/** What the app shows about the attached coordinates file. */
export type CoordinatesSummary = {
  /** Buses placed from their neighbours. */
  estimated: number;
  /** Buses of the case placed by the file. */
  from_file: number;
  /** Buses with no place at all (no placed bus in their island). */
  missing: number;
  name: string;
  path: string;
  /** Lines skipped in all. */
  problem_count: number;
  /** Lines skipped, with the reason; at most 20 are listed. */
  problems: Problem[];
  /** Rows read from the file. */
  rows: number;
  /** Rows naming buses this case doesn't have. */
  unknown_buses: number;
};

export type DcBusRow = {
  kv: number;
  name: string;
  number: number;
};

export type DcConverterRow = {
  angle: number;
  /** AC bus number and name. */
  bus: number;
  bus_name: string;
  control: "voltage" | "order" | "angle_limited" | "blocked";
  current_ka: number;
  /** DC bus number. */
  dc_bus: number;
  dc_kv: number;
  loss_mw: number;
  /** MW drawn from the AC bus (negative: delivered). */
  p_mw: number;
  /** MVAr absorbed at the AC bus. */
  q_mvar: number;
  rectifier: boolean;
  tap: number;
};

export type DcGridCounts = {
  base_kv: number;
  buses: number;
  converters: number;
  lines: number;
  name: string;
  poles: number;
};

/** What the app shows about the attached DC grid file. */
export type DcGridsSummary = {
  grids: DcGridCounts[];
  name: string;
  path: string;
};

/**
 * A solved DC line row, from its sending (rectifier) bus to its receiving
 * (inverter) bus.
 */
export type DcLineRow = {
  /** Converter operating point from the detailed LCC model. */
  detail: LccDetail | null;
  /** Shared id, such as `dcline:101:205:DC1`. */
  element: string;
  from: number;
  from_name: string;
  index: number;
  /** "two_terminal", "vsc" or "multi_terminal". */
  kind: "two_terminal" | "vsc" | "multi_terminal";
  loss_mw: number;
  /** This row's converter from the detailed multi-terminal model. */
  mtdc_detail: MtdcDetail | null;
  online: boolean;
  /** MW taken from the AC system at the sending end. */
  pf: number;
  /** MW delivered to the AC system at the receiving end. */
  pt: number;
  /**
   * Largest MVAr outside either end's range; zero when within, offline,
   * or solved with the detailed model (which has no Q range).
   */
  q_violation: number;
  /** MVAr injected into the AC system at each end (negative = absorbed). */
  qf: number;
  qmaxf: number;
  qmaxt: number;
  qminf: number;
  qmint: number;
  qt: number;
  to: number;
  to_name: string;
  vf: number;
  /** Converter operating point from the detailed VSC model. */
  vsc_detail: VscDetail | null;
  vt: number;
};

export type DcLinkRow = {
  circuit: string;
  current_ka: number;
  from: number;
  from_mw: number;
  loss_mw: number;
  to: number;
  to_mw: number;
};

/**
 * How two-terminal (LCC), VSC and multi-terminal DC lines are solved when
 * the case has their data.
 */
export type DcModelChoice = "simplified" | "detailed";

/** A multi-terminal DC system's DC network under the detailed model. */
export type DcSystemReport = {
  buses: DcBusRow[];
  converter_loss_mw: number;
  converters: DcConverterRow[];
  line_loss_mw: number;
  links: DcLinkRow[];
  name: string;
  reduced_voltage: boolean;
};

export type DegreeColumns = {
  bus: number[];
  degree: number[];
  island: number[];
};

export type Diagnostic = {
  line: number;
  message: string;
  section: string;
  severity: "error" | "warning" | "info";
};

export type DiagnosticRow = {
  file: string;
  line: number;
  message: string;
  raw_lines: string[];
  section: string;
  /** "baseline" or "comparison". */
  side: "baseline" | "comparison";
};

export type DiffRecordDetail = {
  after_lines: string[] | null;
  before_lines: string[] | null;
  fields: FieldRow[];
  key: string;
  section: string;
  status: DiffStatus;
};

export type DiffRecordRow = {
  /** Shared element id (see `crate::elements`) for sections other modules show. */
  element: string | null;
  /** Position within the section, used to request details. */
  index: number;
  key: string;
  name: string;
  status: DiffStatus;
};

export type DiffStatus = "added" | "removed" | "changed" | "unchanged";

export type DiffSummary = {
  baseline: CaseInfo;
  comparison: CaseInfo;
  complete: boolean;
  diagnostics: DiagnosticRow[];
  /** Records that are added, removed or changed, across all sections. */
  differing: number;
  sections: SectionSummary[];
};

export type EdgeColumns = {
  /** By position in the equipment table. */
  equipment: number[];
  /** By position in the node table. */
  source: number[];
  target: number[];
  winding: (number | null)[];
};

/** One change to the case, by shared element id or bus number. */
export type Edit = {
  element: string;
  kind: "status";
  online: boolean;
} | {
  element: string;
  kind: "generation";
  mw: number;
} | {
  bus: number;
  kind: "load";
  mvar: number;
  mw: number;
};

export type ElementDetail = {
  /** The terminals: the bus itself, or the buses a branch or DC line joins. */
  buses: BusRef[];
  /** Elements connected at a bus: its branches, generators and DC lines. */
  connected: string[];
  /** Parameters from the file. */
  data: Group[];
  element: string;
  /** "bus", "line", "transformer", "generator", "load" or "dcline". */
  kind: string;
  /** Ratings and limits from the file. */
  limits: Field[];
  /** A caveat about how the values were read, if any. */
  note: string | null;
  /** In service in the file (before any study edits). */
  online: boolean;
};

/** One log line. */
export type Entry = {
  /** "error", "warn", "info" or "debug". */
  level: "error" | "warn" | "info" | "debug";
  message: string;
  /** Increases by one per line, so a reader can ask for what it hasn't seen. */
  seq: number;
  /** The part of the program that wrote it, such as "rs-power". */
  source: string;
  /** Milliseconds since the Unix epoch. */
  time: number;
};

export type EquipmentColumns = {
  buses: number[][];
  circuit: string[];
  id: string[];
  /** 0 line, 1 transformer, 2 switch. */
  kind: number[];
  line: number[];
  name: string[];
  /** Bit `i` set when terminal `i` is open: 0 for most equipment. */
  open_terminals: number[];
  section: number[];
  status: number[];
};

/** An error from a command: what kind it is, and what to tell the person. */
export type Error = {
  kind: ErrorKind;
  message: string;
};

/** What went wrong, in a form a client can match on. */
export type ErrorKind = "unknown_command" | "invalid_arguments" | "no_case" | "case_not_open" | "not_ready" | "not_found" | "unsupported" | "case_changed" | "cancelled" | "file" | "failed" | "internal";

export type Field = {
  label: string;
  /** "MW", "p.u.", "kV", ... or empty. */
  unit: string;
  value: FieldValue;
};

export type FieldRow = {
  after: string | null;
  /** `None` means the field is absent on that side. */
  before: string | null;
  name: string;
  status: DiffStatus;
};

/** A number, or text for values that aren't numbers (a bus type, a missing limit). */
export type FieldValue = number | string;

/**
 * What a study records about its case, to tell later whether the file
 * changed.
 */
export type Fingerprint = {
  /** Last modified, in milliseconds since 1970, when the system says. */
  modified_ms: number | null;
  /** Lowercase hex SHA-256 of the file's bytes. */
  sha256: string;
  size: number;
};

/** GeneratorRow rows as columns: one array per field, each `count` long. */
export type GeneratorRowColumns = {
  bus: number[];
  bus_name: string[];
  count: number;
  element: string[];
  index: number[];
  machine_id: string[];
  online: boolean[];
  pg: number[];
  pmax: number[];
  q_violation: number[];
  qg: number[];
  qmax: number[];
  qmin: number[];
  vg: number[];
};

export type GraphWire = {
  absorbed_equipment_ids: string[];
  edges: EdgeColumns;
  merge_zero_impedance: boolean;
  nodes: NodeColumns;
  zero_impedance_groups: number[][];
};

/** A titled list of fields, such as one winding of a transformer. */
export type Group = {
  fields: Field[];
  /** Empty when the element only has one group. */
  title: string;
};

export type InjectionColumn = {
  bus: number;
  bus_name: string;
  column: number;
  /** "p" (active) or "q" (reactive). */
  kind: "p" | "q";
};

export type InjectionColumns = {
  bus: number[];
  id: string[];
  line: number[];
  p_mw: number[];
  q_mvar: number[];
  section: number[];
  status: number[];
};

export type IslandWire = {
  id: number;
  synthetic_nodes: string[];
};

/** Operating point of a two-terminal DC line under the detailed model. */
export type LccDetail = {
  alpha: number;
  /**
   * "rectifier", "inverter" (current margin), "angle_limited" or "blocked":
   * which converter holds the DC current.
   */
  control: "rectifier" | "inverter" | "angle_limited" | "blocked";
  converter_loss_mw: number;
  current_ka: number;
  gamma: number;
  inverter_kv: number;
  inverter_tap: number;
  line_loss_mw: number;
  rectifier_kv: number;
  rectifier_tap: number;
  /** Low DC voltage switched a power order to current control. */
  reduced_voltage: boolean;
};

export type LimitSummary = {
  branch_overloads: number;
  reactive_violations: number;
  voltage_violations: number;
  /** (from, to, loading %) of the most heavily loaded rated branch. */
  worst_branch: [number, number, number] | null;
  worst_reactive: [number, number] | null;
  worst_voltage: [number, number] | null;
};

/**
 * A bus's load at the solved voltage by part, in MW and Mvar:
 * `[constant power, constant current, constant impedance]`.
 */
export type LoadParts = {
  p: [number, number, number];
  q: [number, number, number];
};

/** How each outage is checked. */
export type Method = "screen" | "ac";

export type ModelChoice = "ac" | "dc";

export type ModuleStatusOfPowerInfo = {
  available: boolean;
  error: string | null;
  info: PowerInfo | null;
};

export type ModuleStatusOfTopologyInfo = {
  available: boolean;
  error: string | null;
  info: TopologyInfo | null;
};

/**
 * One converter of a multi-terminal DC system under the detailed model, as
 * shown on the row that links it to the voltage-controlling converter.
 */
export type MtdcDetail = {
  /** Firing (rectifier) or extinction (inverter) angle, degrees. */
  angle: number;
  /** Whether the converter is the row's sending end. */
  at_from: boolean;
  /** "voltage", "order", "angle_limited" or "blocked". */
  control: "voltage" | "order" | "angle_limited" | "blocked";
  current_ka: number;
  /** DC voltage across the converter, kV. */
  dc_kv: number;
  rectifier: boolean;
  /** Low DC voltage switched the power orders to current orders. */
  reduced_voltage: boolean;
  /** The system's name. */
  system: string;
  tap: number;
};

export type NetworkBranch = {
  circuit: string;
  /** Shared id, such as `line:101:102:1`. */
  element: string;
  from: number;
  /** Index in the case's branch table (matches the power flow report). */
  index: number;
  online: boolean;
  rate_a: number;
  shift: number;
  /** Off-nominal turns ratio (1.0 when unity) and phase shift in degrees. */
  tap: number;
  to: number;
  transformer: boolean;
  /** 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise. */
  winding: number;
};

export type NetworkBus = {
  area: number;
  base_kv: number;
  bs: number;
  /** Shunt MW / MVAr at 1 p.u. */
  gs: number;
  id: number;
  kind: "PQ" | "PV" | "Ref" | "Isolated";
  name: string;
  /** Load at the bus in MW / MVAr (rs-power aggregates loads per bus). */
  pd: number;
  qd: number;
  /** The star point of a three-winding transformer, not a real bus. */
  star: boolean;
};

/**
 * A DC line, from its sending (rectifier) bus to its receiving (inverter)
 * bus. A multi-terminal system gives one per scheduled converter, each
 * running to the converter that holds the system's DC voltage.
 */
export type NetworkDcLine = {
  /** Shared id, such as `dcline:101:205:DC1`. */
  element: string;
  from: number;
  /** Row in the case's DC line table. */
  index: number;
  /** "two_terminal", "vsc" or "multi_terminal". */
  kind: "two_terminal" | "vsc" | "multi_terminal";
  /** Scheduled MW sent from the rectifier. The power flow holds it fixed. */
  mw: number;
  /** The RAW name, or a number for MATPOWER cases. */
  name: string;
  online: boolean;
  to: number;
};

export type NetworkGenerator = {
  bus: number;
  /** Shared id, such as `generator:101:1`. */
  element: string;
  /** Index in the case's generator table (matches the power flow report). */
  index: number;
  online: boolean;
  pmax: number;
};

export type NodeColumns = {
  /** The bus a bus node stands for, or null for a junction. */
  bus: (number | null)[];
  /** A junction's equipment, by position in the equipment table. */
  equipment: (number | null)[];
  /**
   * Bus nodes whose member buses are not just their own bus, as
   * `[node, members]`. Junctions have no members.
   */
  members: ([number, number[]])[];
};

/** A branch over its limit after an outage. */
export type Overload = {
  base_mw: number;
  base_pct: number;
  element: string;
  from: number;
  /** Active flow at the from end, MW, after and before. */
  mw: number;
  /** MVA as a percent of rate A, after the outage and before it. */
  pct: number;
  to: number;
};

export type Path = {
  buses: number[];
  connection_ids: string[];
  equipment_ids: string[];
  hops: number;
  node_ids: string[];
};

export type PowerInfo = {
  counts: CaseCounts;
  warnings: Warning[];
};

/** A line of the file that was skipped, and why. */
export type Problem = {
  line: number;
  message: string;
};

/** Progress of the run in hand, for a progress bar. */
export type Progress = {
  done: number;
  /** "idle", "screening" or "ac". */
  phase: "idle" | "screening" | "ac";
  total: number;
};

export type QuantityInfo = {
  key: string;
  title: string;
};

export type RecordBook = {
  /** Whether cells can be edited at all (RAW 33/35 and MATPOWER files). */
  editable: boolean;
  /** Shown above every table, such as why the model is shown instead. */
  note: string | null;
  /** RAW revision, when read from the file. */
  revision: number | null;
  sections: SectionInfo[];
  source: RecordSource;
};

export type RecordColumn = {
  /** Whether its cells can be edited (key fields and case details can't). */
  editable: boolean;
  kind: ColumnKind;
  name: string;
};

/** One cell's new value, as typed. */
export type RecordEdit = {
  /** Column position in the table. */
  column: number;
  /** Row position in the table. */
  row: number;
  /** Table id, such as "BUS" or "gen". */
  section: string;
  value: string;
};

export type RecordRow = {
  /** One value per column: a number, text, or null when absent. */
  cells: (string | number | null)[];
  /** Shared element id (see `crate::elements`), when the record is one. */
  element: string | null;
  /** Line in the file where the record starts (0 when not from the file). */
  line: number;
};

/** Where the tables come from. */
export type RecordSource = "raw" | "model";

export type RecordTable = {
  columns: RecordColumn[];
  /** Stable id, such as "BUS" for RAW or "bus" for the model. */
  id: string;
  label: string;
  /** Why some records are missing, or how to read the table. */
  note: string | null;
  rows: RecordRow[];
  /** Records in the file that couldn't be read into this table. */
  skipped: number;
};

export type Response = {
  /** Shared element id of the bus or branch, for drawing on the diagram. */
  element: string;
  /** Original row (state row or branch index), for stable "input order" sorting. */
  index: number;
  label: string;
  offline: boolean;
  value: number;
};

export type ResponseTable = {
  rows: Response[];
  title: string;
  units: string;
};

/** One record type in the list, without its rows. */
export type SectionInfo = {
  count: number;
  id: string;
  label: string;
  skipped: number;
};

export type SectionSummary = {
  counts: StatusCounts;
  name: string;
};

export type SensitivitySummary = {
  branches: number;
  column_explanation: string;
  elapsed_ms: number;
  explanation: string;
  injections: InjectionColumn[];
  model: ModelChoice;
  quantities: QuantityInfo[];
  states: number;
  status: string;
};

export type SolveReport = {
  branches: BranchRowColumns;
  /** Sent as columns (see [`wire`]); the frontend turns them back into rows. */
  buses: BusRowColumns;
  converged: boolean;
  dc_lines: DcLineRow[];
  /** DC networks of the multi-terminal systems (detailed model). */
  dc_systems: DcSystemReport[];
  /** Passes that stepped DC converter taps (detailed model). */
  dc_tap_passes: number;
  elapsed_ms: number;
  generators: GeneratorRowColumns;
  iterations: number;
  limits: LimitSummary;
  model: ModelChoice;
  q_limit_passes: number;
  residual_mva: number;
  residual_pu: number;
  /** Switched shunts that ended away from their starting susceptance. */
  shunts_switched: number;
  status: string;
  /** Transformer taps that ended away from their starting ratio. */
  taps_moved: number;
  total_generation_mw: number;
  total_load_mw: number;
  total_losses_mw: number;
  /** Passes that switched shunts or stepped transformer taps. */
  voltage_control_passes: number;
  /**
   * VSC DC grids from an attached DC grid file, solved in sequence with
   * the AC network.
   */
  vsc_grids: VscGridsReport | null;
  /** Passes that reduced VSC output to converter MVA or current limits. */
  vsc_limit_passes: number;
};

export type SolveSettings = {
  /** Step DC converter taps (detailed model only). */
  adjust_dc_taps?: boolean;
  /** Step transformer taps in voltage control (AC only). */
  adjust_transformer_taps?: boolean;
  dc_model?: DcModelChoice;
  enforce_q_limits: boolean;
  max_iterations: number;
  model: ModelChoice;
  start: StartChoice;
  /** Switch shunts in voltage control (AC only). */
  switch_shunts?: boolean;
  /**
   * Largest power mismatch allowed at any bus, in MVA. It converts to per
   * unit on each case's own base at solve time, so the same setting means
   * the same MW and Mvar on every case.
   */
  tolerance_mva: number;
};

export type SourceRecord = {
  fields: Record<string, string>;
  line: number;
  raw_lines: string[];
  section: string;
};

export type StartChoice = "case" | "flat" | "last";

export type StatusCounts = {
  added: number;
  changed: number;
  removed: number;
  unchanged: number;
};

export type TopologyInfo = {
  buses: number;
  /** Built from the power flow model (MATPOWER, RAW other than revision 35). */
  derived: boolean;
  equipment: number;
  /** RAW revision nodal read, or 0 when built from the power flow model. */
  revision: number;
  titles: string[];
};

/** The topology after a change of options: the case stays as it was. */
export type TopologyViewWire = {
  analysis: AnalysisWire;
  graph: GraphWire;
};

export type TopologyWire = {
  analysis: AnalysisWire;
  case: CaseWire;
  graph: GraphWire;
};

/** A bus outside its voltage limits after an outage. */
export type VoltageProblem = {
  base_vm: number;
  bus: number;
  name: string;
  /** Distance outside the limits, p.u. */
  outside: number;
  vm: number;
  vmax: number;
  vmin: number;
};

/** Operating point of a VSC DC line under the detailed model. */
export type VscDetail = {
  /** DC current from the sending to the receiving end, kA. */
  current_ka: number;
  /** DC voltage at the sending and receiving converters, kV. */
  from_kv: number;
  /** Whether each end's MVA or current limit reduced its output. */
  from_limited: boolean;
  /** Converter losses at each end, MW. */
  from_loss_mw: number;
  line_loss_mw: number;
  /** Fraction of the power order kept after MVA or current limits. */
  order_scale: number;
  to_kv: number;
  to_limited: boolean;
  to_loss_mw: number;
};

export type VscGridBusRow = {
  injection_mw: number;
  kv: number;
  name: string;
  number: number;
  v_pu: number;
};

export type VscGridConverterRow = {
  ac_bus: number;
  ac_bus_name: string;
  /** "reactive", "voltage" or "reactive_limit". */
  ac_control: "reactive" | "voltage" | "reactive_limit";
  current_ka: number;
  current_pu: number;
  dc_bus: number;
  /** "slack", "power" or "droop". */
  dc_control: "slack" | "power" | "droop";
  in_service: boolean;
  loss_mw: number;
  name: string;
  outside_limits: boolean;
  p_limited: boolean;
  pc_mw: number;
  pdc_mw: number;
  ps_mw: number;
  q_max_mvar: number | null;
  /** Reactive range at the operating point; None without limits. */
  q_min_mvar: number | null;
  qc_mvar: number;
  qs_mvar: number;
  uc_deg: number;
  uc_pu: number;
  udc_kv: number;
  udc_pu: number;
  us_deg: number;
  us_pu: number;
};

export type VscGridLineRow = {
  current_ka: number;
  from: number;
  from_mw: number;
  in_service: boolean;
  loss_mw: number;
  name: string;
  to: number;
  to_mw: number;
};

export type VscGridReport = {
  base_kv: number;
  buses: VscGridBusRow[];
  converter_loss_mw: number;
  converters: VscGridConverterRow[];
  line_loss_mw: number;
  lines: VscGridLineRow[];
  name: string;
  poles: number;
};

/** The sequential AC/DC solve, for the VSC grids tab. */
export type VscGridsReport = {
  ac_iterations: number;
  converged: boolean;
  file: string;
  grids: VscGridReport[];
  /** AC/DC iterations, and AC Newton steps over all of them. */
  iterations: number;
  /** Largest converter change in the last iteration, MVA. */
  last_change_mva: number;
};

export type Warning = {
  /**
   * The branch or generator whose record the warning is about, as a
   * shared element id, when the line is one of theirs.
   */
  element?: string;
  line: number;
  message: string;
};

/** Every core command, with its arguments and result. */
export interface CoreCommands {
  /** Write cell edits into the case and reopen it in place. */
  apply_record_edits: {
    args: {
      case?: CaseId | null;
      edits: RecordEdit[];
    };
    result: CaseSummary;
  };
  /**
   * Where every bus goes on the map, as columns in bus order, with the
   * source of each place ("file", "estimated" or "none").
   */
  bus_coordinates: {
    args: {
      case?: CaseId | null;
    };
    result: BusCoordinates;
  };
  /** Calculate sensitivities at the solved operating point. */
  calculate_sensitivities: {
    args: {
      case?: CaseId | null;
      settings: SolveSettings;
    };
    result: SensitivitySummary;
  };
  /** Stop the contingency run in progress. */
  cancel_contingencies: {
    args: Record<string, never>;
    result: null;
  };
  /** Stop the solve in progress; the case keeps its previous results. */
  cancel_solve: {
    args: Record<string, never>;
    result: null;
  };
  /** The case's record types and their counts. */
  case_records: {
    args: {
      case?: CaseId | null;
    };
    result: RecordBook;
  };
  /** The open case's summary, or null when none is open. */
  case_summary: {
    args: {
      case?: CaseId | null;
    };
    result: CaseSummary | null;
  };
  /** Detach the bus coordinates file. */
  clear_coordinates: {
    args: {
      case?: CaseId | null;
    };
    result: null;
  };
  /** Detach the DC grid file. */
  clear_dc_grids: {
    args: {
      case?: CaseId | null;
    };
    result: null;
  };
  /** Close the case. */
  close_case: {
    args: {
      case?: CaseId | null;
    };
    result: null;
  };
  /**
   * Every command's arguments and result, and the error, as JSON Schema
   * (2020-12), for clients that check calls or generate types.
   */
  command_schemas: {
    args: Record<string, never>;
    result: unknown;
  };
  /** One outage's flow and voltage changes, by rank. */
  contingency_detail: {
    args: {
      case?: CaseId | null;
      rank: number;
    };
    result: ContingencyDetail;
  };
  /** How far the contingency run in progress has got. */
  contingency_progress: {
    args: Record<string, never>;
    result: Progress;
  };
  /** A page of the last run's ranked results. */
  contingency_rows: {
    args: {
      case?: CaseId | null;
      limit: number;
      offset: number;
    };
    result: ContingencyRow[];
  };
  /** The last contingency run's summary, or null. */
  contingency_summary: {
    args: {
      case?: CaseId | null;
    };
    result: ContingencySummary | null;
  };
  /** The case as a drawable network, for one-line diagrams. */
  diagram_network: {
    args: {
      case?: CaseId | null;
    };
    result: CaseNetwork;
  };
  /** Compare two RAW files record by record. */
  diff_compare: {
    args: {
      baseline: string;
      comparison: string;
    };
    result: DiffSummary;
  };
  /** Compare the case's file with the case as it is being worked on. */
  diff_compare_working: {
    args: {
      case?: CaseId | null;
    };
    result: DiffSummary;
  };
  /** One record of the last comparison, field by field. */
  diff_record_detail: {
    args: {
      index: number;
      section: string;
    };
    result: DiffRecordDetail | null;
  };
  /** The last comparison's records of one type. */
  diff_section_records: {
    args: {
      section: string;
    };
    result: DiffRecordRow[];
  };
  /** One element's data and limits from the file. */
  element_detail: {
    args: {
      case?: CaseId | null;
      element: string;
    };
    result: ElementDetail;
  };
  /** Write the case, solved or as filed, as a MATPOWER file. */
  export_matpower: {
    args: {
      case?: CaseId | null;
      path: string;
      solved: boolean;
    };
    result: null;
  };
  /** Size, hash and date of a file, so a study can tell whether its case changed. */
  file_fingerprint: {
    args: {
      path: string;
    };
    result: Fingerprint;
  };
  /** The commands this version answers, and the API version. */
  list_commands: {
    args: Record<string, never>;
    result: CommandList;
  };
  /**
   * Attach a bus coordinates CSV (bus, latitude, longitude, optional
   * substation) to the case, for the map.
   */
  load_coordinates: {
    args: {
      case?: CaseId | null;
      path: string;
    };
    result: CoordinatesSummary;
  };
  /** Attach a Stannum DC grid file to the case. */
  load_dc_grids: {
    args: {
      case?: CaseId | null;
      path: string;
    };
    result: DcGridsSummary;
  };
  /** Log lines numbered above `since`. */
  log_entries: {
    args: {
      since: number;
    };
    result: Entry[];
  };
  /** Open a case file (PSS/E RAW or MATPOWER). It replaces the open case. */
  open_case: {
    args: {
      path: string;
    };
    result: CaseSummary;
  };
  /** Every record of one type, with every field. */
  record_table: {
    args: {
      case?: CaseId | null;
      section: string;
    };
    result: RecordTable;
  };
  /**
   * A result by name ("base", "study", "contingency 3", "trip line:1:2:1"),
   * as a full solve report.
   */
  result_report: {
    args: {
      case?: CaseId | null;
      name: string;
    };
    result: SolveReport;
  };
  /**
   * Run N-1 contingencies. `cancel_contingencies` stops the run and
   * `contingency_progress` follows it.
   */
  run_contingencies: {
    args: {
      case?: CaseId | null;
      settings: ContingencySettings;
    };
    result: ContingencySummary;
  };
  /** Save the case, with its edits, to a new file. */
  save_case_as: {
    args: {
      case?: CaseId | null;
      path: string;
    };
    result: CaseSummary;
  };
  /** One sensitivity column's responses at every bus. */
  sensitivity_responses: {
    args: {
      case?: CaseId | null;
      column: number;
      engineering: boolean;
      quantity: string;
    };
    result: ResponseTable;
  };
  /** Replace the study's edits; every later solve uses them. */
  set_study_edits: {
    args: {
      case?: CaseId | null;
      edits: Edit[];
    };
    result: null;
  };
  /** Solve the power flow with the study's edits. `cancel_solve` stops it. */
  solve_power_flow: {
    args: {
      case?: CaseId | null;
      settings: SolveSettings;
    };
    result: SolveReport;
  };
  /** Where a study's case is now (see `study_file::locate_case`). */
  study_locate_case: {
    args: {
      absolute: string;
      relative?: string | null;
      study: string;
    };
    result: string | null;
  };
  /** A case's path relative to a study file's folder, if there is one. */
  study_relative_path: {
    args: {
      casePath: string;
      study: string;
    };
    result: string | null;
  };
  /**
   * Merge buses joined by zero-impedance lines, or split them again. The
   * answer is the new analysis and graph in the same form as `topology_snapshot`.
   */
  topology_set_merge: {
    args: {
      case?: CaseId | null;
      enabled: boolean;
    };
    result: TopologyViewWire;
  };
  /** The shortest path between two buses, or null. */
  topology_shortest_path: {
    args: {
      case?: CaseId | null;
      from: number;
      to: number;
    };
    result: Path | null;
  };
  /**
   * Islands, bridges and articulation buses, with the case's graph, as
   * compact columns (see `topology_wire`). Source fields are left out:
   * `topology_source` sends one element's.
   */
  topology_snapshot: {
    args: {
      case?: CaseId | null;
    };
    result: TopologyWire;
  };
  /** The record an element was read from: its fields and RAW lines, or null. */
  topology_source: {
    args: {
      case?: CaseId | null;
      element: string;
    };
    result: SourceRecord | null;
  };
  /** Take one element out of the last AC solve and solve again. */
  trip_and_compare: {
    args: {
      case?: CaseId | null;
      element: string;
      limitPct: number;
    };
    result: ContingencyDetail;
  };
}

export type CoreCommand = keyof CoreCommands;
export type CoreArgs<C extends CoreCommand> = CoreCommands[C]["args"];
export type CoreResult<C extends CoreCommand> = CoreCommands[C]["result"];

/** What every core command rejects with. */
export type CoreError = Error;

export const CORE_API_VERSION = 2;
