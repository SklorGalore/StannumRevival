// The last PV curve study (continuation power flow): its settings and
// results. stannum-core runs it (pv.rs) and answers everything at once; a
// new case or a study change clears it, as for transfer limits.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import type { ContingencyProgress } from "../lib/analysis/types";
import type { PvResults, PvSettings, Scaling } from "../lib/analysis/commands.gen";
import { subsystem, type Side } from "./transferStore";

export interface PvChoice {
  sink: Side;
  /** Where the extra power comes from: every bus, or one picked side. */
  allSource: boolean;
  source: Side;
  sinkScaling: Scaling;
  sourceScaling: Scaling;
  /** The first step, MW; blank for 5% of what the sink starts with. */
  stepMw: string;
  /** Stop at this transfer, MW; blank to go on to the nose. */
  maxMw: string;
  /** Low voltage limit, p.u.; blank for each bus's own. */
  vminPu: string;
  respectGeneration: boolean;
}

export const DEFAULT_PV: PvChoice = {
  sink: { kind: "area", value: 0, buses: "" },
  allSource: true,
  source: { kind: "area", value: 0, buses: "" },
  sinkScaling: "load",
  sourceScaling: "generation",
  stepMw: "",
  maxMw: "",
  vminPu: "",
  respectGeneration: true,
};

/** A typed positive number, or null when blank or not one. */
export function positive(text: string): number | null {
  if (!text.trim()) return null;
  const v = Number(text);
  return Number.isFinite(v) && v > 0 ? v : null;
}

/** Whether a typed optional number is blank or a positive number. */
export function validPositive(text: string): boolean {
  return !text.trim() || positive(text) !== null;
}

export function pvSettings(c: PvChoice): PvSettings {
  return {
    sink: subsystem(c.sink),
    source: c.allSource ? {} : subsystem(c.source),
    sink_scaling: c.sinkScaling,
    source_scaling: c.sourceScaling,
    step_mw: positive(c.stepMw),
    max_transfer_mw: positive(c.maxMw),
    vmin_pu: positive(c.vminPu),
    respect_generation_limits: c.respectGeneration,
    max_curves: 8,
  };
}

/** The points as CSV rows, with one voltage column per curve's bus. */
export function pvRows(r: PvResults): Record<string, number | string>[] {
  return r.points.map((p, i) => {
    const row: Record<string, number | string> = {
      transfer_mw: p.transfer_mw,
      sink_load_mw: p.sink_load_mw,
      losses_mw: p.losses_mw,
      min_vm_pu: p.min_vm_pu,
      min_vm_bus: p.min_vm_bus,
      q_limited: p.q_limited,
      q_reserve_mvar: p.q_reserve_mvar,
    };
    for (const c of r.curves) row[`vm_${c.bus}`] = c.vm_pu[i];
    return row;
  });
}

interface PvState {
  choice: PvChoice;
  setChoice: (c: Partial<PvChoice>) => void;
  results: PvResults | null;
  running: boolean;
  progress: ContingencyProgress | null;
  error: string | null;
  run: () => Promise<boolean>;
  cancel: () => void;
  clear: () => void;
}

export const usePv = create<PvState>((set, get) => ({
  choice: DEFAULT_PV,
  setChoice: (c) => set((s) => ({ choice: { ...s.choice, ...c } })),
  results: null,
  running: false,
  progress: null,
  error: null,
  async run() {
    set({ running: true, error: null, progress: null });
    const timer = setInterval(() => {
      api
        .pvProgress()
        .then((p) => get().running && set({ progress: p }))
        .catch(() => undefined);
    }, 250);
    try {
      const results = await api.runPvCurve(pvSettings(get().choice));
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
    void api.cancelPvCurve();
  },
  clear: () => set({ results: null, error: null }),
}));
