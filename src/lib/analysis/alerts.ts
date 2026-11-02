// Alerts: the limit problems in one solve result, as critical (past a limit,
// counted as the overview's checks count them) or marginal (close to one),
// for the shared panel under every module.
import type { SolveReport } from "./types";
import { busElement } from "../elements";

export type AlertLevel = "critical" | "marginal";
export type AlertKind = "loading" | "low-voltage" | "high-voltage" | "reactive";

export interface Alert {
  level: AlertLevel;
  kind: AlertKind;
  element: string;
  /** Bus names, for the element's label. */
  names: string;
  /** What is wrong, in words. */
  condition: string;
  value: string;
  limit: string;
  /** How far past (or how close to) the limit, larger is worse; sorts within a level. */
  severity: number;
}

export interface AlertThresholds {
  /** Branch loading, percent of rate A, from which a branch is marginal. */
  loadingPct: number;
  /** Per unit inside a bus's voltage limits from which it is marginal. */
  voltageBandPu: number;
}

export const DEFAULT_THRESHOLDS: AlertThresholds = { loadingPct: 95, voltageBandPu: 0.01 };

const pct = (v: number) => `${v.toFixed(1)} %`;
const pu = (v: number) => `${v.toFixed(3)} pu`;

export function alertsFor(report: SolveReport, t: AlertThresholds = DEFAULT_THRESHOLDS): Alert[] {
  const out: Alert[] = [];
  const names = new Map<number, string>();
  for (const b of report.buses) if (b.name.trim()) names.set(b.id, b.name.trim());
  const busName = (id: number) => names.get(id) ?? "";

  for (const b of report.buses) {
    // Isolated buses carry no voltage; the core leaves them out of its check too.
    if (/isolated/i.test(b.kind) || !(b.vmax > b.vmin)) continue;
    const past = b.violation > 0;
    const low = past && b.vm < b.vmin;
    const high = past && !low;
    const nearLow = !low && b.vm < b.vmin + t.voltageBandPu;
    const nearHigh = !high && b.vm > b.vmax - t.voltageBandPu;
    if (!(low || high || nearLow || nearHigh)) continue;
    const under = low || nearLow;
    out.push({
      level: low || high ? "critical" : "marginal",
      kind: under ? "low-voltage" : "high-voltage",
      element: busElement(b.id),
      names: busName(b.id),
      condition: under ? "Under voltage" : "Over voltage",
      value: pu(b.vm),
      limit: pu(under ? b.vmin : b.vmax),
      severity: under ? b.vmin - b.vm : b.vm - b.vmax,
    });
  }

  for (const br of report.branches) {
    const loading = br.loading_pct;
    if (!br.online || loading === null || loading === undefined || loading < t.loadingPct) continue;
    const from = busName(br.from);
    const to = busName(br.to);
    out.push({
      // Past 100 % counts as an overload, as in the overview's check.
      level: loading > 100 ? "critical" : "marginal",
      kind: "loading",
      element: br.element,
      names: from || to ? `${from || br.from} – ${to || br.to}` : "",
      condition: loading > 100 ? "Overload" : "Loading",
      value: pct(loading),
      limit: `${br.rate_a.toFixed(1)} MVA`,
      severity: loading,
    });
  }

  for (const g of report.generators) {
    if (!g.online || g.q_violation <= 0) continue;
    const over = g.qg > g.qmax;
    out.push({
      level: "critical",
      kind: "reactive",
      element: g.element,
      names: g.bus_name.trim() || busName(g.bus),
      condition: over ? "Above Q max" : "Below Q min",
      value: `${g.qg.toFixed(1)} Mvar`,
      limit: `${(over ? g.qmax : g.qmin).toFixed(1)} Mvar`,
      severity: g.q_violation,
    });
  }

  const rank = (a: Alert) => (a.level === "critical" ? 0 : 1);
  // Critical first; within a level, overloads (percent) and voltages (pu) each worst first.
  return out.sort((a, b) => rank(a) - rank(b) || kindOrder(a) - kindOrder(b) || b.severity - a.severity);
}

function kindOrder(a: Alert): number {
  return a.kind === "loading" ? 0 : a.kind === "reactive" ? 2 : 1;
}

export function alertCounts(alerts: Alert[]): { critical: number; marginal: number } {
  let critical = 0;
  for (const a of alerts) if (a.level === "critical") critical++;
  return { critical, marginal: alerts.length - critical };
}
