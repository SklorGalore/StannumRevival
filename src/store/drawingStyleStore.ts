// The Diagram module's drawing style, and what both styles share: the bus a
// neighbourhood is drawn around, its hop count, and the selected element.
import { create } from "zustand";

/**
 * One-line: symbols and routed wires. Network: the bus-branch graph
 * (Cytoscape). Map: the whole case at its bus coordinates (MapLibre).
 */
export type DrawingStyle = "oneline" | "network" | "map";

export interface Focus {
  bus: number;
  depth: number;
}

/**
 * What the Network style shows: the neighbourhood of the focus bus, a whole
 * island (from the Islands table), or the buses on the shortest path.
 */
export type NetworkView = { kind: "around" } | { kind: "island"; island: number } | { kind: "path" };

interface DrawingStyleState {
  style: DrawingStyle;
  setStyle: (style: DrawingStyle) => void;
  /** What the Network style draws around. The One-line style keeps its own in metadata.around. */
  focus: Focus | null;
  setFocus: (focus: Focus | null) => void;
  /** The element selected in the Diagram (a shared element id), kept across styles. */
  element: string | null;
  setElement: (element: string | null) => void;
  view: NetworkView;
  /** Show a whole island in the Network style. */
  showIsland: (island: number) => void;
  /** Show the shortest path's buses in the Network style. */
  showPath: () => void;
  /** Back to the neighbourhood of the focus bus. */
  showAround: () => void;
  /** Incremented to fit the network to the view. */
  fit: number;
  requestFit: () => void;
  /** Set when switching to One-line should bring the selected symbols into view. */
  reveal: { n: number; redrawn: boolean } | null;
  /** True while switching to One-line redraws it around the network's focus. */
  busy: boolean;
  /** Why the last redraw failed, until dismissed. */
  error: string | null;
  /** Forget the focus and selection, for a newly opened case. */
  reset: () => void;
}

export const MAX_HOPS = 6;

export const useDrawingStyle = create<DrawingStyleState>((set) => ({
  style: "oneline",
  setStyle: (style) => set({ style }),
  focus: null,
  setFocus: (focus) =>
    set((s) => {
      const next = focus && { bus: focus.bus, depth: Math.min(MAX_HOPS, Math.max(1, focus.depth)) };
      // The same focus again (the one-line catching up) keeps an island or
      // path view, and so does a first focus (the network catching up).
      if (next && s.focus && next.bus === s.focus.bus && next.depth === s.focus.depth) return {};
      return s.focus ? { view: { kind: "around" }, focus: next } : { focus: next };
    }),
  view: { kind: "around" },
  showIsland: (island) => set({ style: "network", view: { kind: "island", island } }),
  showPath: () => set({ style: "network", view: { kind: "path" } }),
  showAround: () => set({ view: { kind: "around" } }),
  element: null,
  setElement: (element) => set({ element }),
  fit: 0,
  reveal: null,
  requestFit: () => set((s) => ({ fit: s.fit + 1 })),
  busy: false,
  error: null,
  reset: () => set({ focus: null, element: null, error: null, view: { kind: "around" } }),
}));
