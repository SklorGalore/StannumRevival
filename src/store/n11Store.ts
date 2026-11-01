// The last N-1-1 run: its settings and results. stannum-core runs it
// (n11.rs) and answers everything at once; a new case or a study change
// clears it, as for the other studies.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import type { Adjustment, N11Progress, N11Results, N11Settings, PairRow, Rating } from "../lib/analysis/commands.gen";
import { branchList } from "./redispatchStore";

export interface N11Choice {
  rating: Rating;
  limitPct: number | null;
  /** Take out branches and/or generators, first and second. */
  branches: boolean;
  generators: boolean;
  /** List the first outages, or take every second one first too. */
  listFirst: boolean;
  /** Element ids, one per line or comma separated. */
  first: string;
  adjustment: Adjustment;
}

export const DEFAULT_N11: N11Choice = {
  rating: "a",
  limitPct: 100,
  branches: true,
  generators: true,
  listFirst: false,
  first: "",
  adjustment: "redispatch",
};

/** The core's settings for a choice, or why it isn't ready yet. */
export function n11Settings(c: N11Choice): { settings: N11Settings } | { problem: string } {
  if (c.limitPct === null || !(c.limitPct > 0)) return { problem: "Enter a loading limit above 0%." };
  if (!c.branches && !c.generators) return { problem: "Take out branches, generators or both." };
  const settings: N11Settings = {
    adjustment: c.adjustment,
    contingencies: { rating: c.rating, limit_pct: c.limitPct, branches: c.branches, generators: c.generators },
    redispatch: { rating: c.rating, limit_pct: c.limitPct },
  };
  if (c.listFirst) {
    const list = branchList(c.first);
    if (!list.length) return { problem: "List the first outages, or take every element out first." };
    settings.first = list;
  }
  return { settings };
}

/** The pairs as CSV rows. */
export function pairRows(pairs: PairRow[]): Record<string, number | string>[] {
  return pairs.map((p) => ({
    rank: p.rank,
    first: p.first,
    first_name: p.first_name,
    second: p.second,
    second_name: p.second_name,
    checked: p.checked,
    converged: p.converged === null || p.converged === undefined ? "" : p.converged ? "yes" : "no",
    overloads: p.overloads,
    worst_branch: p.worst?.name ?? "",
    worst_from: p.worst?.from ?? "",
    worst_to: p.worst?.to ?? "",
    worst_pct: p.worst ? Number(p.worst.pct.toFixed(2)) : "",
    worst_before_pct: p.worst ? Number(p.worst.base_pct.toFixed(2)) : "",
    voltage_violations: p.voltage_violations,
    worst_bus: p.worst_voltage?.bus ?? "",
    worst_vm: p.worst_voltage ? Number(p.worst_voltage.vm.toFixed(4)) : "",
    note: p.note ?? "",
  }));
}

interface N11State {
  choice: N11Choice;
  setChoice: (c: Partial<N11Choice>) => void;
  results: N11Results | null;
  running: boolean;
  progress: N11Progress | null;
  error: string | null;
  run: () => Promise<boolean>;
  cancel: () => void;
  clear: () => void;
}

export const useN11 = create<N11State>((set, get) => ({
  choice: DEFAULT_N11,
  setChoice: (c) => set((s) => ({ choice: { ...s.choice, ...c } })),
  results: null,
  running: false,
  progress: null,
  error: null,
  async run() {
    const r = n11Settings(get().choice);
    if ("problem" in r) {
      set({ error: r.problem });
      return false;
    }
    set({ running: true, error: null, progress: null });
    const timer = setInterval(() => {
      api
        .n11Progress()
        .then((p) => get().running && set({ progress: p }))
        .catch(() => undefined);
    }, 250);
    try {
      const results = await api.runN11(r.settings);
      set({ results });
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
    void api.cancelN11();
  },
  clear: () => set({ results: null, error: null }),
}));
