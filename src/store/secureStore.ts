// The last secure transfer study: its settings, results, and the curves of
// the channel on screen. stannum-core runs it (secure.rs), solving every
// contingency at every step, and keeps every channel's values in a results
// file; the curves of one channel are read back from it on demand. The
// contingencies and monitors are the contingency setup's. A new case or a
// study change clears it, as for transfer limits.
import { create } from "zustand";
import { api } from "../lib/analysis/api";
import type { SolveSettings } from "../lib/analysis/types";
import type {
  Channel,
  ChannelCurves,
  Quantity,
  Scaling,
  SecureProgress,
  SecureResults,
  SecureTransferSettings,
} from "../lib/analysis/commands.gen";
import { contingencySettings, useContingencies } from "./contingencyStore";
import { subsystem, type Side } from "./transferStore";
import { positive } from "./pvStore";

export interface SecureChoice {
  sink: Side;
  allSource: boolean;
  source: Side;
  sinkScaling: Scaling;
  sourceScaling: Scaling;
  /** The step while nothing new is found, MW; blank for 5% of the sink. */
  majorMw: string;
  /** The step that pins a problem down, MW; blank for a tenth of the major. */
  minorMw: string;
  /** Stop at this transfer, MW; blank to go on. */
  maxMw: string;
  stopThermal: boolean;
  stopVoltage: boolean;
  /** Stop at the first problem; off carries on to the nose. */
  stopAtViolation: boolean;
  respectGeneration: boolean;
  /** Quantities recorded as well as the sink's (or monitored) bus voltages. */
  channels: Channel[];
}

export const DEFAULT_SECURE: SecureChoice = {
  sink: { kind: "area", value: 0, buses: "" },
  allSource: false,
  source: { kind: "area", value: 0, buses: "" },
  sinkScaling: "load",
  sourceScaling: "generation",
  majorMw: "",
  minorMw: "",
  maxMw: "",
  stopThermal: true,
  stopVoltage: true,
  stopAtViolation: true,
  respectGeneration: true,
  channels: [],
};

/** What each quantity measures, and the kind of element it takes. */
export const QUANTITIES: { value: Quantity; label: string; element: string; placeholder: string }[] = [
  { value: "vm", label: "Bus voltage (p.u.)", element: "bus", placeholder: "bus:101" },
  { value: "va", label: "Bus angle (deg)", element: "bus", placeholder: "bus:101" },
  { value: "mw", label: "Branch MW", element: "branch", placeholder: "line:101:102:1" },
  { value: "mvar", label: "Branch Mvar", element: "branch", placeholder: "line:101:102:1" },
  { value: "pct", label: "Branch loading (%)", element: "branch", placeholder: "line:101:102:1" },
  { value: "pg", label: "Generator MW", element: "generator", placeholder: "generator:101:1" },
  { value: "qg", label: "Generator Mvar", element: "generator", placeholder: "generator:101:1" },
  { value: "interface", label: "Interface MW", element: "interface", placeholder: "interface name" },
];

/** The element id a typed entry stands for: a bare number is a bus. */
export function channelElement(quantity: Quantity, text: string): string {
  const t = text.trim();
  if ((quantity === "vm" || quantity === "va") && /^\d+$/.test(t)) return `bus:${t}`;
  return t;
}

export function secureSettings(c: SecureChoice, solve: SolveSettings, elements: string[] | null): SecureTransferSettings {
  return {
    sink: subsystem(c.sink),
    source: c.allSource ? {} : subsystem(c.source),
    sink_scaling: c.sinkScaling,
    source_scaling: c.sourceScaling,
    major_step_mw: positive(c.majorMw),
    minor_step_mw: positive(c.minorMw),
    max_transfer_mw: positive(c.maxMw),
    stop_on_thermal: c.stopThermal,
    stop_on_voltage: c.stopVoltage,
    stop_at_violation: c.stopAtViolation,
    respect_generation_limits: c.respectGeneration,
    contingencies: contingencySettings(useContingencies.getState().choice, solve, elements),
    channels: c.channels,
  };
}

/** How many curves the chart starts with: the base case and the most limiting. */
export const FIRST_CURVES = 8;

/** The contingencies drawn at first: the most limiting ones, then none. */
export function defaultPicks(r: SecureResults): number[] {
  return r.contingencies
    .filter((x) => x.index !== 0 && x.limit_mw !== null && x.limit_mw !== undefined)
    .slice(0, FIRST_CURVES - 1)
    .map((x) => x.index);
}

/** The curves of a channel as CSV rows: transfer, then one column per curve. */
export function curveRows(c: ChannelCurves): Record<string, number | string>[] {
  return c.transfer_mw.map((t, i) => {
    const row: Record<string, number | string> = { transfer_mw: t };
    for (const k of c.curves) row[k.name] = k.values[i] ?? "";
    return row;
  });
}

interface SecureState {
  choice: SecureChoice;
  setChoice: (c: Partial<SecureChoice>) => void;
  results: SecureResults | null;
  running: boolean;
  progress: SecureProgress | null;
  error: string | null;
  /** The channel on the chart (its position in the results' channels). */
  channel: number;
  /** Contingencies drawn beside the base case, by index. */
  picks: number[];
  curves: ChannelCurves | null;
  run: (solve: SolveSettings, elements: string[] | null) => Promise<boolean>;
  cancel: () => void;
  show: (channel: number, picks?: number[]) => Promise<void>;
  togglePick: (index: number) => Promise<void>;
  clear: () => void;
}

export const useSecure = create<SecureState>((set, get) => ({
  choice: DEFAULT_SECURE,
  setChoice: (c) => set((s) => ({ choice: { ...s.choice, ...c } })),
  results: null,
  running: false,
  progress: null,
  error: null,
  channel: 0,
  picks: [],
  curves: null,
  async run(solve, elements) {
    set({ running: true, error: null, progress: null });
    const timer = setInterval(() => {
      api
        .secureTransferProgress()
        .then((p) => get().running && set({ progress: p }))
        .catch(() => undefined);
    }, 400);
    try {
      const results = await api.runSecureTransfer(secureSettings(get().choice, solve, elements));
      set({ results, curves: null, channel: 0, picks: defaultPicks(results) });
      if (results.channels.length) await get().show(0);
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
    void api.cancelSecureTransfer();
  },
  async show(channel, picks) {
    const chosen = picks ?? get().picks;
    set({ channel, picks: chosen });
    try {
      const curves = await api.secureTransferChannel(channel, chosen);
      if (get().channel === channel && get().picks === chosen) set({ curves });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },
  async togglePick(index) {
    const picks = get().picks.includes(index) ? get().picks.filter((i) => i !== index) : [...get().picks, index];
    await get().show(get().channel, picks);
  },
  clear: () => set({ results: null, error: null, curves: null, picks: [], channel: 0 }),
}));
