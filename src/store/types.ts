import type { Node, Edge } from "@xyflow/react";

// ── Node data interfaces ─────────────────────────────────────────────────────

export interface BusData extends Record<string, unknown> {
  label: string;
  voltageKV: number;
  orientation: "horizontal" | "vertical";
  length: number;
}

export interface GeneratorData extends Record<string, unknown> {
  label: string;
  nominalMW: number;
  voltageKV: number;
  powerFactor: number;
  groundingType: "solidly" | "resistance" | "reactance" | "ungrounded";
}

export interface LoadData extends Record<string, unknown> {
  label: string;
  nominalMW: number;
  nominalMVAR: number;
  loadType: "static" | "motor" | "composite";
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
}

// ── Typed React Flow unions ──────────────────────────────────────────────────

export type SLDNodeType =
  | "bus"
  | "generator"
  | "load"
  | "transformer"
  | "circuitBreaker"
  | "disconnector"
  | "fuse";

export type BusNode = Node<BusData, "bus">;
export type GeneratorNode = Node<GeneratorData, "generator">;
export type LoadNode = Node<LoadData, "load">;
export type TransformerNode = Node<TransformerData, "transformer">;
export type CircuitBreakerNode = Node<CircuitBreakerData, "circuitBreaker">;
export type DisconnectorNode = Node<DisconnectorData, "disconnector">;
export type FuseNode = Node<FuseData, "fuse">;

export type SLDNode =
  | BusNode
  | GeneratorNode
  | LoadNode
  | TransformerNode
  | CircuitBreakerNode
  | DisconnectorNode
  | FuseNode;

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
}

export interface SLDDiagram {
  metadata: DiagramMetadata;
  nodes: SLDNode[];
  edges: SLDEdge[];
  viewport: { x: number; y: number; zoom: number };
}

// ── Store shape ──────────────────────────────────────────────────────────────

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

  setNodes: (nodes: SLDNode[]) => void;
  setEdges: (edges: SLDEdge[]) => void;
  updateNodeData: (id: string, data: Partial<SLDNode["data"]>) => void;
  updateEdgeData: (id: string, data: Partial<ConductorData>) => void;
  addNode: (node: SLDNode) => void;
  deleteSelected: () => void;
  setSelection: (nodeIds: string[], edgeIds: string[]) => void;

  newDiagram: () => void;
  loadDiagramFromJSON: (diagram: SLDDiagram) => void;
  setFilePath: (path: string | null) => void;
  markDirty: () => void;
  markClean: () => void;

  toggleGrid: () => void;
  toggleSnap: () => void;
  setViewport: (vp: { x: number; y: number; zoom: number }) => void;
  updateMetadata: (meta: Partial<DiagramMetadata>) => void;
}
