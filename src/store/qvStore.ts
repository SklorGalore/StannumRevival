// The last QV curve study (reactive power margin): its settings and
// results. stannum-core runs it (qv.rs) and answers everything at once; a
// new case or a study change clears it, as for PV curves.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import type { BusRow, ContingencyProgress } from "../lib/analysis/types";
import type { QvResults, QvSettings } from "../lib/analysis/commands.gen";
import { busList } from "./transferStore";
import { positive } from "./pvStore";

/** The most buses one study sweeps (QvSettings in qv.rs). */
export const MAX_QV_BUSES = 100;

export interface QvChoice {
  /** Bus numbers, typed: "7314, 7340". */
  buses: string;
  /** Highest and lowest voltage and the step, p.u.; blank for the defaults. */
  vHigh: string;
  vLow: string;
  step: string;
}

export const DEFAULT_QV: QvChoice = { buses: "", vHigh: "", vLow: "", step: "" };

export function qvSettings(c: QvChoice): QvSettings {
  const s: QvSettings = { buses: busList(c.buses) };
  const high = positive(c.vHigh);
  const low = positive(c.vLow);
  const step = positive(c.step);
  if (high !== null) s.v_high_pu = high;
  if (low !== null) s.v_low_pu = low;
  if (step !== null) s.step_pu = step;
  return s;
}

/**
 * The `n` load buses with the lowest solved voltage, in `area` (or the
 * whole case when null): the usual first buses to check.
 */
export function lowestVoltageBuses(buses: BusRow[], area: number | null, n: number): number[] {
  return buses
    .filter((b) => b.kind === "PQ" && b.pd > 0 && b.vm > 0 && (area === null || b.area === area))
    .sort((a, b) => a.vm - b.vm || a.id - b.id)
    .slice(0, n)
    .map((b) => b.id);
}

/** Every point of every curve as CSV rows. */
export function qvRows(r: QvResults): Record<string, number | string>[] {
  return r.curves.flatMap((c) =>
    c.v_pu.map((v, i) => ({
      bus: c.bus,
      name: c.name.trim(),
      v_pu: v,
      q_mvar: c.q_mvar[i],
      q_limited: c.q_limited[i],
    })),
  );
}

interface QvState {
  choice: QvChoice;
  setChoice: (c: Partial<QvChoice>) => void;
  results: QvResults | null;
  running: boolean;
  progress: ContingencyProgress | null;
  error: string | null;
  run: () => Promise<boolean>;
  cancel: () => void;
  clear: () => void;
}

export const useQv = create<QvState>((set, get) => ({
  choice: DEFAULT_QV,
  setChoice: (c) => set((s) => ({ choice: { ...s.choice, ...c } })),
  results: null,
  running: false,
  progress: null,
  error: null,
  async run() {
    set({ running: true, error: null, progress: null });
    const timer = setInterval(() => {
      api
        .qvProgress()
        .then((p) => get().running && set({ progress: p }))
        .catch(() => undefined);
    }, 250);
    try {
      const results = await api.runQvCurve(qvSettings(get().choice));
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
    void api.cancelQvCurve();
  },
  clear: () => set({ results: null, error: null }),
}));
