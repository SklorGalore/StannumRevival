import type { Node, Edge } from "@xyflow/react";

// ── Node data interfaces ─────────────────────────────────────────────────────

/** Links a symbol to a record of the open case, so results can be shown on it. */
export interface CaseRef {
  kind: "bus" | "gen" | "branch" | "load" | "shunt" | "dcline";
  /** Bus number for buses, loads and shunts; table index for generators, branches and DC lines. */
  id: number;
  /** Shared element id (see lib/elements.ts). Missing in diagrams saved before it existed. */
  element?: string;
}

/** How far a label has been dragged from where it is drawn by default. */
export type LabelOffset = { x: number; y: number };

/** Clockwise turn of a symbol, in degrees. Text stays upright. */
export type Rotation = 0 | 90 | 180 | 270;

/** A connection point on a bus, at `x` px along it: from the left end of a
 *  horizontal bus, from the top of a vertical one. A vertical bus is a
 *  horizontal one turned clockwise, so its "top" side faces right and its
 *  "bottom" side faces left. */
export interface BusTap {
  id: string;
  x: number;
  side: "top" | "bottom";
}

export interface BusData extends Record<string, unknown> {
  label: string;
  /** Moves the bus's name and voltage. */
  labelOffset?: LabelOffset;
  voltageKV: number;
  orientation: "horizontal" | "vertical";
  length: number;
  /** Generated buses carry one tap per connection; hand-drawn ones use defaults. */
  taps?: BusTap[];
  /** Star point of a three-winding transformer: drawn as a small junction.
   *  Only in drawings made before the one-symbol transformer (transformer3). */
  star?: boolean;
  /** The bus a local drawing is centred on. */
  focus?: boolean;
  caseRef?: CaseRef;
}

export interface GeneratorData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  nominalMW: number;
  voltageKV: number;
  powerFactor: number;
  groundingType: "solidly" | "resistance" | "reactance" | "ungrounded";
  caseRef?: CaseRef;
  /** The element a local drawing was made around: drawn with a halo. */
  focus?: boolean;
}

export interface LoadData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  nominalMW: number;
  nominalMVAR: number;
  loadType: "static" | "motor" | "composite";
  caseRef?: CaseRef;
}

/** Fixed shunt at a bus, in MW / Mvar at 1 pu voltage. Positive Mvar is a
 *  capacitor (supplies reactive power), negative a reactor. */
export interface ShuntData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  nominalMVAR: number;
  nominalMW: number;
  caseRef?: CaseRef;
}

export interface TransformerData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  ratingMVA: number;
  primaryKV: number;
  secondaryKV: number;
  connection: "DeltaDelta" | "DeltaWye" | "WyeDelta" | "WyeWye";
  groundingPrimary: "solidly" | "resistance" | "none";
  groundingSecondary: "solidly" | "resistance" | "none";
  impedancePct: number;
  caseRef?: CaseRef;
  /** Winding connections are not in the case data: draw plain circles. */
  windingsUnknown?: boolean;
  /** The case's "to" bus is on the primary (top) side. */
  reversed?: boolean;
  outOfService?: boolean;
  /** The element a local drawing was made around: drawn with a halo. */
  focus?: boolean;
}

/** One winding of a three-winding transformer and the terminal it leaves from. */
export interface Transformer3Leg {
  handle: "x3-a" | "x3-b" | "x3-c";
  /** 1, 2 or 3, as in the case. */
  winding: number;
  /** The winding's bus. */
  bus: number;
  kv: number;
  ratingMVA: number;
  /** The winding's branch, so its flow is drawn on its wire. */
  caseRef?: CaseRef;
  outOfService?: boolean;
}

/** A three-winding transformer drawn as one symbol, standing in for the
 *  case's internal star bus and its three windings. */
export interface Transformer3Data extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  /** Empty on one drawn by hand. */
  legs: Transformer3Leg[];
  /** The case's star bus the symbol stands for. */
  star?: number;
  caseRef?: CaseRef;
  outOfService?: boolean;
  /** The element a local drawing was made around: drawn with a halo. */
  focus?: boolean;
}

export interface CircuitBreakerData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  state: "open" | "closed" | "fault";
  ratingKV: number;
  ratingKA: number;
  normallyOpen: boolean;
}

export interface DisconnectorData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  state: "open" | "closed";
  ratingKV: number;
  type: "gang-operated" | "single-pole";
}

export interface FuseData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  ratingKV: number;
  ratingA: number;
  type: "current-limiting" | "expulsion";
  blown: boolean;
}

export interface ConductorData extends Record<string, unknown> {
  label?: string;
  voltageKV: number;
  conductorType: "transmission" | "distribution" | "buswork";
  impedanceR?: number;
  impedanceX?: number;
  /** Orthogonal route, endpoints included, in flow coordinates: from the
   *  layout or the wire router. Ignored once either end moves away from where
   *  it was routed, until the router catches up. */
  route?: { x: number; y: number }[];
  /** Set once a segment has been dragged by hand: the router then keeps this
   *  shape, only moving its ends, until the wire is rerouted. */
  pinnedRoute?: boolean;
  /** Box reserved by the layout for this wire's results, used with `route`. */
  labelAt?: { x: number; y: number; width: number; height: number };
  /** The flow text dragged this far from its spot, which follows the wire. */
  labelOffset?: LabelOffset;
  caseRef?: CaseRef;
  /** The edge's source is the branch's "to" bus. */
  reversed?: boolean;
  outOfService?: boolean;
  /** The element a local drawing was made around: drawn with a halo. */
  focus?: boolean;
}

/** One branch leaving the drawn part of the case. */
export interface OffPageLink {
  /** Bus at the far end, outside the drawing. */
  bus: number;
  busName: string;
  caseRef?: CaseRef;
  /** Flow is read at the inside end, and the inside end is the "to" bus. */
  reversed?: boolean;
  outOfService?: boolean;
}

/** Marks a branch that leaves the drawn part of the case, or (with `links`)
 *  all the branches leaving one bus. */
export interface OffPageData extends Record<string, unknown> {
  label: string;
  rotation?: Rotation;
  labelOffset?: LabelOffset;
  /** Bus at the far end, outside the drawing. */
  bus: number;
  busName: string;
  voltageKV: number;
  caseRef?: CaseRef;
  reversed?: boolean;
  outOfService?: boolean;
  /** Set on a bundled marker: every branch it stands for, `bus` being the first's. */
  links?: OffPageLink[];
}

/** A two-terminal DC line: a converter at each end and the DC line between. */
export interface DcLinkData extends Record<string, unknown> {
  label: string;
  /** Scheduled MW from the rectifier. */
  scheduledMW: number;
  /** AC voltage at the top and bottom converter. */
  upperKV: number;
  lowerKV: number;
  caseRef?: CaseRef;
  /** The rectifier is at the bottom end. */
  reversed?: boolean;
  outOfService?: boolean;
  rotation?: Rotation;
  /** Two-terminal when left out. */
  dcKind?: "two_terminal" | "vsc" | "multi_terminal";
  /** The element a local drawing was made around: drawn with a halo. */
  focus?: boolean;
}

// ── Typed React Flow unions ──────────────────────────────────────────────────

export type SLDNodeType =
  | "bus"
  | "generator"
  | "load"
  | "shunt"
  | "transformer"
  | "transformer3"
  | "circuitBreaker"
  | "disconnector"
  | "fuse"
  | "offPage"
  | "dcLink";

export type BusNode = Node<BusData, "bus">;
export type GeneratorNode = Node<GeneratorData, "generator">;
export type LoadNode = Node<LoadData, "load">;
export type ShuntNode = Node<ShuntData, "shunt">;
export type TransformerNode = Node<TransformerData, "transformer">;
export type Transformer3Node = Node<Transformer3Data, "transformer3">;
export type CircuitBreakerNode = Node<CircuitBreakerData, "circuitBreaker">;
export type DisconnectorNode = Node<DisconnectorData, "disconnector">;
export type FuseNode = Node<FuseData, "fuse">;
export type OffPageNode = Node<OffPageData, "offPage">;
export type DcLinkNode = Node<DcLinkData, "dcLink">;

export type SLDNode =
  | BusNode
  | GeneratorNode
  | LoadNode
  | ShuntNode
  | TransformerNode
  | Transformer3Node
  | CircuitBreakerNode
  | DisconnectorNode
  | FuseNode
  | OffPageNode
  | DcLinkNode;

export type SLDEdge = Edge<ConductorData>;

// ── Diagram file format ──────────────────────────────────────────────────────

export interface DiagramMetadata {
  title: string;
  revision: string;
  company: string;
  description: string;
  createdAt: string;
  modifiedAt: string;
  schemaVersion: "1.0";
  /** File name of the case this diagram was generated from, if any. */
  caseName?: string;
  /** For a local drawing: the studied bus and how many hops out it reaches. */
  around?: Around;
}

/** A local drawing: `depth` hops around `bus`, or around both ends of
 *  `element` (a branch) when it is one. */
export interface Around {
  bus: number;
  depth: number;
  /** The element asked for, marked on the drawing. */
  element?: string;
}

export interface SLDDiagram {
  metadata: DiagramMetadata;
  nodes: SLDNode[];
  edges: SLDEdge[];
  viewport: { x: number; y: number; zoom: number };
}

// ── Store shape ──────────────────────────────────────────────────────────────

export interface DiagramSnapshot {
  nodes: SLDNode[];
  edges: SLDEdge[];
}

export type AlignMode =
  | "left"
  | "center"
  | "right"
  | "top"
  | "middle"
  | "bottom"
  | "distribute-x"
  | "distribute-y";

export interface DiagramStore {
  nodes: SLDNode[];
  edges: SLDEdge[];
  viewport: { x: number; y: number; zoom: number };
  metadata: DiagramMetadata;

  currentFilePath: string | null;
  isDirty: boolean;
  selectedNodeIds: string[];
  selectedEdgeIds: string[];

  paletteOpen: boolean;
  propertiesPanelOpen: boolean;
  gridVisible: boolean;
  snapToGrid: boolean;
  gridSize: number;
  /** Show the latest power flow results on symbols linked to the case. */
  showResults: boolean;
  /** Bumped to ask the canvas to fit the drawing on screen. */
  fitRequest: number;
  /** "mono" draws everything black, for printing. */
  colorMode: "voltage" | "mono";
  /** The drawing is locked against changes (see setLocked). Shared by both drawing styles. */
  locked: boolean;
  /** Undo/redo history of the drawing (nodes and edges only). */
  past: DiagramSnapshot[];
  future: DiagramSnapshot[];

  /** `dirty` is false for changes that don't alter the drawing (selection, measuring). */
  setNodes: (nodes: SLDNode[], dirty?: boolean) => void;
  setEdges: (edges: SLDEdge[], dirty?: boolean) => void;
  /** Record the drawing before an edit so it can be undone. Edits sharing a
   *  `key` within a moment (typing in one field) collapse into one step. */
  checkpoint: (key?: string) => void;
  undo: () => void;
  redo: () => void;
  copySelection: () => void;
  paste: () => void;
  duplicateSelection: () => void;
  selectAll: () => void;
  clearSelection: () => void;
  /** Select exactly these symbols, without touching undo history. */
  selectOnly: (nodeIds: string[], edgeIds: string[]) => void;
  /** Move the selected symbols; arrow keys nudge by one grid step. */
  nudge: (dx: number, dy: number) => void;
  /** Turn the selected symbols a quarter about their centres (clockwise for
   *  90); buses swap between horizontal and vertical. */
  rotateSelection: (turn: 90 | -90) => void;
  align: (how: AlignMode) => void;
  toggleColorMode: () => void;
  /** Lock the drawing: symbols and wires can still be selected and
   *  inspected, but not moved, edited, deleted or rewired. */
  setLocked: (locked: boolean) => void;
  updateNodeData: (id: string, data: Partial<SLDNode["data"]>) => void;
  updateEdgeData: (id: string, data: Partial<ConductorData>) => void;
  addNode: (node: SLDNode) => void;
  deleteSelected: () => void;
  setSelection: (nodeIds: string[], edgeIds: string[]) => void;

  newDiagram: () => void;
  loadDiagramFromJSON: (diagram: SLDDiagram) => void;
  /** Replace the drawing with a generated one (unsaved, new file). */
  replaceWithGenerated: (
    nodes: SLDNode[],
    edges: SLDEdge[],
    caseName: string,
    around?: Around
  ) => void;
  toggleResults: () => void;
  requestFit: () => void;
  setFilePath: (path: string | null) => void;
  markDirty: () => void;
  markClean: () => void;

  toggleGrid: () => void;
  toggleSnap: () => void;
  setViewport: (vp: { x: number; y: number; zoom: number }) => void;
  updateMetadata: (meta: Partial<DiagramMetadata>) => void;
}
