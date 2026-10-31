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

/** One area's net interchange after a solve. */
export type AreaRow = {
  /** Whether the solve held this area's interchange. */
  controlled: boolean;
  /** Desired net export in MW, when the case's area data gives one. */
  desired_mw: number | null;
  /** Interchange minus desired, in MW, when there is a desired value. */
  error_mw: number | null;
  /** Net export in MW (negative is import). */
  interchange_mw: number;
  name: string;
  number: number;
  slack_bus: number | null;
  slack_bus_name: string;
  /** How far the solve moved the area slack generation, in MW. */
  slack_change_mw: number;
  tolerance_mw: number;
};

/** A branch's flow after one outage, for drawing the change on a diagram. */
export type BranchChange = {
  base_mw: number;
  base_pct: number | null;
  element: string;
  from: number;
  mw: number;
  /**
   * Percent of the run's rating after, and of its base rating before;
   * `None` when unrated.
   */
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
  shift_deg: number[];
  tap: number[];
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
  /**
   * "file" (the coordinates file), "case" (the case's own substation
   * data), "estimated" or "none".
   */
  source: ("file" | "case" | "estimated" | "none")[];
  /** The substation the file or the case names, if any. */
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

export type CaseFormat = "matpower" | "raw" | "stn";

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
  /**
   * Where the bus places for the map come from: the attached coordinates
   * file, the case's own substation data, or both. Null when neither
   * places any bus.
   */
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

/** How many events a category gives, for showing before a run. */
export type CategoryCount = {
  category: string;
  count: number;
  /** What it takes out, such as "Transformer". */
  label: string;
};

/** One planning event category's share of a run. */
export type CategorySummary = {
  category: string;
  islanding: number;
  not_converged: number;
  total: number;
  with_overloads: number;
  /**
   * Events with any overload, voltage problem or failed solve, each
   * counted once.
   */
  with_problems: number;
  with_voltage_problems: number;
};

export type ColumnKind = "integer" | "number" | "text";

/** One command, as `list_commands` describes it. */
export type CommandInfo = {
  /**
   * Whether it changes the case or writes a file, so Record writes it
   * into a run file.
   */
  action: boolean;
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
  /** Every monitored interface's flow. */
  interfaces: InterfaceFlow[];
  overloads: Overload[];
  row: ContingencyRow;
  voltage_problems: VoltageProblem[];
};

/** The worst limit of one contingency. */
export type ContingencyLimit = {
  contingency: string;
  factor: number;
  fcitc_mw: number;
  monitored: string;
};

/** One outage's outcome, as a row of the ranked results. */
export type ContingencyRow = {
  /** The planning event category, such as "P1.2"; none in an N-1 run. */
  category: string | null;
  /** "ac" when solved in AC, "linear" when only screened. */
  checked: "ac" | "linear";
  /** AC only: whether the solve converged, and in how many iterations. */
  converged: boolean | null;
  /** Buses split off from the rest of the network, and what they carried. */
  cut_off_buses: number;
  /** What the event is, in words, when the element alone doesn't say. */
  description: string | null;
  /** The element taken out; empty for the base case (P0). */
  element: string;
  /**
   * Monitored interfaces over their limit that are new or worse, and
   * the worst of them.
   */
  interface_violations: number;
  iterations: number | null;
  /**
   * "branch", "generator", "shunt", "dc_line", "bus", "pair" for two
   * elements out together, "group" for more, or "base" for P0.
   */
  kind: "branch" | "generator" | "shunt" | "dc_line" | "bus" | "pair" | "group" | "base";
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
  worst_interface: InterfaceFlow | null;
  /** AC only: the worst bus outside its limits that is new or worse. */
  worst_voltage: VoltageProblem | null;
};

/** ContingencyRow rows as columns: one array per field, each `count` long. */
export type ContingencyRowColumns = {
  category: (string | null)[];
  checked: ("ac" | "linear")[];
  converged: (boolean | null)[];
  count: number;
  cut_off_buses: number[];
  description: (string | null)[];
  element: string[];
  interface_violations: number[];
  iterations: (number | null)[];
  kind: ("branch" | "generator" | "shunt" | "dc_line" | "bus" | "pair" | "group" | "base")[];
  lost_generation_mw: number[];
  lost_load_mw: number[];
  note: (string | null)[];
  overloads: number[];
  rank: number[];
  screened_pct: (number | null)[];
  severity: number[];
  voltage_violations: number[];
  worst: (Overload | null)[];
  worst_interface: (InterfaceFlow | null)[];
  worst_voltage: (VoltageProblem | null)[];
};

/** What to run. */
export type ContingencySettings = {
  /** How many of the worst screened outages get an AC solve. */
  ac_count?: number;
  /**
   * The rating for the base case: its loadings (the "before" figures)
   * and the P0 event.
   */
  base_rating?: Rating;
  branches?: boolean;
  /**
   * Take out each bus: every branch and DC line connected to it, which
   * cuts off its loads, generators and shunts.
   */
  buses?: boolean;
  /**
   * Ids to take out, one at a time. A bus id (bus:101) takes out the
   * bus, and ids joined by "&" (line:1:2:1&line:2:3:1) go out together.
   * When absent, every in-service branch, generator and/or bus, as
   * chosen below.
   */
  elements?: string[] | null;
  generators?: boolean;
  /**
   * Loading, as a percent of the rating, above which a branch is
   * overloaded.
   */
  limit_pct?: number;
  /**
   * Run the contingencies of this lists file (`.stnl`): its
   * `contingency` records and `contingency_set` rules. Equipment the
   * case lacks is reported in the summary's notes and skipped. Takes
   * the place of `elements`, `branches`, `generators`, `buses` and
   * `pairs`; `planning` is unused.
   */
  lists?: string | null;
  method?: Method;
  /**
   * Check only what the `monitor` records of this lists file (`.stnl`)
   * name: branch flows and bus voltages with their own limits, and
   * interfaces. Every rated branch and every bus when absent.
   */
  monitors?: string | null;
  /**
   * N-2: take the chosen branches and generators out two at a time,
   * every pair once, instead of one at a time. Buses stay one at a time.
   */
  pairs?: boolean;
  /**
   * Run these NERC TPL-001 planning events instead of every branch and
   * generator; `elements`, `branches` and `generators` are then unused.
   */
  planning?: PlanningSettings | null;
  /** The rating loadings are measured against after an outage. */
  rating?: Rating;
  /**
   * Report every branch, interface and bus outside its limits after the
   * outage, also those that already were before it and got no worse.
   */
  report_existing?: boolean;
  /**
   * Leave out zero-impedance lines: lines with no resistance and a
   * reactance at or below `zero_impedance_pu`, as PSS/E treats them
   * (THRSHZ). Applies to N-1 and N-2, also to listed single elements;
   * a bus outage still opens them.
   */
  skip_zero_impedance?: boolean;
  solve?: SolveSettings;
  /**
   * How many percentage points of loading a branch or interface already
   * over its limit before the outage must gain to be reported. 1 by
   * default.
   */
  worse_by_pct?: number;
  /**
   * How many per unit a bus already outside its voltage limits before
   * the outage must move further out to be reported. 0.001 by default.
   */
  worse_by_pu?: number;
  /**
   * The zero-impedance threshold in per unit; 0.0001 by default, as
   * PSS/E's THRSHZ.
   */
  zero_impedance_pu?: number;
};

export type ContingencySummary = {
  ac_checked: number;
  ac_ms: number;
  already_out: number;
  base_rating: Rating;
  /** Stopped early: outages after the stop keep their screened result. */
  cancelled: boolean;
  /**
   * Planning event runs: outages and problems in each category, in
   * category order. Empty in an N-1 run.
   */
  categories: CategorySummary[];
  elapsed_ms: number;
  islanding: number;
  limit_pct: number;
  method: Method | null;
  /** Whether a lists file's monitors chose what was checked. */
  monitored: boolean;
  not_converged: number;
  /** Lists file runs: what was skipped and why, in sentences. */
  notes: string[];
  /**
   * The ratings loadings were measured against, after outages and in
   * the base case.
   */
  rating: Rating;
  screen_ms: number;
  /**
   * Outages run, and in-service checks skipped because the element was
   * already out.
   */
  total: number;
  /** Outages that take a monitored interface over its limit. */
  with_interface_problems: number;
  with_overloads: number;
  with_voltage_problems: number;
  worst: ContingencyRow | null;
};

/** What the app shows about where the case's bus places come from. */
export type CoordinatesSummary = {
  /** Substations in the case's own data with a place and buses. */
  case_substations: number;
  /** Buses placed from their neighbours. */
  estimated: number;
  /**
   * How those were placed: "lengths" (from the lines' lengths, scaled by
   * the lines whose ends are placed), "reactance" (the same, from line
   * reactance where the case gives no lengths) or "neighbours" (at the
   * average of placed neighbours, with too few placed lines to scale).
   */
  estimated_from: "lengths" | "reactance" | "neighbours";
  /** Buses placed by the case's own substation data (and not by the file). */
  from_case: number;
  /** Buses of the case placed by the file. */
  from_file: number;
  /** Buses with no place at all (no placed bus in their island). */
  missing: number;
  /** The attached file's name, else the case's. */
  name: string;
  /**
   * The attached coordinates file; null when the places come only from
   * the case's own substation data.
   */
  path: string | null;
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

/**
 * Versions and platform for a problem report. It holds nothing from the
 * case: no file names, element names or values.
 */
export type Diagnostics = {
  /** The command set's version (API_VERSION in the core). */
  api_version: number;
  /** The processor ("x86_64", "aarch64"). */
  arch: string;
  /** stannum-core's version. */
  core: string;
  /** Whether this is a debug build. */
  debug_build: boolean;
  /** Internal errors (panics) since the program started. */
  internal_errors: number;
  /** nodal's version (topology). */
  nodal: string;
  /** The operating system ("windows", "macos", "linux"). */
  os: string;
  /** rs-power's version (power flow). */
  rs_power: string;
  /** Processor threads available to solves. */
  threads: number;
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
  /** The element's kind, as `element_kinds` lists them. */
  kind: string;
  /** Ratings and limits from the file. */
  limits: Field[];
  /** A caveat about how the values were read, if any. */
  note: string | null;
  /** In service in the file (before any study edits). */
  online: boolean;
};

/** One kind of element: how its id is written and where it comes from. */
export type ElementKind = {
  /**
   * How many bus numbers follow the kind: 0 when the id is a name, and
   * for a transformer 2, or 3 for a three-winding one.
   */
  buses: number;
  /** The id's parts after the kind, such as `<bus>:<bus>:<circuit>`. */
  format: string;
  /** Where the element inspector reads it. */
  inspector: Source;
  /** The id's first part, such as "line". */
  kind: string;
  /** What people call it, such as "Line". */
  label: string;
  /** The MATPOWER table its rows are in; null when MATPOWER has none. */
  matpower_table: string | null;
  /** Whether a study edit or a contingency can take it out. */
  outage: boolean;
  /** The RAW sections its records are in; empty when no record is one. */
  raw_sections: string[];
  /** Whether a solve report has a row for it. */
  solved: boolean;
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

/** One FACTS device after a solve. */
export type FactsRow = {
  /** The shunt element ended at an end of its range. */
  at_limit: boolean;
  /** Sending-end bus, where the shunt element connects. */
  bus: number;
  bus_name: string;
  /**
   * The shunt element was held at zero (a generator holds its bus, or
   * the solve was DC).
   */
  held: boolean;
  name: string;
  /** Bus whose voltage the shunt element holds. */
  regulated_bus: number;
  series_mvar: number;
  /** Power the series element sends to the terminal-end bus. */
  series_mw: number;
  /** Largest shunt output at the setpoint (SHMX × VSET), in Mvar. */
  shunt_max_mvar: number;
  /** Reactive power the shunt element injects, in Mvar. */
  shunt_mvar: number;
  /** Terminal-end bus of the series element; none for a STATCOM. */
  terminal_bus: number | null;
  /** Solved voltage at the regulated bus, in per unit. */
  vm: number;
  /** Voltage setpoint in per unit. */
  vset: number;
};

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

/**
 * How a Records column filters: by number, by text, or by picking one of
 * a few values.
 */
export type FieldType = "number" | "text" | "category";

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

/**
 * Area interchange control: what counts as an area's net export when its
 * slack generation holds it at the desired value.
 */
export type InterchangeChoice = "off" | "tie_lines" | "tie_lines_and_loads";

/** A flowgate: branch flows summed with weights, as a `.stnl` interface. */
export type Interface = {
  /**
   * Branches as `[element, weight]`; flow measured at the from end of
   * the branch row.
   */
  elements: ([string, number])[];
  /** The interface's limit in MW, the same in both directions. */
  limit_mw: number;
  name: string;
};

/** An interface's summed flow after an outage, from a lists file monitor. */
export type InterfaceFlow = {
  base_mw: number;
  base_pct: number | null;
  /** Its limit; none when the interface has no limit. */
  limit_mw: number | null;
  /** Percent of the limit that counts as a violation. */
  limit_pct: number;
  /** Weighted sum of its branch flows, MW, after and before. */
  mw: number;
  name: string;
  /** |MW| as a percent of the limit, after and before. */
  pct: number | null;
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

/**
 * How each AC pass finds the voltages. Every method stops at the same
 * mismatch tolerance, so they reach the same answer at different speeds.
 */
export type MethodChoice = "auto" | "newton" | "fixed_slope" | "fast_decoupled_xb" | "fast_decoupled_bx";

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

/** Which branches and interfaces to watch. */
export type MonitorSettings = {
  /**
   * Every rated branch in service (lines and transformers), narrowed by
   * `subsystem` and `kv_min`.
   */
  branches?: boolean;
  /**
   * Branches watched as well as (or, with `branches` off, instead of)
   * the ones above, by id.
   */
  elements?: string[];
  /** Flowgates: weighted sums of branch flows with their own limit. */
  interfaces?: Interface[];
  /** Only branches with a terminal bus at or above this voltage, kV. */
  kv_min?: number;
  /** Only branches with a terminal bus in this subsystem. */
  subsystem?: Subsystem | null;
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
  /** Set when the branch is a RAW system switching device. */
  switching?: NetworkSwitch | null;
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
  /**
   * Owner from the RAW bus record; 0 when the case has no owners
   * (MATPOWER).
   */
  owner: number;
  /** Load at the bus in MW / MVAr (rs-power aggregates loads per bus). */
  pd: number;
  qd: number;
  /** The star point of a three-winding transformer, not a real bus. */
  star: boolean;
  /** Loss zone. */
  zone: number;
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

/** A system switching device drawn on a one-line. */
export type NetworkSwitch = {
  /** "breaker", "disconnector" or "connector" (PSS/E's generic type). */
  kind: "breaker" | "disconnector" | "connector";
  /** The device's name; may be empty. */
  name: string;
  /** Its normal status is open. */
  normally_open: boolean;
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

export type Operator = "eq" | "neq" | "contains" | "gt" | "gte" | "lt" | "lte" | "between" | "missing" | "present";

/** A branch over its limit after an outage. */
export type Overload = {
  base_mw: number;
  base_pct: number;
  element: string;
  from: number;
  /**
   * The loading, in percent, above which this branch counts as over:
   * the run's limit, or its monitor's.
   */
  limit_pct: number;
  /** Active flow at the from end, MW, after and before. */
  mw: number;
  /**
   * MVA as a percent of the run's rating after the outage, and of its
   * base rating before it.
   */
  pct: number;
  to: number;
};

/** One page of a table. */
export type PageOfBranchRow = {
  /** Where `find` is among the matched rows, if it is there. */
  found: number | null;
  /** Rows passing the search and rules (and the kept row). */
  matched: number;
  /** Where this page starts among the matched rows. */
  offset: number;
  /** Each row's position in the whole table, in the order sent. */
  positions: number[];
  rows: BranchRowColumns;
  /** Rows in the whole table. */
  total: number;
};

/** One page of a table. */
export type PageOfBusRow = {
  /** Where `find` is among the matched rows, if it is there. */
  found: number | null;
  /** Rows passing the search and rules (and the kept row). */
  matched: number;
  /** Where this page starts among the matched rows. */
  offset: number;
  /** Each row's position in the whole table, in the order sent. */
  positions: number[];
  rows: BusRowColumns;
  /** Rows in the whole table. */
  total: number;
};

/** One page of a table. */
export type PageOfContingencyRow = {
  /** Where `find` is among the matched rows, if it is there. */
  found: number | null;
  /** Rows passing the search and rules (and the kept row). */
  matched: number;
  /** Where this page starts among the matched rows. */
  offset: number;
  /** Each row's position in the whole table, in the order sent. */
  positions: number[];
  rows: ContingencyRowColumns;
  /** Rows in the whole table. */
  total: number;
};

/** One page of a table. */
export type PageOfGeneratorRow = {
  /** Where `find` is among the matched rows, if it is there. */
  found: number | null;
  /** Rows passing the search and rules (and the kept row). */
  matched: number;
  /** Where this page starts among the matched rows. */
  offset: number;
  /** Each row's position in the whole table, in the order sent. */
  positions: number[];
  rows: GeneratorRowColumns;
  /** Rows in the whole table. */
  total: number;
};

/** One page of a table. */
export type PageOfRecordRow = {
  /** Where `find` is among the matched rows, if it is there. */
  found: number | null;
  /** Rows passing the search and rules (and the kept row). */
  matched: number;
  /** Where this page starts among the matched rows. */
  offset: number;
  /** Each row's position in the whole table, in the order sent. */
  positions: number[];
  rows: RecordRowColumns;
  /** Rows in the whole table. */
  total: number;
};

/** Which rows of a table to send, in what order. */
export type PageQuery = {
  /** And, when there are any, passing at least one of these. */
  any?: Rule[];
  /**
   * An element id whose position in the sorted, filtered rows is sent
   * back as `found`, so the app can scroll to it.
   */
  find?: string | null;
  /**
   * An element id listed even when the search or rules would hide it, so
   * the picked row stays in view.
   */
  keep?: string | null;
  /** Rows to send: 200 when left out, at most 5,000. */
  limit?: number | null;
  /** Rows to skip in the sorted, filtered order. */
  offset?: number;
  /** Keep rows passing every rule. */
  rules?: Rule[];
  /** Keep rows where any field's text contains this, ignoring case. */
  search?: string;
  /** Sort by one field. Left out, rows keep the table's own order. */
  sort?: Sort | null;
};

/**
 * How far the source and sink can move before a generator or load runs
 * out of room.
 */
export type Participation = {
  /**
   * MW the side can move in total: room to Pmax (source) or to Pmin
   * (sink) for generator scaling, load to drop for a load source; `None`
   * when unbounded (a sink of loads).
   */
  available_mw: number | null;
  /** Buses taking part. */
  buses: number;
  /** Generators (or loads) taking part. */
  units: number;
};

export type Path = {
  buses: number[];
  connection_ids: string[];
  equipment_ids: string[];
  hops: number;
  node_ids: string[];
};

/** Which planning events to generate. */
export type PlanningSettings = {
  /**
   * Only elements with a terminal bus in one of these areas; every area
   * when empty.
   */
  areas?: number[];
  /**
   * Categories or groups of them, such as "P1" (every P1 event) or
   * "P1.4". Every category Stannum generates when absent.
   */
  categories?: string[];
  /**
   * Only branches, shunts and DC lines with a terminal bus at or above
   * this voltage, kV. A bus with no base voltage counts as above.
   * Generators are not filtered by voltage: most connect below it
   * through a step-up transformer.
   */
  kv_min?: number;
};

export type PowerInfo = {
  counts: CaseCounts;
  /** The RAW revision the file was read as; null for MATPOWER. */
  revision: number | null;
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

/** Which of a branch's three ratings a loading is measured against. */
export type Rating = "a" | "b" | "c";

/** What an export wrote, and what it couldn't keep. */
export type RawExport = {
  /**
   * What changed on the way, such as fields the revision has no place
   * for. Empty when the file holds everything the case had.
   */
  notes: string[];
  /** Records written, in every section. */
  records: number;
  /** The RAW revision written: 33 or 35. */
  revision: number;
  /**
   * "records" when the file's own records were written, "model" when the
   * power flow model was (MATPOWER cases, other RAW revisions).
   */
  source: "records" | "model";
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

/** A page of one record type: its columns, and the rows asked for. */
export type RecordPage = {
  columns: RecordColumn[];
  id: string;
  label: string;
  note: string | null;
  page: PageOfRecordRow;
  skipped: number;
};

/** RecordRow rows as columns: one array per field, each `count` long. */
export type RecordRowColumns = {
  cells: ((string | number | null)[])[];
  count: number;
  element: (string | null)[];
  line: number[];
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
  /** Sent as columns, like every big table. */
  rows: RecordRowColumns;
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

/**
 * One filter rule: a field compared with a value. The same rules as the
 * app's filter chips (`src/lib/filters.ts`).
 */
export type Rule = {
  field: string;
  operator: Operator;
  upper?: string | null;
  value?: string;
};

/** What running a run file did. */
export type RunReport = {
  cancelled: boolean;
  failed: number;
  /** Steps that ran, in order (the ones after a failure or a cancel don't). */
  outcomes: StepOutcome[];
  path: string;
  seconds: number;
  /** Steps in the file. */
  steps: number;
};

/** A case built into Stannum. */
export type SampleCase = {
  /** How many buses it has, counting the star bus of a three-winding transformer. */
  buses: number;
  description: string;
  /** The case file's name. */
  file_name: string;
  /** "matpower" or "raw". */
  format: string;
  /** Whether it comes with bus coordinates for the map. */
  has_coordinates: boolean;
  /** What `open_sample` takes. */
  id: string;
  title: string;
};

/** How the source and sink share out the transfer. */
export type Scaling = "generation" | "reserve" | "load";

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
  /** The area interchange control the solve used (off for DC). */
  area_interchange: InterchangeChoice;
  /**
   * Every area's net interchange (AC only): areas from the case's area
   * data first, then the others found on buses.
   */
  areas: AreaRow[];
  branches: BranchRowColumns;
  /** Sent as columns (see [`Table`]); the frontend turns them back into rows. */
  buses: BusRowColumns;
  converged: boolean;
  dc_lines: DcLineRow[];
  /** DC networks of the multi-terminal systems (detailed model). */
  dc_systems: DcSystemReport[];
  /** Passes that stepped DC converter taps (detailed model). */
  dc_tap_passes: number;
  /** How many of `iterations` were decoupled iterations. */
  decoupled_iterations: number;
  elapsed_ms: number;
  /** FACTS devices in service, in file order (RAW only). */
  facts: FactsRow[];
  /** Passes that moved phase shifters. */
  flow_control_passes: number;
  generators: GeneratorRowColumns;
  /** Passes that moved area slack generation. */
  interchange_passes: number;
  /** Iterations in all passes, Newton and decoupled together. */
  iterations: number;
  limits: LimitSummary;
  /** The AC method asked for. */
  method: MethodChoice;
  model: ModelChoice;
  /** Passes in which the automatic method finished with Newton. */
  newton_fallbacks: number;
  /** Phase shifters in active power flow control in the case. */
  phase_shifters: number;
  /** Whether the solve locked the phase shifters at their angles (AC only). */
  phase_shifters_locked: boolean;
  /** Phase shifters that ended away from their starting angle. */
  phase_shifters_moved: number;
  q_limit_passes: number;
  residual_mva: number;
  residual_pu: number;
  /**
   * Whether the solve locked the switched shunts, and the transformer
   * taps, where they were instead of moving them (AC only).
   */
  shunts_locked: boolean;
  /** Switched shunts that ended away from their starting susceptance. */
  shunts_switched: number;
  status: string;
  taps_locked: boolean;
  /**
   * Transformer taps that ended away from their starting ratio, in
   * voltage control (COD 1), reactive flow control (COD 2) or moved by a
   * DC line converter (COD 4, with the DC converter taps).
   */
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

/**
 * How to solve. Fields left out take their defaults: AC with the automatic
 * method from the last solution, to 2 MVA, with reactive limits held and
 * every control on.
 */
export type SolveSettings = {
  /**
   * Step DC converter taps, and the transformers that some converters
   * step instead (COD 4); detailed model only.
   */
  adjust_dc_taps?: boolean;
  /**
   * Move phase shifters in active power flow control (AC only). When
   * false they are locked, like taps.
   */
  adjust_phase_shifters?: boolean;
  /**
   * Step transformer taps in voltage control (COD 1) and reactive flow
   * control (COD 2) (AC only). When false they are locked, like shunts.
   */
  adjust_transformer_taps?: boolean;
  /**
   * Hold each area's net interchange at its desired value with the
   * area slack generation (AC only; RAW area data). Off by default.
   */
  area_interchange?: InterchangeChoice;
  dc_model?: DcModelChoice;
  /**
   * Decoupled iterations per pass (fixed-slope and fast decoupled, and
   * the decoupled stage of the automatic method).
   */
  decoupled_iterations?: number;
  /**
   * Hold generators at their reactive limits (and regulate remote buses).
   * On by default, as real cases are solved in PSS/E and PowerWorld.
   */
  enforce_q_limits?: boolean;
  /** Newton iterations per pass. */
  max_iterations?: number;
  /** AC solution method. */
  method?: MethodChoice;
  model?: ModelChoice;
  start?: StartChoice;
  /**
   * Switch shunts in voltage control (AC only). When false they are
   * locked: they stay where they are, which is where the last solve left
   * them when starting from it, and at their case values otherwise.
   */
  switch_shunts?: boolean;
  /**
   * Largest power mismatch allowed at any bus, in MVA. It converts to per
   * unit on each case's own base at solve time, so the same setting means
   * the same MW and Mvar on every case.
   */
  tolerance_mva?: number;
};

export type Sort = {
  descending?: boolean;
  field: string;
};

/** Where the element inspector reads an element from. */
export type Source = "model" | "topology" | "record";

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

/**
 * One command of a run file, with its arguments as JSON, named as the
 * command takes them (camelCase).
 */
export type Step = {
  args?: Record<string, unknown>;
  command: string;
  /** The line it starts on, from 1. */
  line?: number;
};

/** How a step went. */
export type StepOutcome = {
  command: string;
  /** Why it failed; absent when it worked. */
  error: Error | null;
  line: number;
  /** What it answered, when results were asked for. */
  result: unknown;
  seconds: number;
};

/** What converting a case to the Stannum format kept or changed. */
export type StnExport = {
  /** Anything worth knowing: conversions, data kept as RAW lines. */
  notes: string[];
  /** Records written, in every table. */
  records: number;
};

/**
 * A set of buses, as a `.stnl` subsystem: listed areas, zones, owners and
 * buses combine as a union (every bus when all are empty), then the kV
 * filters narrow it.
 */
export type Subsystem = {
  areas?: number[];
  buses?: number[];
  /** Only buses at or below this base voltage, kV. */
  kv_max?: number | null;
  /** Only buses at or above this base voltage, kV. */
  kv_min?: number | null;
  owners?: number[];
  zones?: number[];
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

/**
 * Which outages to study the transfer under. The base case is always
 * studied too.
 */
export type TransferContingencies = {
  branches?: boolean;
  /**
   * Ids to take out, one at a time, as for contingency analysis (ids
   * joined by "&" go out together). When absent, every in-service
   * branch and/or generator, as chosen below.
   */
  elements?: string[] | null;
  generators?: boolean;
  /** Only branch outages with a terminal bus at or above this voltage, kV. */
  kv_min?: number;
  /** Leave out zero-impedance lines (PSS/E THRSHZ, 0.0001 p.u.). */
  skip_zero_impedance?: boolean;
};

/** Base case PTDF of one monitored element. */
export type TransferFactor = {
  flow_mw: number;
  from: number;
  /** Base rating limit, MW; 0 when unrated. */
  limit_mw: number;
  monitored: string;
  ptdf: number;
  to: number;
  winding: number;
};

/**
 * One limit the transfer runs into: a monitored element, in the base case
 * or after a contingency.
 */
export type TransferLimit = {
  /** What is out, "" for the base case. */
  contingency: string;
  /** Distribution factor: PTDF in the base case, OTDF after an outage. */
  factor: number;
  /**
   * The transfer that brings the element to its limit, MW. Negative
   * when it is already past it in the direction the transfer pushes.
   */
  fcitc_mw: number;
  /** Flow before the transfer (after the contingency), MW, from end. */
  flow_mw: number;
  /** End buses of a branch; 0 for an interface. */
  from: number;
  limit_mw: number;
  /** The branch id, or `interface:<name>`. */
  monitored: string;
  /** Base case PTDF. */
  ptdf: number;
  to: number;
  /** 1-3 for a leg of a three-winding transformer. */
  winding: number;
};

export type TransferResults = {
  /** Each contingency's lowest limit, lowest first; the base case is "". */
  contingencies: ContingencyLimit[];
  /** Base case PTDFs at or above the cutoff, largest first. */
  factors: TransferFactor[];
  /**
   * Each monitored element's lowest limit over the base case and every
   * contingency, lowest first.
   */
  limits: TransferLimit[];
  /**
   * Elements already past their limit before any transfer (where it
   * pushes them further), each at its worst, lowest FCITC first. Empty
   * when `include_past_limits` puts them in `limits`.
   */
  past_limits: TransferLimit[];
  summary: TransferSummary;
};

/** A transfer limit study. */
export type TransferSettings = {
  base_rating?: Rating;
  /** Outages to study; `None` studies the base case only. */
  contingencies?: TransferContingencies | null;
  /**
   * Elements whose distribution factor is below this (as a fraction,
   * 0.05 = 5%) in size don't limit the transfer.
   */
  cutoff?: number;
  /**
   * Let elements already past their limit before any transfer set the
   * FCITC (negative). Off by default: they are listed apart in
   * `past_limits`, and the FCITC is the transfer that first takes an
   * element within its limit past it.
   */
  include_past_limits?: boolean;
  /** Percent of the rating that counts as the limit. */
  limit_pct?: number;
  /**
   * A lists file (`.stnl`). Its contingencies and contingency sets,
   * when it has any, are the outages studied instead of
   * `contingencies`; its flow and interface monitors, when it has any,
   * are what is watched instead of `monitor`.
   */
  lists?: string | null;
  /** The most rows kept in each of the results' lists. */
  max_rows?: number;
  monitor?: MonitorSettings;
  /** Rating after an outage, and in the base case (`base_rating`). */
  rating?: Rating;
  scaling?: Scaling;
  /** Where it goes. */
  sink?: Subsystem;
  /** Where the power comes from. */
  source?: Subsystem;
  /**
   * The lists file's `transfer` record to run: its source, sink,
   * scaling, cutoff, ratings and limit replace the ones above. The
   * file's only transfer when absent.
   */
  transfer?: string | null;
};

export type TransferSummary = {
  base_rating: Rating;
  cancelled: boolean;
  /**
   * Contingencies studied (the base case not counted), and those left
   * out because they split the network.
   */
  contingencies: number;
  cutoff: number;
  elapsed_ms: number;
  /**
   * The first change of total transfer capability from network limits,
   * MW; `None` when no monitored element reaches the cutoff.
   */
  fcitc_mw: number | null;
  /**
   * The most the source and sink can move: the smaller of their
   * available MW.
   */
  generation_limit_mw: number | null;
  islanding: number;
  limit_pct: number;
  /** "network", "generation" or "none". */
  limited_by: "network" | "generation" | "none";
  limiting: TransferLimit | null;
  /** Monitored branch rows and interfaces. */
  monitored: number;
  /** Things worth knowing about the setup, in words. */
  notes: string[];
  rating: Rating;
  scaling: Scaling;
  sink: Participation;
  source: Participation;
  /** The transfer limit: the smaller of the two above. */
  transfer_limit_mw: number | null;
};

/** What [`ContingencyResults::write_violations_csv`] wrote. */
export type ViolationsExport = {
  /** Outages with at least one line. */
  outages: number;
  /**
   * Lines written, one per violation (and one per outage whose AC solve
   * failed), not counting the header.
   */
  rows: number;
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
      settings?: SolveSettings | null;
    };
    result: SensitivitySummary;
  };
  /** Stop the contingency run in progress. */
  cancel_contingencies: {
    args: Record<string, never>;
    result: null;
  };
  /**
   * Stop the run file in progress before its next line, and the solve or
   * contingency run it is waiting on.
   */
  cancel_run_file: {
    args: Record<string, never>;
    result: null;
  };
  /** Stop the solve in progress; the case keeps its previous results. */
  cancel_solve: {
    args: Record<string, never>;
    result: null;
  };
  /** Stop the transfer limit study in progress. */
  cancel_transfer_limits: {
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
  /**
   * A page of the last run's results, sorted, searched and filtered in
   * the core. Left unsorted, the rows keep their rank order.
   */
  contingency_page: {
    args: {
      case?: CaseId | null;
      query?: PageQuery | null;
    };
    result: PageOfContingencyRow;
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
    result: ContingencyRowColumns;
  };
  /** The last contingency run's summary, or null. */
  contingency_summary: {
    args: {
      case?: CaseId | null;
    };
    result: ContingencySummary | null;
  };
  /**
   * Versions and platform for a problem report, and how many internal
   * errors there have been. Nothing from the case.
   */
  diagnostics: {
    args: Record<string, never>;
    result: Diagnostics;
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
  /**
   * Every kind of element id: its format, the RAW section and MATPOWER
   * table it comes from, and where the inspector reads it.
   */
  element_kinds: {
    args: Record<string, never>;
    result: ElementKind[];
  };
  /**
   * Write every violation of the last run to a CSV file, one line each:
   * branch overloads, bus voltages and interfaces, and a line for each
   * outage whose AC solve failed.
   */
  export_contingency_violations: {
    args: {
      case?: CaseId | null;
      path: string;
    };
    result: ViolationsExport;
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
  /**
   * Write the case, with its Records and study edits, as a complete PSS/E
   * RAW file of revision 33 or 35. The answer lists anything the file
   * couldn't keep.
   */
  export_raw: {
    args: {
      case?: CaseId | null;
      path: string;
      revision: number;
    };
    result: RawExport;
  };
  /**
   * Write the case, with its Records and study edits, as a Stannum case
   * file (format preview): binary when `path` ends in `.stnb`, else text
   * (`.stn`). The answer lists anything converted or kept as RAW lines.
   */
  export_stn: {
    args: {
      case?: CaseId | null;
      path: string;
    };
    result: StnExport;
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
  /**
   * Write a built-in sample case (and its bus coordinates, if it has
   * any) into `dir`, or a `stannum-samples` folder in the temp folder,
   * and open it. It replaces the open case.
   */
  open_sample: {
    args: {
      dir?: string | null;
      id: string;
    };
    result: CaseSummary;
  };
  /**
   * How many NERC TPL-001 planning events each category gives on the
   * open case, before running them with `run_contingencies`. Settings
   * left out take their defaults: every category Stannum generates, at
   * 100 kV and above.
   */
  planning_event_counts: {
    args: {
      case?: CaseId | null;
      settings?: PlanningSettings | null;
    };
    result: CategoryCount[];
  };
  /**
   * How each column of one record type filters: by number, by text, or
   * by picking one of its few values.
   */
  record_fields: {
    args: {
      case?: CaseId | null;
      section: string;
    };
    result: FieldType[];
  };
  /**
   * A page of one record type, sorted, searched and filtered in the core
   * (fields are column positions, "line" and "element").
   */
  record_page: {
    args: {
      case?: CaseId | null;
      query?: PageQuery | null;
      section: string;
    };
    result: RecordPage;
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
   * The distinct values of one column of a record type, sorted, missing
   * last: the choices its filter offers. At most `limit` (500 when left out).
   */
  record_values: {
    args: {
      case?: CaseId | null;
      field: string;
      limit?: number | null;
      section: string;
    };
    result: unknown[];
  };
  /**
   * A page of a result's branches. `busRules` keep the branches with an
   * end at a bus passing every rule.
   */
  result_branches: {
    args: {
      busRules?: Rule[] | null;
      case?: CaseId | null;
      name: string;
      query?: PageQuery | null;
    };
    result: PageOfBranchRow;
  };
  /**
   * The distinct values of a bus filter field in a result, sorted,
   * missing last. At most `limit` (500 when left out).
   */
  result_bus_values: {
    args: {
      case?: CaseId | null;
      field: string;
      limit?: number | null;
      name: string;
    };
    result: unknown[];
  };
  /**
   * A page of a result's buses, sorted, searched and filtered in the
   * core. `name` is a result name, or "latest" for the latest solve. `busRules` keep the buses passing every rule (the Power flow
   * filter chips: area, base_kv, kind, number, name, vm).
   */
  result_buses: {
    args: {
      busRules?: Rule[] | null;
      case?: CaseId | null;
      name: string;
      query?: PageQuery | null;
    };
    result: PageOfBusRow;
  };
  /**
   * A page of a result's generators. `busRules` keep the generators at a
   * bus passing every rule.
   */
  result_generators: {
    args: {
      busRules?: Rule[] | null;
      case?: CaseId | null;
      name: string;
      query?: PageQuery | null;
    };
    result: PageOfGeneratorRow;
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
   * Run N-1 contingencies, or with `settings.planning` the NERC TPL-001
   * planning events. `cancel_contingencies` stops the run and
   * `contingency_progress` follows it. Settings left out take their
   * defaults: every branch and generator, screened, then the worst in AC.
   */
  run_contingencies: {
    args: {
      case?: CaseId | null;
      settings?: ContingencySettings | null;
    };
    result: ContingencySummary;
  };
  /**
   * Run a run file (`.stnrun`): commands one per line, in order (see
   * `run_file`). Every line is checked first. The run stops at the first
   * command that fails unless `keepGoing`; `results` keeps each answer.
   */
  run_file: {
    args: {
      keepGoing?: boolean | null;
      path: string;
      results?: boolean | null;
    };
    result: RunReport;
  };
  /**
   * Transfer limits (DFAX and FCITC): how much power can move from the
   * source subsystem to the sink before a monitored branch or interface
   * reaches its limit, in the base case and after each contingency.
   * Linear factors on top of the AC solve. `cancel_transfer_limits` stops
   * the study and `transfer_progress` follows it.
   */
  run_transfer_limits: {
    args: {
      case?: CaseId | null;
      settings: TransferSettings;
    };
    result: TransferResults;
  };
  /** The small cases built into Stannum, to open with `open_sample`. */
  sample_cases: {
    args: Record<string, never>;
    result: SampleCase[];
  };
  /** Save the case, with its edits, to a new file. */
  save_case_as: {
    args: {
      case?: CaseId | null;
      path: string;
    };
    result: CaseSummary;
  };
  /**
   * Write commands as a run file, such as the ones Record kept. Paths in
   * the run file's folder are written relative to it, and settings at
   * their defaults are left out.
   */
  save_run_file: {
    args: {
      path: string;
      steps: Step[];
    };
    result: null;
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
  /**
   * Solve the power flow with the study's edits. `cancel_solve` stops it.
   * Settings left out take their defaults.
   */
  solve_power_flow: {
    args: {
      case?: CaseId | null;
      settings?: SolveSettings | null;
    };
    result: SolveReport;
  };
  /** The study's edits, as `set_study_edits` last set them. */
  study_edits: {
    args: {
      case?: CaseId | null;
    };
    result: Edit[];
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
  /** How far the transfer limit study in progress has got. */
  transfer_progress: {
    args: Record<string, never>;
    result: Progress;
  };
  /**
   * Take one element out of the last AC solve and solve again. Loadings
   * are measured against `rating` after the trip and `base_rating`
   * before it (both rate A when left out).
   */
  trip_and_compare: {
    args: {
      baseRating?: Rating | null;
      case?: CaseId | null;
      element: string;
      limitPct: number;
      rating?: Rating | null;
    };
    result: ContingencyDetail;
  };
}

export type CoreCommand = keyof CoreCommands;
export type CoreArgs<C extends CoreCommand> = CoreCommands[C]["args"];
export type CoreResult<C extends CoreCommand> = CoreCommands[C]["result"];

/** Commands that change the case or write a file: the ones Record keeps. */
export const ACTION_COMMANDS: readonly CoreCommand[] = [
  "apply_record_edits",
  "calculate_sensitivities",
  "clear_coordinates",
  "clear_dc_grids",
  "close_case",
  "diff_compare",
  "diff_compare_working",
  "export_matpower",
  "export_raw",
  "export_stn",
  "load_coordinates",
  "load_dc_grids",
  "open_case",
  "open_sample",
  "run_contingencies",
  "run_transfer_limits",
  "save_case_as",
  "set_study_edits",
  "solve_power_flow",
  "topology_set_merge",
  "trip_and_compare",
];

/** What every core command rejects with. */
export type CoreError = Error;

/** Every kind of element id (stannum_core::elements::KINDS). */
export const ELEMENT_KINDS = [
  {
    "buses": 1,
    "format": "<bus>",
    "inspector": "model",
    "kind": "bus",
    "label": "Bus",
    "matpower_table": "bus",
    "outage": false,
    "raw_sections": [
      "BUS"
    ],
    "solved": true
  },
  {
    "buses": 2,
    "format": "<bus>:<bus>:<circuit>",
    "inspector": "model",
    "kind": "line",
    "label": "Line",
    "matpower_table": "branch",
    "outage": true,
    "raw_sections": [
      "BRANCH"
    ],
    "solved": true
  },
  {
    "buses": 2,
    "format": "<bus>:<bus>[:<bus>]:<circuit>",
    "inspector": "model",
    "kind": "transformer",
    "label": "Transformer",
    "matpower_table": "branch",
    "outage": true,
    "raw_sections": [
      "TRANSFORMER"
    ],
    "solved": true
  },
  {
    "buses": 2,
    "format": "<bus>:<bus>:<circuit>",
    "inspector": "model",
    "kind": "switch",
    "label": "Switch",
    "matpower_table": null,
    "outage": true,
    "raw_sections": [
      "SYSTEM SWITCHING DEVICE"
    ],
    "solved": true
  },
  {
    "buses": 1,
    "format": "<bus>:<machine>",
    "inspector": "model",
    "kind": "generator",
    "label": "Generator",
    "matpower_table": "gen",
    "outage": true,
    "raw_sections": [
      "GENERATOR"
    ],
    "solved": true
  },
  {
    "buses": 1,
    "format": "<bus>:<id>",
    "inspector": "model",
    "kind": "load",
    "label": "Load",
    "matpower_table": null,
    "outage": false,
    "raw_sections": [
      "LOAD"
    ],
    "solved": false
  },
  {
    "buses": 1,
    "format": "<bus>:<id>",
    "inspector": "record",
    "kind": "shunt",
    "label": "Fixed shunt",
    "matpower_table": null,
    "outage": false,
    "raw_sections": [
      "FIXED SHUNT"
    ],
    "solved": false
  },
  {
    "buses": 1,
    "format": "<bus>:<id>",
    "inspector": "record",
    "kind": "switched-shunt",
    "label": "Switched shunt",
    "matpower_table": null,
    "outage": false,
    "raw_sections": [
      "SWITCHED SHUNT"
    ],
    "solved": false
  },
  {
    "buses": 1,
    "format": "<bus>:<id>",
    "inspector": "record",
    "kind": "machine",
    "label": "Induction machine",
    "matpower_table": null,
    "outage": false,
    "raw_sections": [
      "INDUCTION MACHINE"
    ],
    "solved": false
  },
  {
    "buses": 2,
    "format": "<bus>:<bus>:<name>",
    "inspector": "model",
    "kind": "dcline",
    "label": "DC line",
    "matpower_table": "dcline",
    "outage": false,
    "raw_sections": [
      "TWO-TERMINAL DC",
      "VSC DC LINE"
    ],
    "solved": true
  },
  {
    "buses": 0,
    "format": "<name>",
    "inspector": "record",
    "kind": "facts",
    "label": "FACTS device",
    "matpower_table": null,
    "outage": false,
    "raw_sections": [
      "FACTS DEVICE"
    ],
    "solved": false
  }
] as const satisfies readonly ElementKind[];

export const CORE_API_VERSION = 3;
