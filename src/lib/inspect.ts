// What the latest solve says about one element, for the element inspector.
// Pure functions over the solve report, so every module shows the same
// numbers the same way.
import type { SolveReport } from "./analysis/types";
import { fixed } from "./analysis/format";
import { elementBus, parseElement } from "./elements";

export type Tone = "ok" | "warn" | "bad";

export interface ResultRow {
  label: string;
  value: string;
  unit: string;
  tone?: Tone;
}

export interface ResultGroup {
  /** Empty when the element has one group. */
  title: string;
  rows: ResultRow[];
}

export interface ElementResults {
  /** One line for the header, such as "123% of rate A". */
  headline: string | null;
  tone: Tone;
  /** Loading in percent of rate A, for a bar. */
  loading: number | null;
  groups: ResultGroup[];
  /** Out of service in the solve. */
  offline: boolean;
}

const row = (label: string, value: number | null | undefined, unit: string, digits = 2, tone?: Tone): ResultRow => ({
  label,
  value: fixed(value, digits),
  unit,
  ...(tone ? { tone } : {}),
});

/** Loading tone: over 100% is a violation, over 90% is worth a look. */
export function loadingTone(pct: number | null): Tone {
  return pct === null ? "ok" : pct > 100 ? "bad" : pct > 90 ? "warn" : "ok";
}

function busResults(report: SolveReport, bus: number, withLoad: boolean): ElementResults | null {
  const b = report.buses.find((x) => x.id === bus);
  if (!b) return null;
  const tone: Tone = b.violation > 0 ? "bad" : "ok";
  const gens = report.generators.filter((g) => g.bus === bus && g.online);
  const rows: ResultRow[] = [
    row("Voltage", b.vm, "p.u.", 4, tone),
    ...(b.base_kv > 0 ? [row("Voltage", b.vm * b.base_kv, "kV", 2, tone)] : []),
    row("Angle", b.va, "°", 2),
  ];
  if (withLoad) rows.push(row("Load", b.pd, "MW", 1), row("Load", b.qd, "Mvar", 1));
  if (gens.length) {
    rows.push(
      row("Generation", gens.reduce((s, g) => s + g.pg, 0), "MW", 1),
      row("Generation", gens.reduce((s, g) => s + g.qg, 0), "Mvar", 1)
    );
  }
  const groups: ResultGroup[] = [{ title: "", rows }];
  if (withLoad && b.load_parts) {
    const { p, q } = b.load_parts;
    groups.push({
      title: "Load by type",
      rows: ["Constant power", "Constant current", "Constant impedance"].flatMap((label, k) => [
        row(label, p[k], "MW", 1),
        row(label, q[k], "Mvar", 1),
      ]),
    });
  }
  const headline =
    b.vm > b.vmax ? `${fixed(b.vm, 3)} p.u., above ${fixed(b.vmax, 2)}` : b.vm < b.vmin ? `${fixed(b.vm, 3)} p.u., below ${fixed(b.vmin, 2)}` : `${fixed(b.vm, 3)} p.u.`;
  return { headline, tone, loading: null, groups, offline: b.kind === "Isolated" };
}

/** The latest solve's results for an element, or null when the solve doesn't list it. */
export function elementResults(report: SolveReport, element: string): ElementResults | null {
  const e = parseElement(element);
  if (!e) return null;
  switch (e.kind) {
    case "bus":
      return busResults(report, e.buses[0], true);
    case "load": {
      // Power flow sums loads per bus, so a load shows its bus.
      const r = busResults(report, elementBus(element)!, true);
      return r && { ...r, groups: [{ title: `Bus ${e.buses[0]}`, rows: r.groups[0].rows }, ...r.groups.slice(1)] };
    }
    case "generator": {
      const g = report.generators.find((x) => x.element === element);
      if (!g) return null;
      if (!g.online) return { headline: "Out of service", tone: "ok", loading: null, groups: [], offline: true };
      const tone: Tone = g.q_violation > 0 ? "bad" : "ok";
      const qRange = `${fixed(g.qmin, 1)} to ${fixed(g.qmax, 1)}`;
      return {
        headline: g.q_violation > 0 ? `${fixed(g.qg, 1)} Mvar, outside ${qRange}` : `${fixed(g.pg, 1)} MW`,
        tone,
        loading: g.pmax > 0 ? (100 * g.pg) / g.pmax : null,
        offline: false,
        groups: [
          {
            title: "",
            rows: [
              row("Output", g.pg, "MW", 1),
              row("Reactive output", g.qg, "Mvar", 1, tone),
              { label: "Reactive range", value: qRange, unit: "Mvar" },
              row("Voltage setpoint", g.vg, "p.u.", 4),
            ],
          },
        ],
      };
    }
    case "line":
    case "transformer":
    case "switch": {
      const rows = report.branches.filter((x) => x.element === element);
      if (!rows.length) return null;
      if (rows.every((r) => !r.online))
        return { headline: e.kind === "switch" ? "Open" : "Out of service", tone: "ok", loading: null, groups: [], offline: true };
      const loading = rows.reduce<number | null>((m, r) => (r.loading_pct === null ? m : Math.max(m ?? 0, r.loading_pct)), null);
      const tone = loadingTone(loading);
      const groups = rows.map((r) => {
        const t = loadingTone(r.loading_pct);
        return {
          title: r.winding ? `Winding ${r.winding}` : "",
          rows: [
            row(`Flow at ${r.from}`, r.pf, "MW", 1),
            row(`Flow at ${r.from}`, r.qf, "Mvar", 1),
            row(`Flow at ${r.to}`, r.pt, "MW", 1),
            row(`Flow at ${r.to}`, r.qt, "Mvar", 1),
            row("Losses", r.loss_mw, "MW", 2),
            r.loading_pct === null
              ? { label: "Loading", value: "no rating", unit: "" }
              : row("Loading", r.loading_pct, `% of ${fixed(r.rate_a, 0)} MVA`, 1, t === "ok" ? undefined : t),
          ],
        };
      });
      return {
        headline: loading === null ? `${fixed(rows[0].pf, 1)} MW` : `${fixed(loading, 1)}% of rate A`,
        tone,
        loading,
        groups,
        offline: false,
      };
    }
    case "dcline": {
      const r = report.dc_lines?.find((x) => x.element === element);
      if (!r) return null;
      if (!r.online) return { headline: "Out of service", tone: "ok", loading: null, groups: [], offline: true };
      const tone: Tone = r.q_violation > 0 ? "bad" : "ok";
      return {
        headline: `${fixed(r.pf, 1)} MW sent`,
        tone,
        loading: null,
        offline: false,
        groups: [
          {
            title: "",
            rows: [
              row(`Sent from ${r.from}`, r.pf, "MW", 1),
              row(`Received at ${r.to}`, r.pt, "MW", 1),
              row("Losses", r.loss_mw, "MW", 2),
              row(`Reactive at ${r.from}`, r.qf, "Mvar", 1, r.q_violation > 0 ? "bad" : undefined),
              row(`Reactive at ${r.to}`, r.qt, "Mvar", 1, r.q_violation > 0 ? "bad" : undefined),
              row(`Voltage at ${r.from}`, r.vf, "p.u.", 4),
              row(`Voltage at ${r.to}`, r.vt, "p.u.", 4),
            ],
          },
        ],
      };
    }
    default:
      return null;
  }
}
