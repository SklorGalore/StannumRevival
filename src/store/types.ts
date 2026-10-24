import type { Node, Edge } from "@xyflow/react";

// ── Node data interfaces ─────────────────────────────────────────────────────

/** Links a symbol to a record of the open case, so results can be shown on it. */
export interface CaseRef {
  kind: "bus" | "gen" | "branch" | "load" | "shunt";
  /** Bus number for buses, loads and shunts; table index for generators and branches. */
  id: number;
}

/** A connection point on a bus, at `x` px from its left end. */
export interface BusTap {
  id: string;
  x: number;
  side: "top" | "bottom";
}

export interface BusData extends Record<string, unknown> {
  label: string;
  voltageKV: number;
  orientation: "horizontal" | "vertical";
  length: number;
  /** Generated buses carry one tap per connection; hand-drawn ones use defaults. */
  taps?: BusTap[];
  /** Star point of a three-winding transformer: drawn as a small junction. */
  star?: boolean;
  /** The bus a local drawing is centred on. */
  focus?: boolean;
  caseRef?: CaseRef;
}

export interface GeneratorData extends Record<string, unknown> {
  label: string;
  nominalMW: number;
  voltageKV: number;
  powerFactor: number;
  groundingType: "solidly" | "resistance" | "reactance" | "ungrounded";
  caseRef?: CaseRef;
}

export interface LoadData extends Record<string, unknown> {
  label: string;
  nominalMW: number;
  nominalMVAR: number;
  loadType: "static" | "motor" | "composite";
  caseRef?: CaseRef;
}

/** Fixed shunt at a bus, in MW / Mvar at 1 pu voltage. Positive Mvar is a
 *  capacitor (supplies reactive power), negative a reactor. */
export interface ShuntData extends Record<string, unknown> {
  label: string;
  nominalMVAR: number;
  nominalMW: number;
  caseRef?: CaseRef;
}

export interface TransformerData extends Record<string, unknown> {
  label: string;
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
}

export interface CircuitBreakerData extends Record<string, unknown> {
  label: string;
  state: "open" | "closed" | "fault";
  ratingKV: number;
  ratingKA: number;
  normallyOpen: boolean;
}

export interface DisconnectorData extends Record<string, unknown> {
  label: string;
  state: "open" | "closed";
  ratingKV: number;
  type: "gang-operated" | "single-pole";
}

export interface FuseData extends Record<string, unknown> {
  label: string;
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
  /** Orthogonal route from layout, endpoints included, in flow coordinates.
   *  Ignored once either end moves away from where it was routed. */
  route?: { x: number; y: number }[];
  /** Box reserved by the layout for this wire's results, used with `route`. */
  labelAt?: { x: number; y: number; width: number; height: number };
  caseRef?: CaseRef;
  /** The edge's source is the branch's "to" bus. */
  reversed?: boolean;
  outOfService?: boolean;
}

/** Marks a branch that leaves the drawn part of the case. */
export interface OffPageData extends Record<string, unknown> {
  label: string;
  /** Bus at the far end, outside the drawing. */
  bus: number;
  busName: string;
  voltageKV: number;
  caseRef?: CaseRef;
  reversed?: boolean;
  outOfService?: boolean;
}

// ── Typed React Flow unions ──────────────────────────────────────────────────

export type SLDNodeType =
  | "bus"
  | "generator"
  | "load"
  | "shunt"
  | "transformer"
  | "circuitBreaker"
  | "disconnector"
  | "fuse"
  | "offPage";

export type BusNode = Node<BusData, "bus">;
export type GeneratorNode = Node<GeneratorData, "generator">;
export type LoadNode = Node<LoadData, "load">;
export type ShuntNode = Node<ShuntData, "shunt">;
export type TransformerNode = Node<TransformerData, "transformer">;
export type CircuitBreakerNode = Node<CircuitBreakerData, "circuitBreaker">;
export type DisconnectorNode = Node<DisconnectorData, "disconnector">;
export type FuseNode = Node<FuseData, "fuse">;
export type OffPageNode = Node<OffPageData, "offPage">;

export type SLDNode =
  | BusNode
  | GeneratorNode
  | LoadNode
  | ShuntNode
  | TransformerNode
  | CircuitBreakerNode
  | DisconnectorNode
  | FuseNode
  | OffPageNode;

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
  around?: { bus: number; depth: number };
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
  /** Move the selected symbols; arrow keys nudge by one grid step. */
  nudge: (dx: number, dy: number) => void;
  align: (how: AlignMode) => void;
  toggleColorMode: () => void;
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
    around?: { bus: number; depth: number }
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
