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
  ContingencySettings,
  ContingencySummary,
  SolveSettings,
} from "../lib/analysis/types";
import type { Rating } from "../lib/analysis/ratings";
import type { Pickup } from "../lib/analysis/commands.gen";
import { monitorsText, outagesText, type SetupSubsystem } from "../lib/analysis/stnl";

/** AC solve options the setup can set apart from the Power flow tab's. */
export type SolveOverrides = Partial<Pick<SolveSettings, "enforce_q_limits" | "adjust_transformer_taps" | "switch_shunts">>;

/** Rows kept here: the worst outages, to open the first. The table reads pages from the core. */
export const FIRST_PAGE = 100;

/** Every branch and generator on its own, or the NERC TPL-001 planning events. */
export type ContingencySource = "n1" | "planning" | "lists";

/** The planning event categories the core generates, in its order. */
export const PLANNING_CATEGORIES = ["P0", "P1.1", "P1.2", "P1.3", "P1.4", "P1.5", "P2.1", "P7.2"];

export interface ContingencyChoice {
  source: ContingencySource;
  branches: boolean;
  generators: boolean;
  /** Take out each bus with everything connected to it. */
  buses: boolean;
  /** N-2: the chosen branches and generators two at a time. */
  pairs: boolean;
  /** Leave out lines with no resistance and |X| at or below the threshold (PSS/E THRSHZ), p.u. */
  skipZeroImpedance: boolean;
  zeroImpedancePu: number;
  /** The lists file (.stnl) whose contingencies run, for source "lists". */
  listsPath: string | null;
  /** Check only what a lists file's monitor records name, and its file. */
  monitorFromLists: boolean;
  monitorsPath: string | null;
  /** Planning events: the categories to run, and the lowest voltage. */
  categories: string[];
  kvMin: number;
  method: ContingencyMethod;
  pickup: Pickup;
  acCount: number;
  limitPct: number;
  /** Points an overload already there before the outage must gain to be reported. */
  worseByPct: number;
  /** Per unit a voltage problem already there must worsen by to be reported. */
  worseByPu: number;
  /** Report every problem after the outage, also those already there before it. */
  reportExisting: boolean;
  /** The rating loadings are measured against after an outage, and before. */
  rating: Rating;
  baseRating: Rating;
  /** Subsystems built in the setup; saved to and read from .stnl files. */
  subsystems: SetupSubsystem[];
  /** N-1/N-2 outages only within this subsystem (by id); the whole case when null. */
  outageSubsystem: string | null;
  /** Check only this subsystem's branches (and voltages); every branch and bus when null. */
  monitorSubsystem: string | null;
  monitorVoltages: boolean;
  /** AC solve options set apart from the Power flow tab's; empty to follow it. */
  solveOverrides: SolveOverrides;
}

export const DEFAULT_CHOICE: ContingencyChoice = {
  source: "n1",
  branches: true,
  generators: true,
  buses: false,
  pairs: false,
  skipZeroImpedance: false,
  zeroImpedancePu: 0.0001,
  listsPath: null,
  monitorFromLists: false,
  monitorsPath: null,
  categories: PLANNING_CATEGORIES,
  kvMin: 100,
  method: "auto",
  pickup: "reference",
  acCount: 50,
  limitPct: 100,
  worseByPct: 1,
  worseByPu: 0.001,
  reportExisting: false,
  rating: "a",
  baseRating: "a",
  subsystems: [],
  outageSubsystem: null,
  monitorSubsystem: null,
  monitorVoltages: true,
  solveOverrides: {},
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

/**
 * The core's contingency settings for a setup: what a contingency run, and
 * a secure transfer study, take out, check and judge by.
 */
export function contingencySettings(c: ContingencyChoice, solve: SolveSettings, elements: string[] | null): ContingencySettings {
  const planning = c.source === "planning";
  const lists = c.source === "lists" ? c.listsPath : null;
  const known = (id: string | null) => (id && c.subsystems.some((x) => x.id === id) ? id : null);
  // Outages within a subsystem and subsystem monitors run as lists text.
  const inSubsystem = c.source === "n1" ? known(c.outageSubsystem) : null;
  const watched = known(c.monitorSubsystem);
  return {
    elements: planning || lists || inSubsystem ? null : elements,
    lists,
    lists_text: inSubsystem
      ? outagesText(c.subsystems, { subsystem: inSubsystem, branches: c.branches, generators: c.generators, buses: c.buses, pairs: c.pairs })
      : null,
    monitors: c.monitorFromLists ? c.monitorsPath : null,
    monitors_text: watched && !c.monitorFromLists ? monitorsText(c.subsystems, watched, c.limitPct, c.monitorVoltages) : null,
    branches: c.branches,
    generators: c.generators,
    buses: !planning && c.buses,
    pairs: !planning && c.pairs,
    skip_zero_impedance: c.skipZeroImpedance,
    zero_impedance_pu: c.zeroImpedancePu,
    method: c.method,
    pickup: c.pickup,
    ac_count: c.acCount,
    limit_pct: c.limitPct,
    worse_by_pct: c.worseByPct,
    worse_by_pu: c.worseByPu,
    report_existing: c.reportExisting,
    rating: c.rating,
    base_rating: c.baseRating,
    solve: { ...solve, ...c.solveOverrides, model: "ac" },
    planning: planning ? { categories: c.categories, kv_min: c.kvMin, areas: [] } : null,
  };
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
      const summary = await api.runContingencies(contingencySettings(c, solve, elements));
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
      const detail = await api.tripAndCompare(element, get().choice.limitPct, get().choice.rating, get().choice.baseRating);
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
