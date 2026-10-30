// The shapes the app works with. Almost all are the core's own types,
// generated from its command schemas into commands.gen.ts (run
// `pwsh scripts/update-schemas.ps1` after changing a command), so they can't
// drift from Rust. Written here by hand: tables the app turns from columns
// back into rows (SolveReport, RecordTable), and the topology shapes
// topologyWire.ts decodes from the compact snapshot.
import type * as G from "./commands.gen";

/** One row of a table the core sends as columns (see `wire.ts`). */
export type Row<C> = { [K in Exclude<keyof C, "count">]: C[K] extends readonly (infer T)[] ? T : never };

export interface ModuleStatus<T> {
  available: boolean;
  error: string | null;
  info: T | null;
}

export type CaseCounts = G.CaseCounts;

export type RawWarning = G.Warning;

export type CaseSummary = G.CaseSummary;

export type CoordinatesProblem = G.Problem;

export type CoordinatesSummary = G.CoordinatesSummary;

export type BusCoordinates = G.BusCoordinates;

// ---- VSC DC grids ---------------------------------------------------------

export type DcGridsSummary = G.DcGridsSummary;

export type VscGridConverterRow = G.VscGridConverterRow;

export type VscGridBusRow = G.VscGridBusRow;

export type VscGridLineRow = G.VscGridLineRow;

export type VscGridReport = G.VscGridReport;

export type VscGridsReport = G.VscGridsReport;

// ---- Power flow -----------------------------------------------------------

export type ModelChoice = G.ModelChoice;

/** Every setting given: the core fills in what a call leaves out, the app always sends all. */
export type SolveSettings = Required<G.SolveSettings>;

export type BusRow = Row<G.BusRowColumns>;

export type LoadParts = G.LoadParts;

export type GeneratorRow = Row<G.GeneratorRowColumns>;

export type BranchRow = Row<G.BranchRowColumns>;

export type DcLineRow = G.DcLineRow;

export type MtdcControl = G.MtdcDetail["control"];

export type MtdcDetail = G.MtdcDetail;

export type DcSystemReport = G.DcSystemReport;

export type AreaRow = G.AreaRow;

export type FactsRow = G.FactsRow;

export type InterchangeChoice = G.InterchangeChoice;

export type DcConverterRow = G.DcConverterRow;

export type DcBusRow = G.DcBusRow;

export type DcLinkRow = G.DcLinkRow;

export type VscDetail = G.VscDetail;

export type LccDetail = G.LccDetail;

export type LimitSummary = G.LimitSummary;

/** The solve report with its tables as rows (decoded in `wire.ts`). */
export type SolveReport = Omit<G.SolveReport, "buses" | "generators" | "branches"> & {
  buses: BusRow[];
  generators: GeneratorRow[];
  branches: BranchRow[];
};

export type InjectionColumn = G.InjectionColumn;

export type SensitivitySummary = G.SensitivitySummary;

export type SensitivityResponse = G.Response;

export type ResponseTable = G.ResponseTable;

// ---- Topology (nodal) -----------------------------------------------------

export interface TopoDiagnostic {
  severity: "error" | "warning" | "info";
  section: string;
  line: number;
  message: string;
}
/** Where a record was read: its line (0 when not from a RAW file) and section. */
export interface SourceRef {
  line: number;
  section: string;
}
export type SourceRecord = G.SourceRecord;
export interface TopoBus {
  number: number;
  name: string;
  base_kv: number;
  bus_type: number;
  area: number | null;
  zone: number | null;
  voltage_pu: number | null;
  source: SourceRef;
}
export interface Equipment {
  id: string;
  kind: "line" | "transformer" | "switch";
  name: string;
  circuit: string;
  buses: number[];
  status: number;
  active_terminals: boolean[];
  source: SourceRef;
}
export interface Injection {
  id: string;
  bus: number;
  status: number;
  p_mw: number;
  q_mvar: number;
  source: SourceRef;
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

export type DiffStatus = G.DiffStatus;

export type DiffSummary = G.DiffSummary;

export type DiffRecordRow = G.DiffRecordRow;

export type DiffRecordDetail = G.DiffRecordDetail;

// ---- Diagram --------------------------------------------------------------

export type NetworkBus = G.NetworkBus;
export type NetworkGenerator = G.NetworkGenerator;
export type NetworkBranch = G.NetworkBranch;
export type NetworkDcLine = G.NetworkDcLine;
export type DcLineKind = G.DcLineRow["kind"];
export type CaseNetwork = G.CaseNetwork;

export type StudyEdit = G.Edit;

// ---- Element inspector ----------------------------------------------------

export type DetailField = G.Field;

export type ElementDetail = G.ElementDetail;

// ---------------------------------------------------------------------------
// Contingency analysis (stannum-core contingency.rs)
// ---------------------------------------------------------------------------

export type ContingencyMethod = G.Method;

export type ContingencySettings = G.ContingencySettings;

export type ContingencyOverload = G.Overload;

export type ContingencyVoltage = G.VoltageProblem;

export type ContingencyRow = G.ContingencyRow;

export type ContingencySummary = G.ContingencySummary;

export type ContingencyBranchChange = G.BranchChange;

export type ContingencyBusChange = G.BusChange;

export type ContingencyDetail = G.ContingencyDetail;

export type ContingencyProgress = G.Progress;

/** NERC TPL-001 planning events to run instead of N-1 (stannum-core planning.rs). */
export type PlanningSettings = G.PlanningSettings;

export type CategoryCount = G.CategoryCount;

export type CategorySummary = G.CategorySummary;

export type RecordSource = G.RecordSource;
export type RecordColumnKind = G.ColumnKind;

export type RecordSectionInfo = G.SectionInfo;

export type RecordBook = G.RecordBook;

export type RecordColumn = G.RecordColumn;

export type RecordEdit = G.RecordEdit;

export type CaseRecord = Row<G.RecordRowColumns>;

/** A record table with its rows decoded from columns. */
export type RecordTable = Omit<G.RecordTable, "rows"> & { rows: CaseRecord[] };

export type FileFingerprint = G.Fingerprint;

// ---- Paged tables (paging.rs) ----------------------------------------------
export type PageQuery = G.PageQuery;
export type PageRule = G.Rule;
export type FieldType = G.FieldType;

/** A page of rows decoded from columns, with each row's position in the whole table. */
export interface Page<T> {
  total: number;
  matched: number;
  offset: number;
  positions: number[];
  rows: T[];
  found: number | null;
}

/** A record type's columns and one page of its records. */
export type RecordPage = Omit<G.RecordPage, "page"> & { page: Page<CaseRecord> };
