// The redispatch dialog's choices: what to relieve, which generators may
// move, and the last preview. Kept while the app is open.
import { create } from "zustand";
import type { Rating, RedispatchReport, RedispatchSettings } from "../lib/analysis/commands.gen";

export interface RedispatchChoice {
  rating: Rating;
  limitPct: number | null;
  /** Relieve every monitored branch over its limit, or only the listed ones. */
  only: boolean;
  /** Element ids, one per line or comma separated. */
  branches: string;
  /** Areas whose generators may move (every area when empty). */
  areas: number[];
  /** Monitor only branches with an end at or above this base voltage. */
  monitorKvMin: number | null;
  acCheck: boolean;
  solveAfter: boolean;
}

export const DEFAULT_REDISPATCH_CHOICE: RedispatchChoice = {
  rating: "a",
  limitPct: 100,
  only: false,
  branches: "",
  areas: [],
  monitorKvMin: null,
  acCheck: true,
  solveAfter: true,
};

/** Element ids typed into the "only" box. */
export function branchList(text: string): string[] {
  return text
    .split(/[\s,;]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Whether an element id names a branch the redispatch can relieve. */
export function isBranch(element: string): boolean {
  return /^(line|transformer|switch):/.test(element);
}

/** The core's settings for a choice, or why it isn't ready yet. */
export function redispatchSettings(c: RedispatchChoice): { settings: RedispatchSettings } | { problem: string } {
  if (c.limitPct === null || !(c.limitPct > 0)) return { problem: "Enter a loading limit above 0%." };
  const settings: RedispatchSettings = {
    rating: c.rating,
    limit_pct: c.limitPct,
    generators: { areas: c.areas },
    monitor: { kv_min: c.monitorKvMin },
    ac_check: c.acCheck,
  };
  if (c.only) {
    const list = branchList(c.branches);
    if (!list.length) return { problem: "List the branches to relieve, or relieve every overload." };
    settings.branches = list;
  }
  return { settings };
}

/** One line for the log after a redispatch. */
export function redispatchLine(r: RedispatchReport): string {
  const s = r.summary;
  if (s.units_moved === 0) return "Redispatch: nothing moved";
  const worst = s.worst_after ? `; worst ${s.worst_after.name} ${s.worst_after.pct_after.toFixed(1)}%` : "";
  return `Redispatched ${s.moved_mw.toFixed(1)} MW across ${s.units_moved} generator${s.units_moved === 1 ? "" : "s"}: ${s.over_before} → ${s.over_after} branches over their limit${worst}`;
}

interface RedispatchState {
  open: boolean;
  choice: RedispatchChoice;
  setOpen: (open: boolean) => void;
  setChoice: (patch: Partial<RedispatchChoice>) => void;
  reset: () => void;
}

export const useRedispatch = create<RedispatchState>((set) => ({
  open: false,
  choice: DEFAULT_REDISPATCH_CHOICE,
  setOpen: (open) => set({ open }),
  setChoice: (patch) => set((s) => ({ choice: { ...s.choice, ...patch } })),
  reset: () => set({ choice: DEFAULT_REDISPATCH_CHOICE }),
}));
