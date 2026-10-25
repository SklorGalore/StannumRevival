import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import { nanoid } from "nanoid";
import type { AlignMode, DiagramSnapshot, DiagramStore, SLDDiagram, SLDEdge, SLDNode, ConductorData } from "./types";
import { defaultMetadata } from "./defaults";

const HISTORY_LIMIT = 100;
/** Edits with the same key closer together than this are one undo step. */
const COALESCE_MS = 1500;
let lastCheckpoint = { key: "", at: 0 };

/** Copied symbols and the wires between them; shared across pastes. */
let clipboard: DiagramSnapshot | null = null;
let pasteCount = 0;

/** Width and height a node occupies on the canvas. */
function nodeSize(n: SLDNode): { w: number; h: number } {
  if (n.type === "bus") return { w: (n.data as { length: number }).length, h: n.measured?.height ?? 6 };
  return { w: n.measured?.width ?? n.width ?? 40, h: n.measured?.height ?? n.height ?? 40 };
}

/** New positions for `nodes` lined up or spread out as `how` says. */
function aligned(nodes: SLDNode[], how: AlignMode): Map<string, { x: number; y: number }> {
  const boxes = nodes.map((n) => ({ n, ...nodeSize(n), x: n.position.x, y: n.position.y }));
  const minX = Math.min(...boxes.map((b) => b.x));
  const maxX = Math.max(...boxes.map((b) => b.x + b.w));
  const minY = Math.min(...boxes.map((b) => b.y));
  const maxY = Math.max(...boxes.map((b) => b.y + b.h));
  const out = new Map<string, { x: number; y: number }>();
  const put = (b: (typeof boxes)[number], x: number, y: number) => out.set(b.n.id, { x, y });
  if (how === "distribute-x" || how === "distribute-y") {
    const horiz = how === "distribute-x";
    const centre = (b: (typeof boxes)[number]) => (horiz ? b.x + b.w / 2 : b.y + b.h / 2);
    const sorted = [...boxes].sort((a, b) => centre(a) - centre(b));
    const first = centre(sorted[0]);
    const step = sorted.length > 1 ? (centre(sorted[sorted.length - 1]) - first) / (sorted.length - 1) : 0;
    sorted.forEach((b, i) =>
      horiz ? put(b, first + step * i - b.w / 2, b.y) : put(b, b.x, first + step * i - b.h / 2)
    );
    return out;
  }
  for (const b of boxes) {
    if (how === "left") put(b, minX, b.y);
    else if (how === "right") put(b, maxX - b.w, b.y);
    else if (how === "center") put(b, (minX + maxX) / 2 - b.w / 2, b.y);
    else if (how === "top") put(b, b.x, minY);
    else if (how === "bottom") put(b, b.x, maxY - b.h);
    else put(b, b.x, (minY + maxY) / 2 - b.h / 2);
  }
  return out;
}

export const useDiagramStore = create<DiagramStore>()(
  immer((set, get) => ({
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
    colorMode: "voltage",
    past: [],
    future: [],

    setNodes: (nodes, dirty = true) =>
      set((s) => {
        s.nodes = nodes as typeof s.nodes;
        if (dirty) s.isDirty = true;
      }),

    setEdges: (edges, dirty = true) =>
      set((s) => {
        s.edges = edges as typeof s.edges;
        if (dirty) s.isDirty = true;
      }),

    checkpoint: (key = "") => {
      const now = Date.now();
      if (key && key === lastCheckpoint.key && now - lastCheckpoint.at < COALESCE_MS) {
        lastCheckpoint.at = now;
        return;
      }
      lastCheckpoint = { key, at: now };
      const { nodes, edges } = get();
      set((s) => {
        s.past.push({ nodes, edges } as never);
        if (s.past.length > HISTORY_LIMIT) s.past.shift();
        s.future = [];
      });
    },

    undo: () => {
      const { past, nodes, edges } = get();
      if (!past.length) return;
      lastCheckpoint = { key: "", at: 0 };
      set((s) => {
        const prev = s.past.pop()!;
        s.future.push({ nodes, edges } as never);
        s.nodes = prev.nodes;
        s.edges = prev.edges;
        s.isDirty = true;
      });
    },

    redo: () => {
      const { future, nodes, edges } = get();
      if (!future.length) return;
      lastCheckpoint = { key: "", at: 0 };
      set((s) => {
        const next = s.future.pop()!;
        s.past.push({ nodes, edges } as never);
        s.nodes = next.nodes;
        s.edges = next.edges;
        s.isDirty = true;
      });
    },

    copySelection: () => {
      const { nodes, edges } = get();
      const picked = nodes.filter((n) => n.selected);
      if (!picked.length) return;
      const ids = new Set(picked.map((n) => n.id));
      clipboard = {
        nodes: structuredClone(picked),
        edges: structuredClone(edges.filter((e) => ids.has(e.source) && ids.has(e.target))),
      };
      pasteCount = 0;
    },

    paste: () => {
      if (!clipboard) return;
      get().checkpoint();
      pasteCount += 1;
      const offset = 40 * pasteCount;
      const idMap = new Map(clipboard.nodes.map((n) => [n.id, nanoid()]));
      const nodes = clipboard.nodes.map(
        (n) =>
          ({
            ...structuredClone(n),
            id: idMap.get(n.id)!,
            position: { x: n.position.x + offset, y: n.position.y + offset },
            selected: true,
          }) as SLDNode
      );
      const edges = clipboard.edges.map((e) => {
        const copy = structuredClone(e) as SLDEdge;
        // The layout route belonged to the originals' positions.
        if (copy.data) delete copy.data.route;
        return {
          ...copy,
          id: nanoid(),
          source: idMap.get(e.source)!,
          target: idMap.get(e.target)!,
          selected: false,
        } as SLDEdge;
      });
      set((s) => {
        for (const n of s.nodes) n.selected = false;
        for (const e of s.edges) e.selected = false;
        (s.nodes as SLDNode[]).push(...nodes);
        (s.edges as SLDEdge[]).push(...edges);
        s.isDirty = true;
      });
    },

    duplicateSelection: () => {
      const saved = clipboard;
      const savedCount = pasteCount;
      get().copySelection();
      get().paste();
      clipboard = saved ?? clipboard;
      pasteCount = saved ? savedCount : pasteCount;
    },

    selectAll: () =>
      set((s) => {
        for (const n of s.nodes) n.selected = true;
        for (const e of s.edges) e.selected = true;
      }),

    selectOnly: (nodeIds, edgeIds) =>
      set((s) => {
        const n = new Set(nodeIds);
        const e = new Set(edgeIds);
        for (const node of s.nodes) node.selected = n.has(node.id);
        for (const edge of s.edges) edge.selected = e.has(edge.id);
        s.selectedNodeIds = nodeIds;
        s.selectedEdgeIds = edgeIds;
      }),

    clearSelection: () =>
      set((s) => {
        for (const n of s.nodes) n.selected = false;
        for (const e of s.edges) e.selected = false;
      }),

    nudge: (dx, dy) => {
      if (!get().nodes.some((n) => n.selected)) return;
      get().checkpoint("nudge");
      set((s) => {
        for (const n of s.nodes)
          if (n.selected) n.position = { x: n.position.x + dx, y: n.position.y + dy };
        s.isDirty = true;
      });
    },

    align: (how) => {
      const picked = get().nodes.filter((n) => n.selected);
      if (picked.length < 2) return;
      get().checkpoint();
      const moves = aligned(picked, how);
      set((s) => {
        for (const n of s.nodes) {
          const to = moves.get(n.id);
          if (to) n.position = to;
        }
        s.isDirty = true;
      });
    },

    toggleColorMode: () =>
      set((s) => {
        s.colorMode = s.colorMode === "voltage" ? "mono" : "voltage";
      }),

    updateNodeData: (id, data) => {
      get().checkpoint(`node:${id}:${Object.keys(data).join(",")}`);
      set((s) => {
        const node = s.nodes.find((n) => n.id === id);
        if (node) Object.assign(node.data, data);
        s.isDirty = true;
      });
    },

    updateEdgeData: (id, data: Partial<ConductorData>) => {
      get().checkpoint(`edge:${id}:${Object.keys(data).join(",")}`);
      set((s) => {
        const edge = s.edges.find((e) => e.id === id);
        if (edge) {
          if (edge.data) Object.assign(edge.data, data);
          else edge.data = data as ConductorData;
        }
        s.isDirty = true;
      });
    },

    addNode: (node) => {
      get().checkpoint();
      set((s) => {
        (s.nodes as SLDNode[]).push(node);
        s.isDirty = true;
      });
    },

    deleteSelected: () => {
      const { nodes, edges } = get();
      if (!nodes.some((n) => n.selected) && !edges.some((e) => e.selected)) return;
      get().checkpoint();
      set((s) => {
        const nodeIds = new Set(s.nodes.filter((n) => n.selected).map((n) => n.id));
        const edgeIds = new Set(s.edges.filter((e) => e.selected).map((e) => e.id));
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
      });
    },

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
        s.past = [];
        s.future = [];
      }),

    loadDiagramFromJSON: (diagram: SLDDiagram) =>
      set((s) => {
        s.nodes = diagram.nodes as typeof s.nodes;
        s.edges = diagram.edges as typeof s.edges;
        s.viewport = diagram.viewport;
        s.metadata = diagram.metadata;
        s.isDirty = false;
        s.past = [];
        s.future = [];
        s.selectedNodeIds = [];
        s.selectedEdgeIds = [];
        s.fitRequest += 1;
      }),

    replaceWithGenerated: (nodes, edges, caseName) => {
      // Undoable, so a generated drawing never silently replaces your work.
      get().checkpoint();
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
      });
    },

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
