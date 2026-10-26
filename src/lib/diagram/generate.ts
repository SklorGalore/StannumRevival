// Build a one-line diagram from the open case: pick which buses to draw, lay
// them out with ELK's layered algorithm (orthogonal wires, top to bottom),
// and turn the result into diagram nodes and edges linked to case records.
import ELK from "elkjs/lib/elk-api.js";
import type { ElkExtendedEdge, ElkNode, ElkPoint } from "elkjs/lib/elk-api";
import workerUrl from "elkjs/lib/elk-worker.min.js?url";
import type { CaseNetwork, NetworkBranch, NetworkBus, NetworkDcLine } from "../analysis/types";
import type {
  BusData,
  BusTap,
  CaseRef,
  ConductorData,
  DcLinkData,
  GeneratorData,
  LoadData,
  OffPageData,
  OffPageLink,
  ShuntData,
  SLDEdge,
  SLDNode,
  TransformerData,
} from "../../store/types";
import { defaultGeneratorData, defaultLoadData, defaultTransformerData } from "../../store/defaults";
import { busElement, elementLabel, parseElement } from "../elements";
import { useLog } from "../../store/logStore";
import { bestFold, hopGroups } from "./compact";

export type Scope =
  | { kind: "all" }
  | { kind: "area"; area: number }
  /** `depth` hops around `bus`, or around `element` when given: a branch is
   *  drawn from both its ends, and a symbol on it is marked. */
  | { kind: "around"; bus: number; depth: number; element?: string };

/** Above this many buses a whole-case drawing stops being readable. */
export const READABLE_BUSES = 150;

// Symbol sizes must match the node components.
export const BUS_BAR = 6;
const GEN = { w: 40, h: 80 };
const LOAD = { w: 40, h: 60 };
const XFMR = { w: 40, h: 100 };
export const DC_LINK = { w: 40, h: 140 };
const OFF_PAGE = { w: 40, h: 40 };
const SHUNT = { w: 40, h: 60 };
/** Width each generator, load or shunt takes along its bus, name and results included. */
const SLOT = 100;
/** Room between wires that leave a bus carrying injections. */
const PORT_GAP = 44;
/** The bar runs this far past its last wire. */
const BAR_END = 22;
/** Space kept to the right of symbols for their name and results. */
const SIDE_LABEL = 96;
/** Room for a branch's flow results: three lines of small text. */
const FLOW_LABEL = { w: 64, h: 34 };

export const busLabel = (b: Pick<NetworkBus, "id" | "name">) => b.name.trim() || `Bus ${b.id}`;

/** A branch, or a DC line carried with the branches so that scopes, layout
 *  and off-page markers treat both alike. */
export type Link = NetworkBranch & { dc?: NetworkDcLine };

/** Keeps DC lines' `index` clear of branch indexes in symbol ids. */
const DC_INDEX = 1_000_000;

const linkCache = new WeakMap<CaseNetwork, Link[]>();
/** Every branch and DC line of the case. Networks without `dc_lines` (from
 *  an older backend) just have none. */
export function linksOf(net: CaseNetwork): Link[] {
  let links = linkCache.get(net);
  if (!links) {
    links = [
      ...net.branches,
      ...(net.dc_lines ?? []).map((d) => ({
        index: DC_INDEX + d.index,
        element: d.element,
        circuit: d.name,
        winding: 0,
        from: d.from,
        to: d.to,
        online: d.online,
        transformer: false,
        rate_a: 0,
        tap: 1,
        shift: 0,
        dc: d,
      })),
    ];
    linkCache.set(net, links);
  }
  return links;
}

/** The case record a branch or DC line stands for. */
export const linkRef = (br: Link): CaseRef =>
  br.dc ? { kind: "dcline", id: br.dc.index, element: br.element } : { kind: "branch", id: br.index, element: br.element };

/** Symbol data for a DC line drawn from `upper` down to `lower`. */
export function dcLinkData(d: NetworkDcLine, upperKV: number, lowerKV: number, reversed: boolean): DcLinkData {
  return {
    label: `${d.kind === "multi_terminal" ? "MTDC" : d.kind === "vsc" ? "VSC" : "DC"} ${d.name}`,
    scheduledMW: d.mw,
    upperKV,
    lowerKV,
    caseRef: { kind: "dcline", id: d.index, element: d.element },
    reversed: reversed || undefined,
    outOfService: !d.online || undefined,
    dcKind: d.kind && d.kind !== "two_terminal" ? d.kind : undefined,
  };
}

interface Adjacent {
  branch: Link;
  other: number;
}

function adjacency(net: CaseNetwork): Map<number, Adjacent[]> {
  const adj = new Map<number, Adjacent[]>();
  for (const b of net.buses) adj.set(b.id, []);
  for (const br of linksOf(net)) {
    adj.get(br.from)?.push({ branch: br, other: br.to });
    adj.get(br.to)?.push({ branch: br, other: br.from });
  }
  return adj;
}

/** The buses a local drawing grows from: the ends of a branch or DC line,
 *  else the one bus. */
export function centresOf(net: CaseNetwork, scope: Extract<Scope, { kind: "around" }>): number[] {
  const e = scope.element ? parseElement(scope.element) : null;
  const ends = e && (e.kind === "line" || e.kind === "transformer" || e.kind === "switch" || e.kind === "dcline") ? e.buses : [];
  const known = new Set(net.buses.filter((b) => !b.star).map((b) => b.id));
  const centres = [...new Set(ends.filter((b) => known.has(b)))];
  return centres.length > 1 ? centres : [scope.bus];
}

/** Buses to draw for a scope. Star points of three-winding transformers
 *  come along with their windings so a transformer is never cut in half. */
export function selectBuses(net: CaseNetwork, scope: Scope): Set<number> {
  const adj = adjacency(net);
  const star = new Set(net.buses.filter((b) => b.star).map((b) => b.id));
  if (scope.kind === "all") return new Set(net.buses.map((b) => b.id));
  if (scope.kind === "area") {
    const chosen = new Set(net.buses.filter((b) => b.area === scope.area && !b.star).map((b) => b.id));
    for (const id of [...chosen])
      for (const a of adj.get(id) ?? []) if (star.has(a.other)) chosen.add(a.other);
    return chosen;
  }
  // Breadth-first by hop count; passing through a star point is free.
  const centres = centresOf(net, scope);
  const depth = new Map<number, number>(centres.map((b) => [b, 0]));
  const queue = [...centres];
  while (queue.length) {
    const id = queue.shift()!;
    const d = depth.get(id)!;
    for (const a of adj.get(id) ?? []) {
      const next = star.has(a.other) ? d : d + 1;
      if (next > scope.depth && !star.has(a.other)) continue;
      if ((depth.get(a.other) ?? Infinity) <= next) continue;
      depth.set(a.other, next);
      if (star.has(a.other)) queue.unshift(a.other);
      else queue.push(a.other);
    }
  }
  return new Set(depth.keys());
}

/** Level of each bus in a breadth-first walk from `root` (for a local
 *  drawing, the studied bus), else from the reference or highest-voltage bus
 *  of each connected part. Branches point down the levels. */
function levels(buses: NetworkBus[], branches: NetworkBranch[], root?: number): Map<number, number> {
  const adj = new Map<number, number[]>();
  for (const b of buses) adj.set(b.id, []);
  for (const br of branches) {
    adj.get(br.from)!.push(br.to);
    adj.get(br.to)!.push(br.from);
  }
  const order = [...buses].sort(
    (a, b) =>
      Number(b.id === root) - Number(a.id === root) ||
      Number(b.kind === "Ref") - Number(a.kind === "Ref") ||
      b.base_kv - a.base_kv ||
      a.id - b.id
  );
  const level = new Map<number, number>();
  for (const root of order) {
    if (level.has(root.id)) continue;
    level.set(root.id, 0);
    const queue = [root.id];
    while (queue.length) {
      const id = queue.shift()!;
      for (const n of adj.get(id)!) {
        if (level.has(n)) continue;
        level.set(n, level.get(id)! + 1);
        queue.push(n);
      }
    }
  }
  return level;
}

const textWidth = (text: string, px = 6.6) => Math.ceil(text.length * px) + 6;
const roundUp = (v: number, step: number) => Math.ceil(v / step) * step;

const outside = (text: string, side: "H_LEFT" | "H_RIGHT", width: number, height = 28) => ({
  text,
  width,
  height,
  layoutOptions: { "elk.nodeLabels.placement": `OUTSIDE V_CENTER ${side}` },
});

const port = (id: string, x: number, y: number, side: "NORTH" | "SOUTH") => ({
  id,
  x,
  y,
  width: 0,
  height: 0,
  layoutOptions: { "elk.port.side": side },
});

const fixedPorts = { "elk.portConstraints": "FIXED_POS" };

let elk: InstanceType<typeof ELK> | null = null;
function layoutEngine() {
  // Layout runs in a worker so large cases don't freeze the window.
  elk ??= new ELK({ workerUrl });
  return elk;
}

let warmed = false;
/** Start the layout worker ahead of time: its first run costs most of a second. */
export function warmLayout() {
  if (warmed) return;
  warmed = true;
  void layoutEngine()
    .layout({
      id: "warm",
      layoutOptions: { "elk.algorithm": "layered", "elk.partitioning.activate": "true" },
      children: [
        { id: "a", width: 10, height: 10, layoutOptions: { "elk.partitioning.partition": "0" } },
        { id: "b", width: 10, height: 10, layoutOptions: { "elk.partitioning.partition": "1" } },
      ],
      edges: [{ id: "e", sources: ["a"], targets: ["b"] }],
    })
    .catch(() => {
      warmed = false;
    });
}

export interface Generated {
  nodes: SLDNode[];
  edges: SLDEdge[];
  buses: number;
  /** Branches to buses outside the scope, drawn as off-page markers. */
  boundary: number;
  /** Branches from the new buses to buses already drawn (see `Piece.drawn`). */
  links: Link[];
}

type EdgeSpec = {
  id: string;
  source: { node: string; port?: string; handle?: string };
  target: { node: string; port?: string; handle?: string };
  data: ConductorData;
};

/** What to lay out: the buses, and how a local drawing is anchored. */
export interface Piece {
  buses: Set<number>;
  /** Bus the layout grows from (whole-case drawings: the top row). */
  root?: number;
  /** Rows follow hop count from the centres (local drawings), folded to
   *  the shape of the view. */
  byHop?: boolean;
  /** Buses a local drawing grows from; `root` (or `anchor`) when left out. */
  centres?: number[];
  /** Share a single centre's neighbours between rows above and below it,
   *  so the centre ends up in the middle of the drawing. */
  twoSided?: boolean;
  /** The studied bus, drawn highlighted. */
  focus?: number;
  /** Case element (see lib/elements) whose symbol or wire is highlighted. */
  focusElement?: string;
  /** Width over height of the view the drawing should fill. */
  aspect?: number;
  /** A bus already on the drawing that new buses hang from (Expand). It is
   *  laid out only to anchor them: its injections keep their room on the bar
   *  but are not drawn again, and it gets no off-page markers. */
  anchor?: number;
  /** Buses already drawn: branches to them are returned in `links` rather
   *  than drawn as off-page markers. */
  drawn?: Set<number>;
}

/** Width over height of the diagram canvas on screen (or the window). */
export function viewAspect(): number {
  if (typeof document === "undefined") return 1.6;
  const el = [...document.querySelectorAll<HTMLElement>(".react-flow")].find((e) => e.offsetWidth > 0 && e.offsetHeight > 0);
  const w = el?.offsetWidth ?? window.innerWidth;
  const h = el?.offsetHeight ?? window.innerHeight;
  return h > 0 ? Math.min(4, Math.max(0.5, w / h)) : 1.6;
}

export async function generateDiagram(net: CaseNetwork, scope: Scope, aspect = viewAspect()): Promise<Generated> {
  const buses = selectBuses(net, scope);
  const where =
    scope.kind === "around"
      ? ` around ${scope.element && parseElement(scope.element)?.kind !== "bus" ? elementLabel(scope.element) : `bus ${scope.bus}`} (${scope.depth} hops)`
      : scope.kind === "area"
        ? ` for area ${scope.area}`
        : "";
  const end = useLog.getState().begin(`Laying out a one-line${where}`);
  try {
    let out: Generated;
    if (scope.kind !== "around") out = await layoutPiece(net, { buses, aspect });
    else {
      const centres = centresOf(net, scope);
      const kind = scope.element ? parseElement(scope.element)?.kind : "bus";
      out = await layoutPiece(net, {
        buses,
        root: centres[0],
        centres,
        byHop: true,
        twoSided: true,
        // A load is drawn on its bus, so the bus stands in for it.
        focus: kind === "bus" || kind === "load" || !kind ? scope.bus : undefined,
        focusElement: scope.element,
        aspect,
      });
    }
    end({ done: `Drew a one-line${where}: ${out.buses} buses, ${out.nodes.length} symbols, ${out.boundary} off-page links` });
    return out;
  } catch (e) {
    end({ error: (e as Error).message });
    throw e;
  }
}

/** What an off-page marker says: where its branch goes, or how many branches
 *  a bundled marker stands for. */
export function offPageLabel(links: OffPageLink[]): string {
  const one = (l: OffPageLink) =>
    (l.busName && l.busName !== `Bus ${l.bus}` ? `To ${l.busName} (${l.bus})` : `To Bus ${l.bus}`) +
    (l.caseRef?.kind === "dcline" ? " by DC" : "");
  if (links.length === 1) return one(links[0]);
  const far = new Set(links.map((l) => l.bus)).size;
  if (far === 1) return `${one(links[0])} ×${links.length}`;
  return `To ${far} buses${far < links.length ? ` (${links.length} branches)` : ""}`;
}

/** Marker data for the branches leaving `inside` (one, or a bundle). */
export function offPageData(links: OffPageLink[], voltageKV: number): OffPageData {
  const [first] = links;
  return {
    label: offPageLabel(links),
    bus: first.bus,
    busName: first.busName,
    voltageKV,
    ...(links.length === 1
      ? { caseRef: first.caseRef, reversed: first.reversed, outOfService: first.outOfService }
      : { links, outOfService: links.every((l) => l.outOfService) || undefined }),
  };
}

/** Estimated height of one folded row: a bus row and the row hanging below it. */
const ROW_HEIGHT = 240;
/** Space ELK keeps between neighbours in a row. */
const NODE_GAP = 36;

/** Lay out a set of buses with everything attached to them. */
export async function layoutPiece(net: CaseNetwork, piece: Piece): Promise<Generated> {
  const { anchor } = piece;
  const drawn = piece.drawn ?? new Set<number>();
  const chosen = new Set(piece.buses);
  if (anchor != null) chosen.add(anchor);
  const buses = net.buses.filter((b) => chosen.has(b.id));
  if (!buses.length) throw new Error("No buses match that selection.");
  const busById = new Map(net.buses.map((b) => [b.id, b]));
  // The anchor only joins the new buses; its other branches are already drawn.
  const touchesAnchor = (br: NetworkBranch) => br.from === anchor || br.to === anchor;
  const all = linksOf(net);
  const inner = all.filter((br) => chosen.has(br.from) && chosen.has(br.to));
  const crossing = all.filter((br) => chosen.has(br.from) !== chosen.has(br.to) && !touchesAnchor(br));
  const boundary = crossing.filter((br) => !drawn.has(chosen.has(br.from) ? br.to : br.from));
  const links = crossing.filter((br) => drawn.has(chosen.has(br.from) ? br.to : br.from));
  const gens = net.generators.filter((g) => chosen.has(g.bus));
  // Star points carry a nominal 1 kV; treat them as having no voltage of their own.
  const kv = (id: number) => {
    const b = busById.get(id);
    return !b || b.star ? 0 : b.base_kv;
  };
  const conductor = (voltageKV: number, extra: Partial<ConductorData> = {}): ConductorData => ({
    voltageKV,
    conductorType: voltageKV >= 100 ? "transmission" : "distribution",
    ...extra,
  });

  // Generators, loads and shunts sit right on their bus bar, side by side
  // from its left end. They are not laid out by ELK: their bus is made wide
  // and deep enough to hold them, and its wires leave from the right part.
  type Injection = { node: SLDNode; handle: string; height: number; data: ConductorData };
  const injections = new Map<number, Injection[]>();
  const inject = (bus: number, inj: Injection) => {
    const list = injections.get(bus) ?? [];
    list.push(inj);
    injections.set(bus, list);
  };
  const genCount = new Map<number, number>();
  for (const g of gens) genCount.set(g.bus, (genCount.get(g.bus) ?? 0) + 1);
  const genSeen = new Map<number, number>();
  for (const g of gens) {
    const n = (genSeen.get(g.bus) ?? 0) + 1;
    genSeen.set(g.bus, n);
    const id = `g${g.index}`;
    const data: GeneratorData = {
      ...defaultGeneratorData,
      // Named by bus, numbered when a bus has several machines.
      label: genCount.get(g.bus)! > 1 ? `G ${g.bus}.${n}` : `G ${g.bus}`,
      nominalMW: g.pmax,
      voltageKV: kv(g.bus),
      caseRef: { kind: "gen", id: g.index, element: g.element },
    };
    inject(g.bus, {
      node: { id, type: "generator", position: { x: 0, y: 0 }, data } as SLDNode,
      handle: "gen-top",
      height: GEN.h,
      data: conductor(kv(g.bus), { outOfService: !g.online || undefined }),
    });
  }
  for (const b of buses) {
    if (b.star) continue;
    if (b.pd !== 0 || b.qd !== 0) {
      const data: LoadData = {
        ...defaultLoadData,
        label: "Load",
        nominalMW: b.pd,
        nominalMVAR: b.qd,
        caseRef: { kind: "load", id: b.id, element: busElement(b.id) },
      };
      inject(b.id, {
        node: { id: `l${b.id}`, type: "load", position: { x: 0, y: 0 }, data } as SLDNode,
        handle: "load-top",
        height: LOAD.h,
        data: conductor(kv(b.id)),
      });
    }
    if (b.gs !== 0 || b.bs !== 0) {
      const data: ShuntData = {
        label: b.bs < 0 ? "Reactor" : "Shunt",
        nominalMVAR: b.bs,
        nominalMW: b.gs,
        caseRef: { kind: "shunt", id: b.id, element: busElement(b.id) },
      };
      inject(b.id, {
        node: { id: `s${b.id}`, type: "shunt", position: { x: 0, y: 0 }, data } as SLDNode,
        handle: "shunt-top",
        height: SHUNT.h,
        data: conductor(kv(b.id)),
      });
    }
  }

  // Branches leaving the drawing, by the bus they leave from. A bus with
  // several gets one marker for all of them, so a local drawing isn't
  // mostly off-page flags.
  const outward = new Map<number, OffPageLink[]>();
  for (const br of boundary) {
    const inside = chosen.has(br.from) ? br.from : br.to;
    const far = inside === br.from ? br.to : br.from;
    const farBus = busById.get(far);
    const list = outward.get(inside) ?? [];
    list.push({
      bus: far,
      busName: farBus ? busLabel(farBus) : "",
      caseRef: linkRef(br),
      // Flow is read at the inside end, so "reversed" means inside is "to".
      reversed: inside === br.to || undefined,
      outOfService: !br.online || undefined,
    });
    outward.set(inside, list);
  }

  // Wires at each bus, and how wide its bar is drawn for them.
  const degree = new Map<number, number>();
  const bump = (id: number) => degree.set(id, (degree.get(id) ?? 0) + 1);
  for (const br of inner) {
    bump(br.from);
    bump(br.to);
  }
  for (const inside of outward.keys()) bump(inside);
  const barWidth = (b: NetworkBus) => {
    if (b.star) return 20;
    const inj = injections.get(b.id);
    const wires = degree.get(b.id) ?? 0;
    return inj
      ? roundUp(Math.max(100, inj.length * SLOT + wires * PORT_GAP), 20)
      : Math.max(100, roundUp((wires + 1) * 44, 20));
  };
  const nameWidth = (b: NetworkBus) => Math.max(textWidth(busLabel(b), 7), textWidth(`${b.id} · ${b.base_kv} kV`, 6));

  // Rows: hop rows folded to the view's shape for local drawings, else
  // breadth-first levels that ELK is free to spread out.
  let rowOf: Map<number, number>;
  let fold: { groups: ReturnType<typeof hopGroups>["groups"]; width: (bus: number) => number; rows: number; estimate: { w: number; h: number } } | null =
    null;
  const aspect = piece.aspect ?? viewAspect();
  if (piece.byHop) {
    // How much row width each bus takes: its bar with name and results, or
    // the symbols hanging below it, whichever is wider.
    const below = new Map<number, number>();
    const hang = (bus: number, w: number) => below.set(bus, (below.get(bus) ?? 0) + w);
    for (const br of inner) {
      if (!br.dc && !br.transformer) continue;
      // Hangs from whichever end ends up higher; count half at each.
      const w = (XFMR.w + SIDE_LABEL + NODE_GAP) / 2;
      hang(br.from, w);
      hang(br.to, w);
    }
    for (const [inside, list] of outward)
      hang(inside, OFF_PAGE.w + Math.max(textWidth(offPageLabel(list)), SIDE_LABEL) + NODE_GAP);
    const unit = new Map(
      buses.map((b) => {
        const bar = b.star ? 20 + NODE_GAP : nameWidth(b) + barWidth(b) + 84 + BAR_END + NODE_GAP;
        return [b.id, Math.max(bar, below.get(b.id) ?? 0)];
      })
    );
    const width = (bus: number) => unit.get(bus) ?? 100;
    const centres = piece.centres ?? [piece.root ?? anchor ?? buses[0].id];
    const { groups } = hopGroups(
      buses.map((b) => b.id),
      inner.map((br) => ({ a: br.from, b: br.to })),
      centres,
      width,
      !!piece.twoSided
    );
    const fit = bestFold(groups, width, aspect, ROW_HEIGHT);
    rowOf = rowsToMap(fit.rows);
    fold = { groups, width, rows: fit.rows.length, estimate: { w: fit.width, h: fit.height } };
  } else {
    rowOf = levels(buses, inner, piece.root ?? anchor);
  }

  const build = (rowOf: Map<number, number>) => {
    // Local drawings put each row on its own ELK partition: buses on even
    // ones, what hangs from them (transformers, markers) on the odd one below.
    const row = (bus: number, below = false): Record<string, string> =>
      piece.byHop ? { "elk.partitioning.partition": String(2 * (rowOf.get(bus) ?? 0) + (below ? 1 : 0)) } : {};
    const nodes: ElkNode[] = [];
    const specs: EdgeSpec[] = [];
    const rfNodes = new Map<string, SLDNode>();

    for (const br of inner) {
      // Draw each branch from the bus nearer the top of the drawing.
      const [la, lb] = [rowOf.get(br.from) ?? 0, rowOf.get(br.to) ?? 0];
      const reversed = lb < la || (lb === la && kv(br.to) > kv(br.from));
      const [upper, lower] = reversed ? [br.to, br.from] : [br.from, br.to];
      const ref = linkRef(br);
      const outOfService = !br.online || undefined;
      if (br.dc) {
        const id = `dc${br.dc.index}`;
        nodes.push({
          id,
          width: DC_LINK.w,
          height: DC_LINK.h,
          ports: [port(`${id}:a`, DC_LINK.w / 2, 0, "NORTH"), port(`${id}:b`, DC_LINK.w / 2, DC_LINK.h, "SOUTH")],
          labels: [outside("DC", "H_RIGHT", SIDE_LABEL)],
          layoutOptions: { ...fixedPorts, ...row(upper, true) },
        });
        const data = dcLinkData(br.dc, kv(upper), kv(lower), reversed);
        rfNodes.set(id, { id, type: "dcLink", position: { x: 0, y: 0 }, data } as SLDNode);
        specs.push({
          id: `e-${id}-a`,
          source: { node: `b${upper}` },
          target: { node: id, port: `${id}:a`, handle: "dc-top" },
          data: conductor(kv(upper), { outOfService }),
        });
        specs.push({
          id: `e-${id}-b`,
          source: { node: id, port: `${id}:b`, handle: "dc-bottom" },
          target: { node: `b${lower}` },
          data: conductor(kv(lower), { outOfService }),
        });
      } else if (br.transformer) {
        const id = `t${br.index}`;
        nodes.push({
          id,
          width: XFMR.w,
          height: XFMR.h,
          ports: [port(`${id}:hv`, XFMR.w / 2, 0, "NORTH"), port(`${id}:lv`, XFMR.w / 2, XFMR.h, "SOUTH")],
          labels: [outside("T", "H_RIGHT", SIDE_LABEL)],
          layoutOptions: { ...fixedPorts, ...row(upper, true) },
        });
        const data: TransformerData = {
          ...defaultTransformerData,
          label: `T ${upper}-${lower}`,
          ratingMVA: br.rate_a,
          primaryKV: kv(upper),
          secondaryKV: kv(lower),
          windingsUnknown: true,
          caseRef: ref,
          reversed: reversed || undefined,
          outOfService,
        };
        rfNodes.set(id, { id, type: "transformer", position: { x: 0, y: 0 }, data } as SLDNode);
        specs.push({
          id: `e-${id}-hv`,
          source: { node: `b${upper}` },
          target: { node: id, port: `${id}:hv`, handle: "xfmr-primary" },
          data: conductor(kv(upper), { outOfService }),
        });
        specs.push({
          id: `e-${id}-lv`,
          source: { node: id, port: `${id}:lv`, handle: "xfmr-secondary" },
          target: { node: `b${lower}` },
          data: conductor(kv(lower), { outOfService }),
        });
      } else {
        specs.push({
          id: `e-br${br.index}`,
          source: { node: `b${upper}` },
          target: { node: `b${lower}` },
          data: conductor(kv(upper), { caseRef: ref, reversed: reversed || undefined, outOfService }),
        });
      }
    }

    for (const [inside, list] of outward) {
      const data = offPageData(list, kv(inside));
      const single = list.length === 1;
      const br0 = boundary.find((br) => linkRef(br).id === list[0].caseRef?.id && linkRef(br).kind === list[0].caseRef?.kind)!;
      // A single branch keeps the id it has always had; a bundle is named by its bus.
      const id = single ? `o${br0.index}` : `ob${inside}`;
      nodes.push({
        id,
        width: OFF_PAGE.w,
        height: OFF_PAGE.h,
        ports: [port(`${id}:p`, OFF_PAGE.w / 2, 0, "NORTH")],
        labels: [outside(data.label, "H_RIGHT", Math.max(textWidth(data.label), SIDE_LABEL))],
        layoutOptions: { ...fixedPorts, ...row(inside, true) },
      });
      rfNodes.set(id, { id, type: "offPage", position: { x: 0, y: 0 }, data } as SLDNode);
      specs.push({
        id: `e-${id}`,
        source: { node: `b${inside}` },
        target: { node: id, port: `${id}:p`, handle: "op-top" },
        data: conductor(
          kv(inside),
          single
            ? { caseRef: data.caseRef, reversed: data.reversed, outOfService: data.outOfService }
            : { outOfService: data.outOfService }
        ),
      });
    }

    // On a bus carrying injections every wire gets a port on the side it
    // leaves from; ELK orders them and packs them at the right end.
    const ports = new Map<string, { id: string; side: "NORTH" | "SOUTH" }[]>();
    const portOn = (end: EdgeSpec["source"], side: "NORTH" | "SOUTH") => {
      if (end.port || !injections.has(Number(end.node.slice(1)))) return;
      const list = ports.get(end.node) ?? [];
      end.port = `${end.node}:w${list.length}`;
      list.push({ id: end.port, side });
      ports.set(end.node, list);
    };
    for (const s of specs) {
      if (s.source.node.startsWith("b")) portOn(s.source, "SOUTH");
      if (s.target.node.startsWith("b")) portOn(s.target, "NORTH");
    }

    // Buses go first so ELK keeps them in case order where it can; folded
    // rows go in row order so each row keeps the order it was filled in.
    const ordered = piece.byHop
      ? [...buses].sort((a, b) => (rowOf.get(a.id) ?? 0) - (rowOf.get(b.id) ?? 0) || order(a.id) - order(b.id))
      : buses;
    const busNodes: ElkNode[] = ordered.map((b) => {
      if (b.star) return { id: `b${b.id}`, width: 20, height: BUS_BAR, layoutOptions: row(b.id) };
      const left = outside(busLabel(b), "H_LEFT", nameWidth(b));
      const inj = injections.get(b.id);
      if (inj) {
        const wires = ports.get(`b${b.id}`) ?? [];
        return {
          id: `b${b.id}`,
          width: barWidth(b),
          height: BUS_BAR + Math.max(...inj.map((i) => i.height)),
          layoutOptions: {
            ...row(b.id),
            "elk.portConstraints": "FIXED_SIDE",
            "elk.portAlignment.default": "END",
          },
          ports: wires.map((w) => ({ id: w.id, width: 0, height: 0, layoutOptions: { "elk.port.side": w.side } })),
          labels: [left, outside("pu", "H_RIGHT", 84 + BAR_END)],
        };
      }
      return {
        id: `b${b.id}`,
        width: barWidth(b),
        height: BUS_BAR,
        layoutOptions: row(b.id),
        labels: [left, outside("pu", "H_RIGHT", 84)],
      };
    });

    const graph: ElkNode = {
      id: "root",
      layoutOptions: {
        "elk.algorithm": "layered",
        "elk.direction": "DOWN",
        "elk.edgeRouting": "ORTHOGONAL",
        "elk.spacing.nodeNode": String(NODE_GAP),
        // Wires at least a grid step apart stay apart when snapped to the grid.
        "elk.spacing.edgeNode": String(GRID),
        "elk.spacing.edgeEdge": String(GRID),
        "elk.spacing.labelNode": "8",
        "elk.layered.spacing.nodeNodeBetweenLayers": "48",
        "elk.layered.spacing.edgeNodeBetweenLayers": String(GRID),
        "elk.layered.spacing.edgeEdgeBetweenLayers": String(GRID),
        // Network simplex gives the tidiest drawing but takes 13 s on case300;
        // Brandes-Koepf takes 1.4 s there, at the cost of a wider drawing.
        "elk.layered.nodePlacement.strategy": buses.length > READABLE_BUSES ? "BRANDES_KOEPF" : "NETWORK_SIMPLEX",
        ...(piece.byHop ? { "elk.partitioning.activate": "true" } : {}),
        "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
        "elk.separateConnectedComponents": "true",
        "elk.spacing.componentComponent": "80",
        "elk.nodeLabels.placement": "OUTSIDE V_CENTER H_RIGHT",
        "elk.edgeLabels.placement": "CENTER",
        "elk.spacing.edgeLabel": "4",
        "elk.spacing.portPort": String(PORT_GAP),
      },
      children: [...busNodes, ...nodes],
      edges: specs.map(
        (s): ElkExtendedEdge => ({
          id: s.id,
          sources: [s.source.port ?? s.source.node],
          targets: [s.target.port ?? s.target.node],
          // Reserve room for flow results on branches so they never overlap.
          labels: s.data.caseRef ? [{ text: "flow", width: FLOW_LABEL.w, height: FLOW_LABEL.h }] : undefined,
        })
      ),
    };
    return { graph, specs, rfNodes };
  };
  // Position of each bus in the fold, to keep rows in the order they were filled.
  const orderOf = new Map<number, number>();
  const order = (bus: number) => orderOf.get(bus) ?? 0;
  const setOrder = (rows: Map<number, number>) => {
    orderOf.clear();
    let i = 0;
    for (const [bus] of rows) orderOf.set(bus, i++);
  };
  setOrder(rowOf);

  let built = build(rowOf);
  let laid = await layoutEngine().layout(built.graph);
  if (fold && laid.width && laid.height) {
    // The estimate of each row's size is rough: measure the real drawing and
    // fold again if a different fold now fits the view better.
    const scale = laid.width / Math.max(fold.estimate.w, 1);
    const rowHeight = laid.height / Math.max(fold.rows, 1);
    const refit = bestFold(fold.groups, fold.width, aspect, rowHeight, scale);
    const next = rowsToMap(refit.rows);
    if ([...next].some(([bus, r]) => rowOf.get(bus) !== r)) {
      const before = Math.min(aspect / laid.width, 1 / laid.height);
      setOrder(next);
      const again = build(next);
      const relaid = await layoutEngine().layout(again.graph);
      if (relaid.width && relaid.height && Math.min(aspect / relaid.width, 1 / relaid.height) > before) {
        rowOf = next;
        built = again;
        laid = relaid;
      } else setOrder(rowOf);
    }
  }
  const { specs, rfNodes } = built;

  const at = new Map<string, ElkNode>();
  for (const n of laid.children ?? []) at.set(n.id, n);
  // The drawing lands on the editor's grid, so taps, terminals and the bends
  // of the laid-out wires line up with the knees a hand-dragged wire snaps to.
  const snap = (v: number) => Math.round(v / GRID) * GRID;
  const shift = (id: string) => {
    const n = at.get(id);
    return n ? { x: snap(n.x!) - n.x!, y: snap(n.y!) - n.y! } : { x: 0, y: 0 };
  };

  // Each wire that reaches a bus gets its own tap where the route lands.
  const taps = new Map<string, BusTap[]>();
  const tapFor = (busNode: string, p: ElkPoint): string => {
    const n = at.get(busNode)!;
    const x = snap(p.x) - snap(n.x!);
    const side: BusTap["side"] = p.y <= n.y! + BUS_BAR / 2 ? "top" : "bottom";
    const list = taps.get(busNode) ?? [];
    const id = `t${list.length}`;
    list.push({ id, x: Math.min(Math.max(x, 0), snap(n.x! + n.width!) - snap(n.x!)), side });
    taps.set(busNode, list);
    return id;
  };

  const focusOn = (ref: CaseRef | undefined) => (!!piece.focusElement && ref?.element === piece.focusElement) || undefined;
  const edges: SLDEdge[] = [];
  const routed = new Map((laid.edges ?? []).map((e) => [e.id, e as ElkExtendedEdge]));
  for (const s of specs) {
    const laidEdge = routed.get(s.id);
    const section = laidEdge?.sections?.[0];
    const label = laidEdge?.labels?.[0];
    if (!section) continue;
    const route = [section.startPoint, ...(section.bendPoints ?? []), section.endPoint].map((p) => ({
      x: p.x,
      y: p.y,
    }));
    // A bus carrying injections is deeper than its bar: wires leaving its
    // bottom start at the bar instead.
    const onBar = (node: string, p: ElkPoint) => {
      const n = at.get(node);
      if (node.startsWith("b") && n && p.y > n.y! + BUS_BAR) p.y = n.y! + BUS_BAR;
    };
    onBar(s.source.node, route[0]);
    onBar(s.target.node, route[route.length - 1]);
    const sourceHandle = s.source.handle ?? tapFor(s.source.node, route[0]);
    const targetHandle = s.target.handle ?? tapFor(s.target.node, route[route.length - 1]);
    // Ends move with what they're on (a tap to its grid point); bends go
    // to the nearest grid point. A route that doesn't survive is left for
    // the wire router.
    const end = (p: ElkPoint, node: string, onBus: boolean) => {
      const d = shift(node);
      return { x: onBus ? snap(p.x) : p.x + d.x, y: p.y + d.y };
    };
    const gridRoute = onGrid([
      end(route[0], s.source.node, !s.source.handle),
      ...route.slice(1, -1).map((p) => ({ x: snap(p.x), y: snap(p.y) })),
      end(route[route.length - 1], s.target.node, !s.target.handle),
    ]);
    edges.push({
      id: s.id,
      type: "conductor",
      source: s.source.node,
      target: s.target.node,
      sourceHandle,
      targetHandle,
      data: {
        ...s.data,
        route: gridRoute,
        labelAt: label ? { x: label.x!, y: label.y!, width: label.width!, height: label.height! } : undefined,
        focus: focusOn(s.data.caseRef),
      },
    } as SLDEdge);
  }

  const out: SLDNode[] = [];
  for (const [bus, list] of injections) {
    const n = at.get(`b${bus}`);
    // The anchor's symbols are drawn already; its stand-in only keeps their room.
    if (!n || bus === anchor) continue;
    list.forEach((inj, i) => {
      const x = 20 + i * SLOT;
      const tap = `i${i}`;
      const busTaps = taps.get(`b${bus}`) ?? [];
      busTaps.push({ id: tap, x, side: "bottom" });
      taps.set(`b${bus}`, busTaps);
      // The symbol's terminal touches the underside of the bar.
      const focus = inj.node.type === "generator" && focusOn((inj.node.data as GeneratorData).caseRef);
      out.push({
        ...inj.node,
        position: { x: snap(n.x!) + x - 20, y: snap(n.y!) + BUS_BAR },
        ...(focus ? { data: { ...inj.node.data, focus } } : {}),
      } as SLDNode);
      edges.push({
        id: `e-${inj.node.id}`,
        type: "conductor",
        source: `b${bus}`,
        sourceHandle: tap,
        target: inj.node.id,
        targetHandle: inj.handle,
        data: inj.data,
      } as SLDEdge);
    });
  }
  for (const b of buses) {
    const n = at.get(`b${b.id}`)!;
    const data: BusData = {
      label: b.star ? "" : busLabel(b),
      voltageKV: b.base_kv,
      orientation: "horizontal",
      length: Math.max(
        snap(n.x! + n.width! + (injections.has(b.id) ? BAR_END : 0)) - snap(n.x!),
        ...(taps.get(`b${b.id}`) ?? []).map((t) => t.x)
      ),
      taps: taps.get(`b${b.id}`) ?? [],
      star: b.star || undefined,
      focus: b.id === piece.focus || undefined,
      caseRef: { kind: "bus", id: b.id, element: busElement(b.id) },
    };
    out.push({ id: `b${b.id}`, type: "bus", position: { x: snap(n.x!), y: snap(n.y!) }, data } as SLDNode);
  }
  for (const [id, node] of rfNodes) {
    const n = at.get(id)!;
    const focus = focusOn((node.data as { caseRef?: CaseRef }).caseRef);
    out.push({ ...node, position: { x: snap(n.x!), y: snap(n.y!) }, ...(focus ? { data: { ...node.data, focus } } : {}) } as SLDNode);
  }

  return { nodes: out, edges, buses: buses.length, boundary: boundary.length, links };
}

/** Pitch of the editor's drawing grid. */
const GRID = 20;

/** A route with repeated and in-line points dropped, or undefined when it
 *  is no longer made of right angles. */
function onGrid(points: { x: number; y: number }[]): { x: number; y: number }[] | undefined {
  const out: { x: number; y: number }[] = [];
  for (const p of points) {
    const q = out[out.length - 1];
    if (q && Math.abs(q.x - p.x) < 0.01 && Math.abs(q.y - p.y) < 0.01) continue;
    const r = out[out.length - 2];
    if (q && r && ((Math.abs(r.x - q.x) < 0.01 && Math.abs(q.x - p.x) < 0.01) || (Math.abs(r.y - q.y) < 0.01 && Math.abs(q.y - p.y) < 0.01)))
      out.pop();
    out.push(p);
  }
  for (let i = 1; i < out.length; i++)
    if (Math.abs(out[i].x - out[i - 1].x) > 0.01 && Math.abs(out[i].y - out[i - 1].y) > 0.01) return undefined;
  return out.length >= 2 ? out : undefined;
}

/** Bus → row index, from rows listed top to bottom. */
function rowsToMap(rows: number[][]): Map<number, number> {
  const m = new Map<number, number>();
  rows.forEach((r, i) => r.forEach((b) => m.set(b, i)));
  return m;
}
