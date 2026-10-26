// Layers: topology findings and field rules drawn on the Diagram, in either
// style. Each layer marks elements by their shared id (a bus number, or a
// branch's element id); the Network style and the One-line style both draw
// the same marks. The contingency layer draws the outage open in Power
// flow's Contingencies tab.
import type { ContingencyDetail, SolveReport, TopoPath, TopologySnapshot } from "../analysis/types";
import { elementLabel } from "../elements";
import { createColorPresentation, type ColorSettings } from "../../modules/topology/fields/field-colors";
import { matchesBus, matchesEquipment, type FilterRule } from "../../modules/topology/fields/field-view";

export type LayerId = "fields" | "islands" | "results" | "merged" | "critical" | "path" | "contingency";

/** How one layer marks an element. Later layers win where they overlap. */
export interface Mark {
  /** Fill of a bus, or stroke of a branch. */
  color?: string;
  /** A ring drawn around the element. */
  halo?: string;
  /** Drawn heavier (a path). */
  bold?: boolean;
  /** Filtered out: faded in the one-line, hidden in the network. */
  dim?: boolean;
  /** Why it is marked, shown on hover. */
  note?: string;
}

export interface LayerMarks {
  buses: Map<number, Mark>;
  /** Branches by shared element id. */
  elements: Map<string, Mark>;
  /** Graph nodes that aren't buses (three-winding junctions), by node id. */
  nodes: Map<string, Mark>;
}

export interface LegendEntry {
  label: string;
  color: string;
  kind: "fill" | "halo" | "line" | "gradient";
  /** For a gradient: the other end's colour. */
  to?: string;
}

export interface LayerLegend {
  id: LayerId;
  title: string;
  entries: LegendEntry[];
}

/** Colours shared by both drawing styles. */
export const LAYER_COLORS = {
  critical: "#b45309",
  path: "#2563eb",
  merged: "#7c3aed",
  tripped: "#dc2626",
  overload: "#dc2626",
  more: "#ea580c",
  less: "#0284c7",
  voltage: "#d97706",
};

/** Island colours, largest island first. */
const ISLAND_COLORS = ["#0072b2", "#e69f00", "#009e73", "#d55e00", "#56b4e9", "#cc79a7", "#f0e442", "#333333"];

export const LAYER_ORDER: LayerId[] = ["fields", "islands", "results", "merged", "critical", "path", "contingency"];

export const LAYER_TITLES: Record<LayerId, string> = {
  critical: "Bridges and cut buses",
  islands: "Islands",
  merged: "Zero-impedance groups",
  path: "Shortest path",
  fields: "Filters and colours",
  results: "Colour by result",
  contingency: "Contingency",
};

/** What the Results layer colours by. */
export type ResultMode = "loading" | "voltage" | "change" | "sensitivity";

/** A sensitivity column, as shown in Power flow's Sensitivities tab. */
export interface SensitivityLayer {
  /** Such as "Voltage angle per MW injected at bus 1023". */
  title: string;
  /** A few words for the legend, such as "MW injected at bus 1023". */
  short: string;
  units: string;
  rows: { element: string; value: number }[];
}

/** The Results layer's inputs. */
export interface ResultInput {
  mode: ResultMode;
  /** The latest solve. */
  report: SolveReport | null;
  /** For "change": the saved solve to compare the latest one with. */
  saved?: { name: string; report: SolveReport } | null;
  sensitivity?: SensitivityLayer | null;
}

export interface LayerInput {
  on: Record<LayerId, boolean>;
  path: TopoPath | null;
  rules: FilterRule[];
  colors: ColorSettings;
  /** The outage open in the Contingencies tab, while shown on the diagram. */
  contingency?: { detail: ContingencyDetail; limitPct: number } | null;
  results?: ResultInput | null;
}

function empty(): LayerMarks {
  return { buses: new Map(), elements: new Map(), nodes: new Map() };
}

function put<K>(map: Map<K, Mark>, key: K, mark: Mark) {
  const old = map.get(key);
  map.set(key, old ? { ...old, ...mark, note: [old.note, mark.note].filter(Boolean).join(" · ") || undefined } : mark);
}

/** One layer's marks and legend. */
export function layerMarks(id: LayerId, s: TopologySnapshot, input: LayerInput): { marks: LayerMarks; legend: LegendEntry[] } {
  const marks = empty();
  const legend: LegendEntry[] = [];
  const { analysis: a, graph } = s;
  if (id === "critical") {
    for (const b of a.bridges) put(marks.elements, b.equipment_id, { halo: LAYER_COLORS.critical, note: "Bridge: taking it out splits its island" });
    for (const bus of a.articulation_buses) put(marks.buses, bus, { halo: LAYER_COLORS.critical, note: "Cut bus: losing it splits its island" });
    for (const node of a.synthetic_articulations) put(marks.nodes, node, { halo: LAYER_COLORS.critical });
    legend.push({ label: `${a.bridges.length} bridges`, color: LAYER_COLORS.critical, kind: "line" });
    legend.push({ label: `${a.articulation_buses.length} cut buses`, color: LAYER_COLORS.critical, kind: "halo" });
  } else if (id === "islands") {
    const islands = [...a.islands].sort((x, y) => y.buses.length - x.buses.length);
    islands.forEach((island, i) => {
      const color = ISLAND_COLORS[i % ISLAND_COLORS.length];
      for (const bus of island.buses) put(marks.buses, bus, { color, note: `Island ${island.id}` });
      if (i < 8) legend.push({ label: `Island ${island.id} · ${island.buses.length} buses`, color, kind: "fill" });
    });
    if (islands.length > 8) legend.push({ label: `+${islands.length - 8} more islands`, color: "#94a3b8", kind: "fill" });
  } else if (id === "merged") {
    const groups = graph.zero_impedance_groups.filter((g) => g.length > 1);
    for (const g of groups)
      for (const bus of g) put(marks.buses, bus, { halo: LAYER_COLORS.merged, note: `Tied by zero-impedance branches to ${g.filter((b) => b !== bus).join(", ")}` });
    legend.push({ label: `${groups.length} groups of buses tied by zero impedance`, color: LAYER_COLORS.merged, kind: "halo" });
  } else if (id === "path" && input.path) {
    for (const bus of input.path.buses) put(marks.buses, bus, { color: LAYER_COLORS.path, bold: true, note: "On the shortest path" });
    for (const e of input.path.equipment_ids) put(marks.elements, e, { color: LAYER_COLORS.path, bold: true, note: "On the shortest path" });
    legend.push({ label: `${input.path.buses[0]} → ${input.path.buses[input.path.buses.length - 1]}, ${input.path.hops} hops`, color: LAYER_COLORS.path, kind: "line" });
  } else if (id === "results" && input.results) {
    resultMarks(input.results, marks, legend);
  } else if (id === "contingency" && input.contingency) {
    contingencyMarks(input.contingency.detail, input.contingency.limitPct, marks, legend);
  } else if (id === "fields") {
    const rules = input.rules;
    if (rules.length) {
      for (const bus of s.case.buses) if (!matchesBus(s, bus.number, rules)) put(marks.buses, bus.number, { dim: true });
      for (const e of s.case.equipment) if (!matchesEquipment(s, e.id, rules)) put(marks.elements, e.id, { dim: true });
      legend.push({ label: `${rules.length} filter${rules.length === 1 ? "" : "s"}: others faded or hidden`, color: "#94a3b8", kind: "fill" });
    }
    const p = createColorPresentation(s, input.colors);
    const nodes = new Map(graph.nodes.map((n) => [n.id, n]));
    for (const [id, color] of Object.entries(p.nodeColors)) {
      const node = nodes.get(id);
      const members = node?.member_buses?.length ? node.member_buses : node?.bus != null ? [node.bus] : [];
      for (const bus of members) put(marks.buses, bus, { color });
    }
    const edges = new Map(graph.edges.map((e) => [e.id, e.equipment_id]));
    for (const [id, color] of Object.entries(p.edgeColors)) {
      const eq = edges.get(id);
      if (eq) put(marks.elements, eq, { color });
    }
    for (const l of p.legends) {
      if (l.gradient) legend.push({ label: `${l.label} ${round(l.gradient.min)}–${round(l.gradient.max)}`, color: l.gradient.low, to: l.gradient.high, kind: "gradient" });
      else for (const e of l.entries.slice(0, 10)) legend.push({ label: `${l.label}: ${e.label}`, color: e.color, kind: l.target === "bus" ? "fill" : "line" });
    }
  }
  return { marks, legend };
}

/** Flow changes smaller than this (MW) aren't coloured. */
const MOVED_MW = 5;

/** The outage, what it overloads, and where flow moved: drawn on the diagram. */
function contingencyMarks(d: ContingencyDetail, limit: number, marks: LayerMarks, legend: LegendEntry[]) {
  const outage = d.row.element;
  const cut = new Set(d.cut_off);
  for (const bus of d.cut_off) put(marks.buses, bus, { dim: true, note: "Split off by the outage" });
  let more = 0;
  let less = 0;
  for (const b of d.branches) {
    const delta = b.mw - b.base_mw;
    if (b.tripped || cut.has(b.from) || cut.has(b.to)) continue;
    const pct = b.pct === null ? "" : `, ${b.pct.toFixed(0)}% of rate A (was ${b.base_pct?.toFixed(0) ?? "—"}%)`;
    const note = `After ${elementLabel(outage)}: ${b.mw.toFixed(1)} MW (${delta >= 0 ? "+" : ""}${delta.toFixed(1)})${pct}`;
    if (b.pct !== null && b.pct > limit && (b.base_pct === null || b.base_pct <= limit || b.pct > b.base_pct + 1)) {
      put(marks.elements, b.element, { color: LAYER_COLORS.overload, bold: true, note });
    } else if (Math.abs(delta) >= MOVED_MW) {
      // Flow moved in its own direction (heavier) or against it (lighter).
      const heavier = Math.abs(b.mw) > Math.abs(b.base_mw);
      if (heavier) more++;
      else less++;
      put(marks.elements, b.element, { color: heavier ? LAYER_COLORS.more : LAYER_COLORS.less, note });
    }
  }
  if (d.row.kind === "generator") {
    const bus = Number(outage.split(":")[1]);
    if (Number.isInteger(bus)) put(marks.buses, bus, { halo: LAYER_COLORS.tripped, note: `${elementLabel(outage)} taken out` });
  } else {
    put(marks.elements, outage, { halo: LAYER_COLORS.tripped, dim: true, note: `${elementLabel(outage)} taken out` });
  }
  for (const v of d.voltage_problems)
    put(marks.buses, v.bus, { halo: LAYER_COLORS.voltage, note: `${v.vm.toFixed(3)} p.u. after the outage (was ${v.base_vm.toFixed(3)})` });
  legend.push({ label: `Out: ${elementLabel(outage)}`, color: LAYER_COLORS.tripped, kind: "halo" });
  legend.push({ label: `${d.overloads.length} over ${limit.toFixed(0)}%, new or worse`, color: LAYER_COLORS.overload, kind: "line" });
  legend.push({ label: `${more} carry ${MOVED_MW}+ MW more`, color: LAYER_COLORS.more, kind: "line" });
  legend.push({ label: `${less} carry ${MOVED_MW}+ MW less`, color: LAYER_COLORS.less, kind: "line" });
  if (d.voltage_problems.length) legend.push({ label: `${d.voltage_problems.length} buses outside voltage limits`, color: LAYER_COLORS.voltage, kind: "halo" });
  if (d.cut_off.length) legend.push({ label: `${d.cut_off.length} buses split off (faded)`, color: "#94a3b8", kind: "fill" });
}

/** Branch loading bands, % of rate A: upper bound, colour, words. */
export const LOADING_BANDS: [number, string, string][] = [
  [50, "#16a34a", "under 50%"],
  [80, "#84cc16", "50–80%"],
  [90, "#eab308", "80–90%"],
  [100, "#f97316", "90–100%"],
  [Infinity, "#dc2626", "over 100%"],
];

/** Bus voltage bands, p.u.: upper bound, colour, words. */
export const VOLTAGE_BANDS: [number, string, string][] = [
  [0.95, "#1d4ed8", "under 0.95 p.u."],
  [0.98, "#60a5fa", "0.95–0.98"],
  [1.02, "#94a3b8", "0.98–1.02"],
  [1.05, "#f59e0b", "1.02–1.05"],
  [Infinity, "#dc2626", "over 1.05 p.u."],
];

/** Diverging ends for a signed result: negative, middle, positive. */
const NEGATIVE = "#2563eb";
const MIDDLE = "#cbd5e1";
const POSITIVE = "#dc2626";

/** Responses smaller than this share of the largest aren't coloured. */
const SMALL_SHARE = 0.01;

function band(value: number, bands: [number, string, string][]): number {
  return bands.findIndex(([upper]) => value < upper);
}

/** Blend two #rrggbb colours, t from 0 (a) to 1 (b). */
export function mix(a: string, b: string, t: number): string {
  const p = (c: string, i: number) => parseInt(c.slice(1 + 2 * i, 3 + 2 * i), 16);
  const ch = (i: number) => Math.round(p(a, i) + (p(b, i) - p(a, i)) * Math.min(1, Math.max(0, t)));
  return `#${[0, 1, 2].map((i) => ch(i).toString(16).padStart(2, "0")).join("")}`;
}

/** A signed value on the blue–grey–red scale, scaled by `max`. */
function diverging(value: number, max: number): string {
  const t = max > 0 ? Math.min(1, Math.abs(value) / max) : 0;
  return value < 0 ? mix(MIDDLE, NEGATIVE, t) : mix(MIDDLE, POSITIVE, t);
}

/** Colour by the latest solve, a change from a saved solve, or a sensitivity. */
function resultMarks(r: ResultInput, marks: LayerMarks, legend: LegendEntry[]) {
  const report = r.report;
  if (r.mode === "loading" && report) {
    const counts = LOADING_BANDS.map(() => 0);
    for (const b of report.branches) {
      if (!b.online || b.loading_pct === null) continue;
      const i = band(b.loading_pct, LOADING_BANDS);
      counts[i]++;
      put(marks.elements, b.element, {
        color: LOADING_BANDS[i][1],
        bold: b.loading_pct >= 100,
        note: `${b.loading_pct.toFixed(0)}% of rate A (${Math.max(Math.abs(b.pf), Math.abs(b.pt)).toFixed(1)} MW)`,
      });
    }
    LOADING_BANDS.forEach(([, color, words], i) => counts[i] && legend.push({ label: `${counts[i]} ${words}`, color, kind: "line" }));
    const unrated = report.branches.filter((b) => b.online && b.loading_pct === null).length;
    if (unrated) legend.push({ label: `${unrated} without a rating (not coloured)`, color: "#e2e8f0", kind: "line" });
  } else if (r.mode === "voltage" && report) {
    const counts = VOLTAGE_BANDS.map(() => 0);
    let outside = 0;
    for (const b of report.buses) {
      if (b.kind === "Isolated") continue;
      const i = band(b.vm, VOLTAGE_BANDS);
      counts[i]++;
      const out = b.vm < b.vmin - 1e-9 || b.vm > b.vmax + 1e-9;
      if (out) outside++;
      put(marks.buses, b.id, {
        color: VOLTAGE_BANDS[i][1],
        halo: out ? LAYER_COLORS.voltage : undefined,
        note: `${b.vm.toFixed(3)} p.u.${out ? ` (limits ${b.vmin.toFixed(2)}–${b.vmax.toFixed(2)})` : ""}`,
      });
    }
    VOLTAGE_BANDS.forEach(([, color, words], i) => counts[i] && legend.push({ label: `${counts[i]} ${words}`, color, kind: "fill" }));
    if (outside) legend.push({ label: `${outside} outside their own limits`, color: LAYER_COLORS.voltage, kind: "halo" });
  } else if (r.mode === "change" && report && r.saved) {
    const before = new Map(r.saved.report.branches.map((b) => [b.element + "#" + b.winding, b]));
    let more = 0;
    let less = 0;
    let switched = 0;
    for (const b of report.branches) {
      const was = before.get(b.element + "#" + b.winding);
      if (!was) continue;
      if (was.online !== b.online) {
        switched++;
        put(marks.elements, b.element, { halo: LAYER_COLORS.tripped, dim: !b.online, note: `${b.online ? "Back in service" : "Out of service"} since ${r.saved.name}` });
        continue;
      }
      if (!b.online) continue;
      const delta = b.pf - was.pf;
      if (Math.abs(delta) < MOVED_MW) continue;
      const heavier = Math.abs(b.pf) > Math.abs(was.pf);
      if (heavier) more++;
      else less++;
      put(marks.elements, b.element, {
        color: heavier ? LAYER_COLORS.more : LAYER_COLORS.less,
        note: `${b.pf.toFixed(1)} MW, ${delta >= 0 ? "+" : ""}${delta.toFixed(1)} since ${r.saved.name}`,
      });
    }
    const vmWas = new Map(r.saved.report.buses.map((b) => [b.id, b.vm]));
    let moved = 0;
    for (const b of report.buses) {
      const was = vmWas.get(b.id);
      if (was === undefined || Math.abs(b.vm - was) < VOLTAGE_MOVED) continue;
      moved++;
      put(marks.buses, b.id, { halo: b.vm > was ? POSITIVE : NEGATIVE, note: `${b.vm.toFixed(3)} p.u. (was ${was.toFixed(3)} in ${r.saved.name})` });
    }
    legend.push({ label: `Against ${r.saved.name}`, color: "#94a3b8", kind: "fill" });
    legend.push({ label: `${more} carry ${MOVED_MW}+ MW more`, color: LAYER_COLORS.more, kind: "line" });
    legend.push({ label: `${less} carry ${MOVED_MW}+ MW less`, color: LAYER_COLORS.less, kind: "line" });
    if (switched) legend.push({ label: `${switched} switched in or out`, color: LAYER_COLORS.tripped, kind: "halo" });
    if (moved) legend.push({ label: `${moved} bus${moved === 1 ? "" : "es"} moved ${VOLTAGE_MOVED}+ p.u.`, color: POSITIVE, kind: "halo" });
  } else if (r.mode === "sensitivity" && r.sensitivity) {
    const rows = r.sensitivity.rows.filter((x) => x.element && Number.isFinite(x.value));
    const max = Math.max(0, ...rows.map((x) => Math.abs(x.value)));
    for (const x of rows) {
      if (Math.abs(x.value) < max * SMALL_SHARE) continue;
      const mark: Mark = { color: diverging(x.value, max), note: `${sigText(x.value)} ${r.sensitivity.units}` };
      if (x.element.startsWith("bus:")) put(marks.buses, Number(x.element.slice(4)), mark);
      else put(marks.elements, x.element, mark);
    }
    legend.push({ label: r.sensitivity.short, color: NEGATIVE, to: POSITIVE, kind: "gradient" });
    legend.push({ label: `${sigText(-max)} to ${sigText(max)} ${r.sensitivity.units}`, color: NEGATIVE, to: POSITIVE, kind: "gradient" });
  }
}

/** Bus voltage changes smaller than this (p.u.) aren't marked. */
const VOLTAGE_MOVED = 0.01;

function sigText(n: number): string {
  return Number(n.toPrecision(3)).toString();
}

function round(n: number): string {
  return Math.abs(n) >= 100 ? n.toFixed(0) : Number(n.toPrecision(3)).toString();
}

/** Every layer that is on, merged in order, with a legend per layer. */
export function resolveLayers(s: TopologySnapshot, input: LayerInput): { marks: LayerMarks; legends: LayerLegend[] } {
  const marks = empty();
  const legends: LayerLegend[] = [];
  for (const id of LAYER_ORDER) {
    if (!input.on[id]) continue;
    const one = layerMarks(id, s, input);
    for (const [k, m] of one.marks.buses) put(marks.buses, k, m);
    for (const [k, m] of one.marks.elements) put(marks.elements, k, m);
    for (const [k, m] of one.marks.nodes) put(marks.nodes, k, m);
    if (one.legend.length) legends.push({ id, title: LAYER_TITLES[id], entries: one.legend });
  }
  return { marks, legends };
}

export const NO_MARKS: LayerMarks = empty();
