// Which Diagram layers are on, and their inputs (the path's ends, the field
// filters and colour rules), shared by both drawing styles. The marks they
// resolve to are kept here too, so every symbol can look up its own.
import { create } from "zustand";
import type { TopoPath } from "../lib/analysis/types";
import { NO_MARKS, type LayerId, type LayerLegend, type LayerMarks, type Mark } from "../lib/diagram/layers";
import { defaultColorSettings, type ColorSettings } from "../modules/topology/fields/field-colors";
import type { FilterRule } from "../modules/topology/fields/field-view";

export interface PathQuery {
  from: number;
  to: number;
  result: TopoPath | null;
  /** Why there is no result: no path, or the search failed. */
  message: string | null;
}

interface LayerState {
  on: Record<LayerId, boolean>;
  toggle: (id: LayerId, on?: boolean) => void;
  path: PathQuery | null;
  setPath: (path: PathQuery | null) => void;
  rules: FilterRule[];
  setRules: (rules: FilterRule[]) => void;
  colors: ColorSettings;
  setColors: (colors: ColorSettings) => void;
  /** The Layers panel beside the canvas. */
  panel: boolean;
  setPanel: (open: boolean) => void;
  /** The topology tables drawer under the canvas. */
  tables: boolean;
  setTables: (open: boolean) => void;
  /** What the layers that are on resolve to (see lib/diagram/layers.ts). */
  marks: LayerMarks;
  legends: LayerLegend[];
  setResolved: (marks: LayerMarks, legends: LayerLegend[]) => void;
  /** A new case: keep which layers are on, forget the path. */
  reset: (fields?: { rules: FilterRule[]; colors: ColorSettings }) => void;
}

export const DEFAULT_LAYERS: Record<LayerId, boolean> = {
  critical: true,
  islands: false,
  merged: false,
  path: true,
  fields: true,
  contingency: true,
};

export const useLayers = create<LayerState>((set) => ({
  on: { ...DEFAULT_LAYERS },
  toggle: (id, on) => set((s) => ({ on: { ...s.on, [id]: on ?? !s.on[id] } })),
  path: null,
  setPath: (path) => set((s) => ({ path, on: path ? { ...s.on, path: true } : s.on })),
  rules: [],
  setRules: (rules) => set((s) => ({ rules, on: rules.length ? { ...s.on, fields: true } : s.on })),
  colors: defaultColorSettings(),
  setColors: (colors) => set((s) => ({ colors, on: { ...s.on, fields: true } })),
  panel: false,
  setPanel: (panel) => set({ panel }),
  tables: false,
  setTables: (tables) => set({ tables }),
  marks: NO_MARKS,
  legends: [],
  setResolved: (marks, legends) => set({ marks, legends }),
  reset: (fields) =>
    set({ path: null, rules: fields?.rules ?? [], colors: fields?.colors ?? defaultColorSettings(), marks: NO_MARKS, legends: [] }),
}));

/** The mark for a bus, or undefined. */
export function useBusMark(bus: number | undefined): Mark | undefined {
  return useLayers((s) => (bus === undefined ? undefined : s.marks.buses.get(bus)));
}

/** The mark for a branch (or other element) by shared id, or undefined. */
export function useElementMark(element: string | undefined): Mark | undefined {
  return useLayers((s) => (element === undefined ? undefined : s.marks.elements.get(element)));
}
