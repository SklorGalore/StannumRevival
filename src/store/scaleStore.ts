// The scaling wizard's choices: which buses, and how their load and
// generation scale. Kept while the app is open, so the wizard opens where it
// was left.
import { create } from "zustand";
import type { ScaleAmount, ScaleLoadParts, ScaleParticipation, ScaleSettings } from "../lib/analysis/commands.gen";

export type AmountKind = ScaleAmount["kind"];

export interface ScaleChoice {
  areas: number[];
  zones: number[];
  owners: number[];
  kvMin: number | null;
  kvMax: number | null;

  scaleLoad: boolean;
  mwKind: AmountKind;
  mw: number | null;
  /** Scale Mvar on its own (when the power factor isn't held). */
  scaleMvar: boolean;
  mvarKind: AmountKind;
  mvar: number | null;
  constantPowerFactor: boolean;
  parts: ScaleLoadParts;
  includeNegative: boolean;

  scaleGeneration: boolean;
  genKind: AmountKind;
  gen: number | null;
  respectLimits: boolean;
  participation: ScaleParticipation;

  solveAfter: boolean;
}

export const DEFAULT_SCALE_CHOICE: ScaleChoice = {
  areas: [],
  zones: [],
  owners: [],
  kvMin: null,
  kvMax: null,
  scaleLoad: true,
  mwKind: "percent",
  mw: null,
  scaleMvar: false,
  mvarKind: "percent",
  mvar: null,
  constantPowerFactor: true,
  parts: "all",
  includeNegative: false,
  scaleGeneration: false,
  genKind: "percent",
  gen: null,
  respectLimits: true,
  participation: "output",
  solveAfter: true,
};

const amount = (kind: AmountKind, value: number): ScaleAmount => ({ kind, value });

/**
 * The core's settings for a choice, or why it isn't ready yet. Load MW left
 * blank leaves MW alone (only Mvar scales); the same for each amount.
 */
export function scaleSettings(c: ScaleChoice): { settings: ScaleSettings } | { problem: string } {
  const settings: ScaleSettings = {
    buses: {
      areas: c.areas,
      zones: c.zones,
      owners: c.owners,
      kv_min: c.kvMin,
      kv_max: c.kvMax,
    },
  };
  if (c.kvMin !== null && c.kvMax !== null && c.kvMin > c.kvMax) return { problem: "The kV range runs backwards: From is above To." };
  if (c.scaleLoad) {
    const mw = c.mw !== null ? amount(c.mwKind, c.mw) : null;
    const mvar = !c.constantPowerFactor && c.scaleMvar && c.mvar !== null ? amount(c.mvarKind, c.mvar) : null;
    if (!mw && !mvar) return { problem: c.constantPowerFactor || !c.scaleMvar ? "Enter how much to scale load MW by." : "Enter how much to scale load MW or Mvar by." };
    settings.load = {
      mw,
      mvar,
      constant_power_factor: c.constantPowerFactor,
      parts: c.parts,
      include_negative: c.includeNegative,
    };
  }
  if (c.scaleGeneration) {
    if (c.gen === null) return { problem: "Enter how much to scale generation by." };
    settings.generation = {
      mw: amount(c.genKind, c.gen),
      respect_limits: c.respectLimits,
      participation: c.participation,
    };
  }
  if (!settings.load && !settings.generation) return { problem: "Choose load, generation or both to scale." };
  return { settings };
}

const KIND_WORDS: Record<AmountKind, (v: number, unit: string) => string> = {
  target: (v, unit) => `to ${v} ${unit}`,
  change: (v, unit) => `by ${v >= 0 ? "+" : ""}${v} ${unit}`,
  percent: (v) => `by ${v >= 0 ? "+" : ""}${v}%`,
};

/** The buses a choice picks, in a few words. */
export function describeBuses(c: ScaleChoice): string {
  const list = (name: string, v: number[]) => (v.length ? `${name}${v.length === 1 ? "" : "s"} ${v.join(", ")}` : null);
  const kv =
    c.kvMin !== null && c.kvMax !== null
      ? `${c.kvMin}–${c.kvMax} kV`
      : c.kvMin !== null
        ? `${c.kvMin} kV and up`
        : c.kvMax !== null
          ? `up to ${c.kvMax} kV`
          : null;
  const parts = [list("area", c.areas), list("zone", c.zones), list("owner", c.owners), kv].filter(Boolean);
  return parts.length ? parts.join(", ") : "every bus";
}

/** The whole choice in one line, for the wizard's footer. */
export function describeScaling(c: ScaleChoice): string {
  const parts: string[] = [];
  if (c.scaleLoad) {
    const bits: string[] = [];
    if (c.mw !== null) bits.push(`MW ${KIND_WORDS[c.mwKind](c.mw, "MW")}`);
    if (c.constantPowerFactor) bits.push("constant power factor");
    else if (c.scaleMvar && c.mvar !== null) bits.push(`Mvar ${KIND_WORDS[c.mvarKind](c.mvar, "Mvar")}`);
    parts.push(`Load ${bits.join(", ")}`);
  }
  if (c.scaleGeneration && c.gen !== null) parts.push(`Generation ${KIND_WORDS[c.genKind](c.gen, "MW")}${c.respectLimits ? " within limits" : ", ignoring Pmax"}`);
  return `${parts.join(" · ") || "Nothing to scale"} · ${describeBuses(c)}`;
}

interface ScaleState {
  open: boolean;
  choice: ScaleChoice;
  setOpen: (open: boolean) => void;
  setChoice: (patch: Partial<ScaleChoice>) => void;
  reset: () => void;
}

export const useScaling = create<ScaleState>((set) => ({
  open: false,
  choice: DEFAULT_SCALE_CHOICE,
  setOpen: (open) => set({ open }),
  setChoice: (patch) => set((s) => ({ choice: { ...s.choice, ...patch } })),
  reset: () => set({ choice: DEFAULT_SCALE_CHOICE }),
}));
