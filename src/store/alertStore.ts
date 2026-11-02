import { create } from "zustand";
import { DEFAULT_THRESHOLDS, type AlertThresholds } from "../lib/analysis/alerts";

const KEY = "stannum-alert-thresholds";

function load(): AlertThresholds {
  try {
    const t = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<AlertThresholds> | null;
    const loadingPct = Number(t?.loadingPct);
    const voltageBandPu = Number(t?.voltageBandPu);
    return {
      loadingPct: loadingPct > 0 && loadingPct <= 100 ? loadingPct : DEFAULT_THRESHOLDS.loadingPct,
      voltageBandPu: voltageBandPu >= 0 && voltageBandPu < 0.5 ? voltageBandPu : DEFAULT_THRESHOLDS.voltageBandPu,
    };
  } catch {
    return DEFAULT_THRESHOLDS;
  }
}

interface AlertState {
  /** Where marginal starts; kept on this computer. */
  thresholds: AlertThresholds;
  setThresholds: (t: Partial<AlertThresholds>) => void;
}

export const useAlerts = create<AlertState>((set, get) => ({
  thresholds: load(),
  setThresholds: (t) => {
    const thresholds = { ...get().thresholds, ...t };
    set({ thresholds });
    try {
      localStorage.setItem(KEY, JSON.stringify(thresholds));
    } catch {
      // Storage may be unavailable; the thresholds still hold for this session.
    }
  },
}));
