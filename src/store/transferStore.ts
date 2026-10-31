// The last transfer limit study (DFAX and FCITC): its settings and results.
// stannum-core runs it (transfer.rs) and answers everything at once; a new
// case or a study change clears it, as for contingencies.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import type { ContingencyProgress } from "../lib/analysis/types";
import type { Scaling, Subsystem, TransferResults, TransferSettings } from "../lib/analysis/commands.gen";
import type { Rating } from "../lib/analysis/ratings";

/** How one side of the transfer is picked: an area, zone or owner, or listed buses. */
export type SideKind = "area" | "zone" | "owner" | "buses";

export interface Side {
  kind: SideKind;
  /** The area, zone or owner number. */
  value: number;
  /** Bus numbers, comma or space separated, for kind "buses". */
  buses: string;
}

export interface TransferChoice {
  source: Side;
  sink: Side;
  scaling: Scaling;
  /** Study each branch outage (and generator outage) as well as the base case. */
  outages: boolean;
  generators: boolean;
  /** Only outages and monitored branches with a terminal at or above this, kV. */
  outageKvMin: number;
  monitorKvMin: number;
  /** Smallest distribution factor that limits, percent. */
  cutoffPct: number;
  limitPct: number;
  rating: Rating;
  baseRating: Rating;
  includePast: boolean;
}

export const DEFAULT_TRANSFER: TransferChoice = {
  source: { kind: "area", value: 0, buses: "" },
  sink: { kind: "area", value: 0, buses: "" },
  scaling: "generation",
  outages: true,
  generators: false,
  outageKvMin: 0,
  monitorKvMin: 0,
  cutoffPct: 5,
  limitPct: 100,
  rating: "a",
  baseRating: "a",
  includePast: false,
};

/** The bus numbers in a typed list such as "101, 102 205". */
export function busList(text: string): number[] {
  return text
    .split(/[\s,;]+/)
    .map((t) => Number(t))
    .filter((n) => Number.isInteger(n) && n > 0);
}

export function subsystem(side: Side): Subsystem {
  switch (side.kind) {
    case "area":
      return { areas: [side.value] };
    case "zone":
      return { zones: [side.value] };
    case "owner":
      return { owners: [side.value] };
    case "buses":
      return { buses: busList(side.buses) };
  }
}

export function transferSettings(c: TransferChoice): TransferSettings {
  return {
    source: subsystem(c.source),
    sink: subsystem(c.sink),
    scaling: c.scaling,
    monitor: { branches: true, kv_min: c.monitorKvMin, elements: [], interfaces: [] },
    contingencies: c.outages ? { branches: true, generators: c.generators, kv_min: c.outageKvMin, skip_zero_impedance: false } : null,
    cutoff: c.cutoffPct / 100,
    limit_pct: c.limitPct,
    rating: c.rating,
    base_rating: c.baseRating,
    include_past_limits: c.includePast,
    max_rows: 500,
  };
}

interface TransferState {
  choice: TransferChoice;
  setChoice: (c: Partial<TransferChoice>) => void;
  results: TransferResults | null;
  running: boolean;
  progress: ContingencyProgress | null;
  error: string | null;
  run: () => Promise<boolean>;
  cancel: () => void;
  clear: () => void;
}

export const useTransfers = create<TransferState>((set, get) => ({
  choice: DEFAULT_TRANSFER,
  setChoice: (c) => set((s) => ({ choice: { ...s.choice, ...c } })),
  results: null,
  running: false,
  progress: null,
  error: null,
  async run() {
    set({ running: true, error: null, progress: null });
    const timer = setInterval(() => {
      api
        .transferProgress()
        .then((p) => get().running && set({ progress: p }))
        .catch(() => undefined);
    }, 250);
    try {
      const results = await api.runTransferLimits(transferSettings(get().choice));
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
    void api.cancelTransferLimits();
  },
  clear: () => set({ results: null, error: null }),
}));
