// The last contingency run: its settings, summary, ranked rows and the
// outage that is open. stannum-core keeps the full results; this holds what
// is on screen. A new case or a study change clears it, as the core does.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import { useWorkspace } from "./workspaceStore";
import type {
  CategoryCount,
  ContingencyDetail,
  ContingencyMethod,
  ContingencyProgress,
  ContingencyRow,
  ContingencySummary,
  SolveSettings,
} from "../lib/analysis/types";

/** Rows kept here: the worst outages, to open the first. The table reads pages from the core. */
export const FIRST_PAGE = 100;

/** Every branch and generator on its own, or the NERC TPL-001 planning events. */
export type ContingencySource = "n1" | "planning";

/** The planning event categories the core generates, in its order. */
export const PLANNING_CATEGORIES = ["P0", "P1.1", "P1.2", "P1.3", "P1.4", "P1.5", "P2.1", "P7.2"];

export interface ContingencyChoice {
  source: ContingencySource;
  branches: boolean;
  generators: boolean;
  /** Planning events: the categories to run, and the lowest voltage. */
  categories: string[];
  kvMin: number;
  method: ContingencyMethod;
  acCount: number;
  limitPct: number;
}

export const DEFAULT_CHOICE: ContingencyChoice = {
  source: "n1",
  branches: true,
  generators: true,
  categories: PLANNING_CATEGORIES,
  kvMin: 100,
  method: "screen",
  acCount: 50,
  limitPct: 100,
};

interface ContingencyState {
  choice: ContingencyChoice;
  setChoice: (c: Partial<ContingencyChoice>) => void;
  summary: ContingencySummary | null;
  rows: ContingencyRow[];
  /** How many planning events each category gives on the open case, at `kvMin`. */
  counts: CategoryCount[] | null;
  loadCounts: () => Promise<void>;
  running: boolean;
  progress: ContingencyProgress | null;
  error: string | null;
  /** The rank of the outage that is open, and its detail once loaded. */
  open: number | null;
  detail: ContingencyDetail | null;
  /** Show the open outage on the diagram (the Contingency layer). */
  onDiagram: boolean;
  /** The element being tripped from an inspector, while it solves. */
  tripping: string | null;
  /** "Trip and compare": take one element out of the latest solve and draw what changes. */
  trip: (element: string) => Promise<ContingencyDetail>;
  run: (solve: SolveSettings, elements: string[] | null) => Promise<boolean>;
  cancel: () => void;
  openRow: (rank: number | null) => Promise<void>;
  setOnDiagram: (on: boolean) => void;
  clear: () => void;
}

/**
 * The outage drawn on the diagram is the result every view shows: the
 * diagram's flow labels, the tables and the inspector follow it. Without one
 * they go back to the latest solve.
 */
function followShown(s: ContingencyState) {
  const ws = useWorkspace.getState();
  const d = s.detail;
  if (s.onDiagram && d) {
    const element = d.row.element;
    void ws.showResult(
      s.open === null
        ? { name: `trip ${element}`, label: "Tripped", element }
        : { name: `contingency ${d.row.rank}`, label: `Outage #${d.row.rank}`, element }
    );
  } else if (ws.shown) ws.showLatest();
}

export const useContingencies = create<ContingencyState>((set, get) => ({
  choice: DEFAULT_CHOICE,
  setChoice: (c) => set((s) => ({ choice: { ...s.choice, ...c } })),
  summary: null,
  rows: [],
  counts: null,
  async loadCounts() {
    const kvMin = get().choice.kvMin;
    try {
      const counts = await api.planningEventCounts({ categories: PLANNING_CATEGORIES, kv_min: kvMin, areas: [] });
      if (get().choice.kvMin === kvMin) set({ counts });
    } catch {
      set({ counts: null });
    }
  },
  running: false,
  progress: null,
  error: null,
  open: null,
  detail: null,
  onDiagram: false,
  async run(solve, elements) {
    const c = get().choice;
    set({ running: true, error: null, progress: null, open: null, detail: null });
    followShown(get());
    // Follow the run's progress while it goes.
    const timer = setInterval(() => {
      api
        .contingencyProgress()
        .then((p) => get().running && set({ progress: p }))
        .catch(() => undefined);
    }, 250);
    try {
      const planning = c.source === "planning";
      const summary = await api.runContingencies({
        elements: planning ? null : elements,
        branches: c.branches,
        generators: c.generators,
        method: c.method,
        ac_count: c.acCount,
        limit_pct: c.limitPct,
        solve: { ...solve, model: "ac" },
        planning: planning ? { categories: c.categories, kv_min: c.kvMin, areas: [] } : null,
      });
      const rows = await api.contingencyRows(0, FIRST_PAGE);
      set({ summary, rows });
      return true;
    } catch (e) {
      set({ error: (e as Error).message });
      return false;
    } finally {
      clearInterval(timer);
      set({ running: false, progress: null });
    }
  },
  cancel() {
    void api.cancelContingencies();
  },
  async openRow(rank) {
    set({ open: rank, detail: rank === null ? null : get().detail?.row.rank === rank ? get().detail : null });
    if (rank === null) return followShown(get());
    try {
      const detail = await api.contingencyDetail(rank);
      if (get().open === rank) {
        set({ detail });
        followShown(get());
      }
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },
  setOnDiagram: (onDiagram) => {
    set({ onDiagram });
    followShown(get());
  },
  tripping: null,
  async trip(element) {
    set({ tripping: element });
    try {
      const detail = await api.tripAndCompare(element, get().choice.limitPct);
      // Shown on its own, not as a row of the last run.
      set({ detail, open: null, onDiagram: true });
      followShown(get());
      return detail;
    } finally {
      set({ tripping: null });
    }
  },
  clear: () => {
    set({ summary: null, rows: [], open: null, detail: null, error: null, onDiagram: false, tripping: null });
    followShown(get());
  },
}));
