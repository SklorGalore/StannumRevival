// The Diagram module's drawing style, and what both styles share: the bus a
// neighbourhood is drawn around, its hop count, and the selected element.
import { create } from "zustand";

/** One-line: symbols and routed wires. Network: the bus-branch graph (Cytoscape). */
export type DrawingStyle = "oneline" | "network";

export interface Focus {
  bus: number;
  depth: number;
}

interface DrawingStyleState {
  style: DrawingStyle;
  setStyle: (style: DrawingStyle) => void;
  /** What the Network style draws around. The One-line style keeps its own in metadata.around. */
  focus: Focus | null;
  setFocus: (focus: Focus | null) => void;
  /** The element selected in the Diagram (a shared element id), kept across styles. */
  element: string | null;
  setElement: (element: string | null) => void;
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
    set({ focus: focus && { bus: focus.bus, depth: Math.min(MAX_HOPS, Math.max(1, focus.depth)) } }),
  element: null,
  setElement: (element) => set({ element }),
  fit: 0,
  reveal: null,
  requestFit: () => set((s) => ({ fit: s.fit + 1 })),
  busy: false,
  error: null,
  reset: () => set({ focus: null, element: null, error: null }),
}));
