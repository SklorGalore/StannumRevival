// What the map draws on top of its sites and lines: the Diagram's layer marks
// (the same ones the Network and One-line styles draw) and the shown result's
// flows. A site stands for several buses, so it takes the marks of its buses.
import type { SolveReport } from "../analysis/types";
import type { LayerMarks, Mark } from "../diagram/layers";
import { mw } from "../diagram/results";
import { elementLabel } from "../elements";
import type { MapModel } from "./model";

export interface SiteOverlay {
  /** Fill, from the first of its buses (highest voltage first) a layer colours. */
  color?: string;
  halo?: string;
  bold: boolean;
  /** Faded: every bus here is filtered out or split off. */
  dim: boolean;
  /** Layer notes and voltages, one line each, for the hover tip. */
  notes: string[];
}

export interface LineOverlay {
  color?: string;
  halo?: string;
  bold: boolean;
  dim: boolean;
  /** In service in the shown result (the case's own status without one). */
  online: boolean;
  /** Active power flow, MW, positive from `from` to `to`; null without flows. */
  mw: number | null;
  notes: string[];
}

export interface MapOverlay {
  sites: SiteOverlay[];
  lines: LineOverlay[];
  /** Largest flow on a drawn line, MW (0 without flows). */
  maxMw: number;
}

/** Flows smaller than this (MW) get no arrow. */
export const ARROW_MIN_MW = 1;

/**
 * The layer marks and shown result laid onto the map's sites and lines.
 * `flows` adds each line's MW and direction from the report.
 */
export function mapOverlay(model: MapModel, marks: LayerMarks, report: SolveReport | null, flows: boolean): MapOverlay {
  const vm = new Map(report?.buses.map((b) => [b.id, b]) ?? []);
  const sites = model.sites.map((site): SiteOverlay => {
    const notes: string[] = [];
    let color: string | undefined;
    let halo: string | undefined;
    let bold = false;
    let dim = site.buses.length > 0;
    const many = site.buses.length > 1;
    for (const bus of site.buses) {
      const m: Mark | undefined = marks.buses.get(bus);
      color ??= m?.color;
      halo ??= m?.halo;
      bold ||= !!m?.bold;
      if (!m?.dim) dim = false;
      const row = flows ? vm.get(bus) : undefined;
      const parts = [row && row.kind !== "Isolated" ? `${row.vm.toFixed(3)} p.u.` : null, m?.note ?? null].filter(Boolean);
      if (parts.length) notes.push(`${many ? `Bus ${bus}: ` : ""}${parts.join(" · ")}`);
    }
    return { color, halo, bold, dim, notes };
  });
  // A marked branch inside a substation (the outage, a bridge) marks its site.
  const seen = new Set<string>();
  for (const b of model.inner) {
    const m = marks.elements.get(b.element);
    if (!m || seen.has(b.element)) continue;
    seen.add(b.element);
    const site = sites[b.site];
    site.halo ??= m.halo ?? (m.bold ? m.color : undefined);
    if (m.note) site.notes.push(`${elementLabel(b.element)}: ${m.note}`);
  }

  const branchRows = new Map(report?.branches.map((b) => [`${b.element}#${b.winding}`, b]) ?? []);
  const dcRows = new Map(report?.dc_lines?.map((d) => [d.element, d]) ?? []);
  let maxMw = 0;
  const lines = model.lines.map((line): LineOverlay => {
    const m = marks.elements.get(line.element);
    const notes: string[] = [];
    let online = line.online;
    let flow: number | null = null;
    if (line.dc) {
      const row = dcRows.get(line.element);
      if (row) {
        online = row.online;
        if (flows && row.online) {
          flow = row.from === line.from ? row.pf : -row.pf;
          notes.push(`${mw(row.pf)} sent, ${mw(row.pt)} received`);
        }
      }
    } else {
      const row = branchRows.get(`${line.element}#${line.winding}`);
      if (row) {
        online = row.online;
        if (flows && row.online) {
          flow = row.from === line.from ? row.pf : -row.pf;
          // The loading layer's own note already gives the loading.
          const pct = m?.note?.includes("rate A") ? "" : row.loading_pct === null ? " · not rated" : ` · ${row.loading_pct.toFixed(0)}% of rate A`;
          notes.push(`${mw(Math.abs(row.pf))} from bus ${row.pf >= 0 ? row.from : row.to} to bus ${row.pf >= 0 ? row.to : row.from}${pct}`);
        }
      }
    }
    if (flows && report && !online) notes.push("Out of service in the shown result");
    if (m?.note) notes.push(m.note);
    if (flow !== null) maxMw = Math.max(maxMw, Math.abs(flow));
    return { color: m?.color, halo: m?.halo, bold: !!m?.bold, dim: !!m?.dim, online, mw: flow, notes };
  });
  return { sites, lines, maxMw };
}

/** An arrow's size for a flow: small for light flows, full size for the heaviest. */
export function arrowSize(mwAbs: number, maxMw: number): number {
  if (maxMw <= 0) return 0.8;
  return 0.8 + 0.8 * Math.sqrt(Math.min(1, mwAbs / maxMw));
}
