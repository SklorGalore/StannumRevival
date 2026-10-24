import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type { DiagramStore, SLDDiagram, SLDNode, ConductorData } from "./types";
import { defaultMetadata } from "./defaults";

export const useDiagramStore = create<DiagramStore>()(
  immer((set) => ({
    nodes: [],
    edges: [],
    viewport: { x: 0, y: 0, zoom: 1 },
    metadata: { ...defaultMetadata },

    currentFilePath: null,
    isDirty: false,
    selectedNodeIds: [],
    selectedEdgeIds: [],

    paletteOpen: true,
    propertiesPanelOpen: true,
    gridVisible: true,
    snapToGrid: true,
    gridSize: 20,
    showResults: true,
    fitRequest: 0,

    setNodes: (nodes) =>
      set((s) => {
        s.nodes = nodes as typeof s.nodes;
        s.isDirty = true;
      }),

    setEdges: (edges) =>
      set((s) => {
        s.edges = edges as typeof s.edges;
        s.isDirty = true;
      }),

    updateNodeData: (id, data) =>
      set((s) => {
        const node = s.nodes.find((n) => n.id === id);
        if (node) Object.assign(node.data, data);
        s.isDirty = true;
      }),

    updateEdgeData: (id, data: Partial<ConductorData>) =>
      set((s) => {
        const edge = s.edges.find((e) => e.id === id);
        if (edge) Object.assign(edge.data ?? {}, data);
        s.isDirty = true;
      }),

    addNode: (node) =>
      set((s) => {
        (s.nodes as SLDNode[]).push(node);
        s.isDirty = true;
      }),

    deleteSelected: () =>
      set((s) => {
        const nodeIds = new Set(s.selectedNodeIds);
        const edgeIds = new Set(s.selectedEdgeIds);
        s.nodes = s.nodes.filter((n) => !nodeIds.has(n.id)) as typeof s.nodes;
        s.edges = s.edges.filter(
          (e) =>
            !edgeIds.has(e.id) &&
            !nodeIds.has(e.source) &&
            !nodeIds.has(e.target)
        ) as typeof s.edges;
        s.selectedNodeIds = [];
        s.selectedEdgeIds = [];
        s.isDirty = true;
      }),

    setSelection: (nodeIds, edgeIds) =>
      set((s) => {
        s.selectedNodeIds = nodeIds;
        s.selectedEdgeIds = edgeIds;
      }),

    newDiagram: () =>
      set((s) => {
        s.nodes = [];
        s.edges = [];
        s.viewport = { x: 0, y: 0, zoom: 1 };
        s.metadata = {
          ...defaultMetadata,
          createdAt: new Date().toISOString(),
          modifiedAt: new Date().toISOString(),
        };
        s.currentFilePath = null;
        s.isDirty = false;
        s.selectedNodeIds = [];
        s.selectedEdgeIds = [];
      }),

    loadDiagramFromJSON: (diagram: SLDDiagram) =>
      set((s) => {
        s.nodes = diagram.nodes as typeof s.nodes;
        s.edges = diagram.edges as typeof s.edges;
        s.viewport = diagram.viewport;
        s.metadata = diagram.metadata;
        s.isDirty = false;
        s.selectedNodeIds = [];
        s.selectedEdgeIds = [];
        s.fitRequest += 1;
      }),

    replaceWithGenerated: (nodes, edges, caseName) =>
      set((s) => {
        const now = new Date().toISOString();
        s.nodes = nodes as typeof s.nodes;
        s.edges = edges as typeof s.edges;
        s.metadata = {
          ...defaultMetadata,
          title: caseName.replace(/\.[^.]+$/, ""),
          description: `One-line generated from ${caseName}`,
          caseName,
          createdAt: now,
          modifiedAt: now,
        };
        s.currentFilePath = null;
        s.isDirty = true;
        s.selectedNodeIds = [];
        s.selectedEdgeIds = [];
        s.fitRequest += 1;
      }),

    toggleResults: () => set((s) => { s.showResults = !s.showResults; }),
    requestFit: () => set((s) => { s.fitRequest += 1; }),

    setFilePath: (path) =>
      set((s) => {
        s.currentFilePath = path;
      }),

    markDirty: () => set((s) => { s.isDirty = true; }),
    markClean: () => set((s) => { s.isDirty = false; }),

    toggleGrid: () => set((s) => { s.gridVisible = !s.gridVisible; }),
    toggleSnap: () => set((s) => { s.snapToGrid = !s.snapToGrid; }),

    setViewport: (vp) =>
      set((s) => {
        s.viewport = vp;
      }),

    updateMetadata: (meta) =>
      set((s) => {
        Object.assign(s.metadata, meta);
        s.metadata.modifiedAt = new Date().toISOString();
        s.isDirty = true;
      }),
  }))
);
