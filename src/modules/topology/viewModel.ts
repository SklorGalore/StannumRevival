// Graph scoping helpers ported from nodal's web/view-model.ts and topology-view.ts.
import type { GraphData, GraphNode, TopologySnapshot } from "../../lib/analysis/types";
import { elementBus, parseElement } from "../../lib/elements";

/** A bus or a piece of equipment picked in the graph. */
export interface GraphSelection {
  bus: number | null;
  equipment: string | null;
}

export const DISPLAY_LIMIT = 500;

export function focusedGraph(graph: GraphData, selected: Set<string>): GraphData {
  return {
    ...graph,
    nodes: graph.nodes.filter((n) => selected.has(n.id)),
    edges: graph.edges.filter((e) => selected.has(e.source) && selected.has(e.target)),
  };
}

export function nodeForBus(graph: GraphData, bus: number): GraphNode | undefined {
  return graph.nodes.find((n) => n.bus === bus || n.member_buses?.includes(bus));
}

/** Buses and junctions within `hops` equipment hops of a bus. */
export function neighborhood(graph: GraphData, bus: number, hops = 2): GraphData {
  const equipment = new Map<string, Set<string>>();
  const incident = new Map<string, Set<string>>();
  for (const edge of graph.edges) {
    const nodes = equipment.get(edge.equipment_id) ?? new Set<string>();
    nodes.add(edge.source);
    nodes.add(edge.target);
    equipment.set(edge.equipment_id, nodes);
    for (const node of [edge.source, edge.target]) {
      const ids = incident.get(node) ?? new Set<string>();
      ids.add(edge.equipment_id);
      incident.set(node, ids);
    }
  }
  const origin = nodeForBus(graph, bus);
  const selected = new Set(origin ? [origin.id] : [`bus:${bus}`]);
  let frontier = new Set(selected);
  for (let i = 0; i < hops && frontier.size; i++) {
    const next = new Set<string>();
    for (const node of frontier)
      for (const eq of incident.get(node) ?? [])
        for (const neighbor of equipment.get(eq) ?? [])
          if (!selected.has(neighbor)) {
            selected.add(neighbor);
            next.add(neighbor);
          }
    frontier = next;
  }
  return focusedGraph(graph, selected);
}

export function islandGraph(snapshot: TopologySnapshot, islandId: number): GraphData | null {
  const island = snapshot.analysis.islands.find((i) => i.id === islandId);
  if (!island) return null;
  const ids = new Set([
    ...island.buses.map((b) => nodeForBus(snapshot.graph, b)?.id ?? `bus:${b}`),
    ...island.synthetic_nodes,
  ]);
  return focusedGraph(snapshot.graph, ids);
}

export interface TopoBusRow {
  bus: number;
  name: string;
  members: number[];
  base_kv: number;
  degree: number | null;
  island: number | null;
  status: "Active" | "Excluded";
  articulation: boolean;
}

export function busRows(snapshot: TopologySnapshot): TopoBusRow[] {
  const { case: c, analysis: a, graph } = snapshot;
  const byNumber = new Map(c.buses.map((b) => [b.number, b]));
  const degrees = new Map(a.degrees.map((d) => [d.bus, d]));
  const cuts = new Set(a.articulation_buses);
  const active = new Set(
    graph.nodes.filter((n) => n.bus !== null).flatMap((n) => (n.member_buses?.length ? n.member_buses : [n.bus!]))
  );
  const rows: TopoBusRow[] = [];
  for (const node of graph.nodes) {
    if (node.bus === null) continue;
    const bus = byNumber.get(node.bus);
    if (!bus) continue;
    const d = degrees.get(node.bus);
    rows.push({
      bus: node.bus,
      name: bus.name.trim(),
      members: node.member_buses?.length ? node.member_buses : [node.bus],
      base_kv: bus.base_kv,
      degree: d?.degree ?? null,
      island: d?.island ?? null,
      status: "Active",
      articulation: cuts.has(node.bus),
    });
  }
  for (const bus of c.buses)
    if (!active.has(bus.number))
      rows.push({
        bus: bus.number,
        name: bus.name.trim(),
        members: [bus.number],
        base_kv: bus.base_kv,
        degree: null,
        island: null,
        status: "Excluded",
        articulation: false,
      });
  return rows;
}

/**
 * What a shared element id is in the topology graph, and the bus to centre a
 * neighbourhood on to show it. Generators and loads are found at their bus.
 */
export function locateElement(element: string, s: TopologySnapshot): { selection: GraphSelection; bus: number | null } | null {
  const parsed = parseElement(element);
  if (!parsed) return null;
  if (parsed.kind === "line" || parsed.kind === "transformer" || parsed.kind === "switch") {
    const e = s.case.equipment.find((x) => x.id === element);
    return e ? { selection: { bus: null, equipment: e.id }, bus: e.buses[0] ?? null } : null;
  }
  const bus = elementBus(element);
  return bus !== null && s.case.buses.some((b) => b.number === bus) ? { selection: { bus, equipment: null }, bus } : null;
}
