// Build a one-line diagram from the open case: pick which buses to draw, lay
// them out with ELK's layered algorithm (orthogonal wires, top to bottom),
// and turn the result into diagram nodes and edges linked to case records.
import ELK from "elkjs/lib/elk-api.js";
import type { ElkExtendedEdge, ElkNode, ElkPoint } from "elkjs/lib/elk-api";
import workerUrl from "elkjs/lib/elk-worker.min.js?url";
import type { CaseNetwork, NetworkBranch, NetworkBus } from "../analysis/types";
import type {
  BusData,
  BusTap,
  ConductorData,
  GeneratorData,
  LoadData,
  OffPageData,
  SLDEdge,
  SLDNode,
  TransformerData,
} from "../../store/types";
import { defaultGeneratorData, defaultLoadData, defaultTransformerData } from "../../store/defaults";

export type Scope =
  | { kind: "all" }
  | { kind: "area"; area: number }
  | { kind: "around"; bus: number; depth: number };

/** Above this many buses a whole-case drawing stops being readable. */
export const READABLE_BUSES = 150;

// Symbol sizes must match the node components.
export const BUS_BAR = 6;
const GEN = { w: 48, h: 80 };
const LOAD = { w: 40, h: 56 };
const XFMR = { w: 56, h: 104 };
const OFF_PAGE = { w: 20, h: 28 };
/** Space kept to the right of symbols for their name and results. */
const SIDE_LABEL = 96;
/** Room for a branch's flow results: three lines of small text. */
const FLOW_LABEL = { w: 64, h: 34 };

export const busLabel = (b: Pick<NetworkBus, "id" | "name">) => b.name.trim() || `Bus ${b.id}`;

interface Adjacent {
  branch: NetworkBranch;
  other: number;
}

function adjacency(net: CaseNetwork): Map<number, Adjacent[]> {
  const adj = new Map<number, Adjacent[]>();
  for (const b of net.buses) adj.set(b.id, []);
  for (const br of net.branches) {
    adj.get(br.from)?.push({ branch: br, other: br.to });
    adj.get(br.to)?.push({ branch: br, other: br.from });
  }
  return adj;
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
  const depth = new Map<number, number>([[scope.bus, 0]]);
  const queue = [scope.bus];
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

/** Level of each bus in a breadth-first walk from the reference (or highest
 *  voltage) bus of each connected part. Branches point down the levels. */
function levels(buses: NetworkBus[], branches: NetworkBranch[]): Map<number, number> {
  const adj = new Map<number, number[]>();
  for (const b of buses) adj.set(b.id, []);
  for (const br of branches) {
    adj.get(br.from)!.push(br.to);
    adj.get(br.to)!.push(br.from);
  }
  const order = [...buses].sort(
    (a, b) => Number(b.kind === "Ref") - Number(a.kind === "Ref") || b.base_kv - a.base_kv || a.id - b.id
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

export interface Generated {
  nodes: SLDNode[];
  edges: SLDEdge[];
  buses: number;
  /** Branches to buses outside the scope, drawn as off-page markers. */
  boundary: number;
}

type EdgeSpec = {
  id: string;
  source: { node: string; port?: string; handle?: string };
  target: { node: string; port?: string; handle?: string };
  data: ConductorData;
};

export async function generateDiagram(net: CaseNetwork, scope: Scope): Promise<Generated> {
  const chosen = selectBuses(net, scope);
  const buses = net.buses.filter((b) => chosen.has(b.id));
  if (!buses.length) throw new Error("No buses match that selection.");
  const busById = new Map(net.buses.map((b) => [b.id, b]));
  const inner = net.branches.filter((br) => chosen.has(br.from) && chosen.has(br.to));
  const boundary = net.branches.filter((br) => chosen.has(br.from) !== chosen.has(br.to));
  const gens = net.generators.filter((g) => chosen.has(g.bus));
  const level = levels(buses, inner);

  const nodes: ElkNode[] = [];
  const specs: EdgeSpec[] = [];
  const rfNodes = new Map<string, SLDNode>();
  const degree = new Map<number, number>();
  const bump = (id: number) => degree.set(id, (degree.get(id) ?? 0) + 1);
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

  // Injections hang below their bus.
  const genCount = new Map<number, number>();
  for (const g of gens) genCount.set(g.bus, (genCount.get(g.bus) ?? 0) + 1);
  const genSeen = new Map<number, number>();
  for (const g of gens) {
    const n = (genSeen.get(g.bus) ?? 0) + 1;
    genSeen.set(g.bus, n);
    bump(g.bus);
    const id = `g${g.index}`;
    nodes.push({
      id,
      width: GEN.w,
      height: GEN.h,
      ports: [port(`${id}:p`, GEN.w / 2, 0, "NORTH")],
      labels: [outside("G", "H_RIGHT", SIDE_LABEL)],
      layoutOptions: fixedPorts,
    });
    const data: GeneratorData = {
      ...defaultGeneratorData,
      // Named by bus, numbered when a bus has several machines.
      label: genCount.get(g.bus)! > 1 ? `G ${g.bus}.${n}` : `G ${g.bus}`,
      nominalMW: g.pmax,
      voltageKV: kv(g.bus),
      caseRef: { kind: "gen", id: g.index },
    };
    rfNodes.set(id, { id, type: "generator", position: { x: 0, y: 0 }, data } as SLDNode);
    specs.push({
      id: `e-${id}`,
      source: { node: `b${g.bus}` },
      target: { node: id, port: `${id}:p`, handle: "gen-top" },
      data: conductor(kv(g.bus), { outOfService: !g.online || undefined }),
    });
  }
  for (const b of buses) {
    if (b.star || (b.pd === 0 && b.qd === 0)) continue;
    bump(b.id);
    const id = `l${b.id}`;
    nodes.push({
      id,
      width: LOAD.w,
      height: LOAD.h,
      ports: [port(`${id}:p`, LOAD.w / 2, 0, "NORTH")],
      labels: [outside("Load", "H_RIGHT", SIDE_LABEL)],
      layoutOptions: fixedPorts,
    });
    const data: LoadData = {
      ...defaultLoadData,
      label: "Load",
      nominalMW: b.pd,
      nominalMVAR: b.qd,
      caseRef: { kind: "load", id: b.id },
    };
    rfNodes.set(id, { id, type: "load", position: { x: 0, y: 0 }, data } as SLDNode);
    specs.push({
      id: `e-${id}`,
      source: { node: `b${b.id}` },
      target: { node: id, port: `${id}:p`, handle: "load-top" },
      data: conductor(kv(b.id)),
    });
  }

  for (const br of inner) {
    // Draw each branch from the bus nearer the top of the drawing.
    const [la, lb] = [level.get(br.from) ?? 0, level.get(br.to) ?? 0];
    const reversed = lb < la || (lb === la && kv(br.to) > kv(br.from));
    const [upper, lower] = reversed ? [br.to, br.from] : [br.from, br.to];
    bump(upper);
    bump(lower);
    const ref = { kind: "branch" as const, id: br.index };
    const outOfService = !br.online || undefined;
    if (br.transformer) {
      const id = `t${br.index}`;
      nodes.push({
        id,
        width: XFMR.w,
        height: XFMR.h,
        ports: [port(`${id}:hv`, XFMR.w / 2, 0, "NORTH"), port(`${id}:lv`, XFMR.w / 2, XFMR.h, "SOUTH")],
        labels: [outside("T", "H_RIGHT", SIDE_LABEL)],
        layoutOptions: fixedPorts,
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

  for (const br of boundary) {
    const inside = chosen.has(br.from) ? br.from : br.to;
    const far = inside === br.from ? br.to : br.from;
    const farBus = busById.get(far);
    const farName = farBus ? busLabel(farBus) : `Bus ${far}`;
    const text = farName === `Bus ${far}` ? `To ${farName}` : `To ${farName} (${far})`;
    bump(inside);
    const id = `o${br.index}`;
    nodes.push({
      id,
      width: OFF_PAGE.w,
      height: OFF_PAGE.h,
      ports: [port(`${id}:p`, OFF_PAGE.w / 2, 0, "NORTH")],
      labels: [outside(text, "H_RIGHT", Math.max(textWidth(text), SIDE_LABEL))],
      layoutOptions: fixedPorts,
    });
    const data: OffPageData = {
      label: text,
      bus: far,
      busName: farBus ? busLabel(farBus) : "",
      voltageKV: kv(inside),
      caseRef: { kind: "branch", id: br.index },
      // Flow is read at the inside end, so "reversed" means inside is "to".
      reversed: inside === br.to || undefined,
      outOfService: !br.online || undefined,
    };
    rfNodes.set(id, { id, type: "offPage", position: { x: 0, y: 0 }, data } as SLDNode);
    specs.push({
      id: `e-${id}`,
      source: { node: `b${inside}` },
      target: { node: id, port: `${id}:p`, handle: "op-top" },
      data: conductor(kv(inside), {
        caseRef: { kind: "branch", id: br.index },
        reversed: inside === br.to || undefined,
        outOfService: !br.online || undefined,
      }),
    });
  }

  // Buses go first so ELK keeps them in case order where it can.
  const busNodes: ElkNode[] = buses.map((b) => {
    if (b.star) return { id: `b${b.id}`, width: 20, height: BUS_BAR };
    const name = busLabel(b);
    const sub = `${b.id} · ${b.base_kv} kV`;
    const width = Math.max(100, roundUp(((degree.get(b.id) ?? 0) + 1) * 44, 20));
    return {
      id: `b${b.id}`,
      width,
      height: BUS_BAR,
      labels: [
        outside(name, "H_LEFT", Math.max(textWidth(name, 7), textWidth(sub, 6))),
        outside("pu", "H_RIGHT", 84),
      ],
    };
  });

  const graph: ElkNode = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "DOWN",
      "elk.edgeRouting": "ORTHOGONAL",
      "elk.spacing.nodeNode": "36",
      "elk.spacing.edgeNode": "18",
      "elk.spacing.edgeEdge": "12",
      "elk.spacing.labelNode": "8",
      "elk.layered.spacing.nodeNodeBetweenLayers": "48",
      "elk.layered.spacing.edgeNodeBetweenLayers": "18",
      "elk.layered.spacing.edgeEdgeBetweenLayers": "12",
      "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX",
      "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
      "elk.separateConnectedComponents": "true",
      "elk.spacing.componentComponent": "80",
      "elk.nodeLabels.placement": "OUTSIDE V_CENTER H_RIGHT",
      "elk.edgeLabels.placement": "CENTER",
      "elk.spacing.edgeLabel": "4",
    },
    children: [...busNodes, ...nodes],
    edges: specs.map(
      (s): ElkExtendedEdge => ({
        id: s.id,
        sources: [s.source.port ?? s.source.node],
        targets: [s.target.port ?? s.target.node],
        // Reserve room for flow results on branches so they never overlap.
        labels: s.data.caseRef
          ? [{ text: "flow", width: FLOW_LABEL.w, height: FLOW_LABEL.h }]
          : undefined,
      })
    ),
  };

  const laid = await layoutEngine().layout(graph);

  const at = new Map<string, ElkNode>();
  for (const n of laid.children ?? []) at.set(n.id, n);

  // Each wire that reaches a bus gets its own tap where the route lands.
  const taps = new Map<string, BusTap[]>();
  const tapFor = (busNode: string, p: ElkPoint): string => {
    const n = at.get(busNode)!;
    const x = p.x - n.x!;
    const side: BusTap["side"] = p.y <= n.y! + BUS_BAR / 2 ? "top" : "bottom";
    const list = taps.get(busNode) ?? [];
    const id = `t${list.length}`;
    list.push({ id, x: Math.min(Math.max(x, 0), n.width!), side });
    taps.set(busNode, list);
    return id;
  };

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
    const sourceHandle = s.source.handle ?? tapFor(s.source.node, section.startPoint);
    const targetHandle = s.target.handle ?? tapFor(s.target.node, section.endPoint);
    edges.push({
      id: s.id,
      type: "conductor",
      source: s.source.node,
      target: s.target.node,
      sourceHandle,
      targetHandle,
      data: {
        ...s.data,
        route,
        labelAt: label ? { x: label.x!, y: label.y!, width: label.width!, height: label.height! } : undefined,
      },
    } as SLDEdge);
  }

  const out: SLDNode[] = [];
  for (const b of buses) {
    const n = at.get(`b${b.id}`)!;
    const data: BusData = {
      label: b.star ? "" : busLabel(b),
      voltageKV: b.base_kv,
      orientation: "horizontal",
      length: n.width!,
      taps: taps.get(`b${b.id}`) ?? [],
      star: b.star || undefined,
      caseRef: { kind: "bus", id: b.id },
    };
    out.push({ id: `b${b.id}`, type: "bus", position: { x: n.x!, y: n.y! }, data } as SLDNode);
  }
  for (const [id, node] of rfNodes) {
    const n = at.get(id)!;
    out.push({ ...node, position: { x: n.x!, y: n.y! } } as SLDNode);
  }

  return { nodes: out, edges, buses: buses.length, boundary: boundary.length };
}
