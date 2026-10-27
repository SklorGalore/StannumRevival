// The topology snapshot arrives as columns, with buses, equipment and graph
// nodes referred to by position and without each record's source fields (see
// `topology_wire` in stannum-core). That keeps a large case's snapshot to
// about a tenth of its full size. The rest of the app works with the full
// snapshot, so rebuild it here. `topology_source` sends one element's fields.
import type {
  Analysis,
  Connection,
  Equipment,
  GraphData,
  GraphNode,
  Injection,
  TopoBus,
  TopoCase,
  TopoDiagnostic,
  TopologySnapshot,
} from "./types";

interface InjectionColumns {
  id: string[];
  bus: number[];
  status: number[];
  p_mw: number[];
  q_mvar: number[];
  line: number[];
  section: number[];
}

export interface WireCase {
  revision: number;
  base_mva: number;
  frequency_hz: number;
  zero_impedance_threshold: number;
  titles: string[];
  encoding: string;
  diagnostics: TopoDiagnostic[];
  skipped_sections: Record<string, number>;
  sections: string[];
  buses: {
    number: number[];
    name: string[];
    base_kv: number[];
    bus_type: number[];
    area: (number | null)[];
    zone: (number | null)[];
    voltage_pu: (number | null)[];
    line: number[];
    section: number[];
  };
  equipment: {
    id: string[];
    kind: number[];
    name: string[];
    circuit: string[];
    buses: number[][];
    status: number[];
    open_terminals: number[];
    line: number[];
    section: number[];
  };
  loads: InjectionColumns;
  generators: InjectionColumns;
}

export interface WireGraph {
  nodes: { bus: (number | null)[]; equipment: (number | null)[]; members: [number, number[]][] };
  edges: { equipment: number[]; source: number[]; target: number[]; winding: (number | null)[] };
  zero_impedance_groups: number[][];
  merge_zero_impedance: boolean;
  absorbed_equipment_ids: string[];
}

export interface WireAnalysis
  extends Omit<Analysis, "islands" | "degrees" | "bridges"> {
  islands: { id: number; synthetic_nodes: string[] }[];
  degrees: { bus: number[]; degree: number[]; island: number[] };
  bridges: number[];
}

export interface WireTopologyView {
  analysis: WireAnalysis;
  graph: WireGraph;
}

export interface WireTopology extends WireTopologyView {
  case: WireCase;
}

const KINDS: Equipment["kind"][] = ["line", "transformer", "switch"];

function injections(c: InjectionColumns, sections: string[]): Injection[] {
  return c.id.map((id, i) => ({
    id,
    bus: c.bus[i],
    status: c.status[i],
    p_mw: c.p_mw[i],
    q_mvar: c.q_mvar[i],
    source: { line: c.line[i], section: sections[c.section[i]] },
  }));
}

export function decodeCase(w: WireCase): TopoCase {
  const { sections } = w;
  const b = w.buses;
  const buses: TopoBus[] = b.number.map((number, i) => ({
    number,
    name: b.name[i],
    base_kv: b.base_kv[i],
    bus_type: b.bus_type[i],
    area: b.area[i],
    zone: b.zone[i],
    voltage_pu: b.voltage_pu[i],
    source: { line: b.line[i], section: sections[b.section[i]] },
  }));
  const e = w.equipment;
  const equipment: Equipment[] = e.id.map((id, i) => ({
    id,
    kind: KINDS[e.kind[i]],
    name: e.name[i],
    circuit: e.circuit[i],
    buses: e.buses[i],
    status: e.status[i],
    active_terminals: e.buses[i].map((_, t) => (e.open_terminals[i] & (1 << t)) === 0),
    source: { line: e.line[i], section: sections[e.section[i]] },
  }));
  return {
    revision: w.revision,
    base_mva: w.base_mva,
    frequency_hz: w.frequency_hz,
    zero_impedance_threshold: w.zero_impedance_threshold,
    titles: w.titles,
    encoding: w.encoding,
    buses,
    equipment,
    loads: injections(w.loads, sections),
    generators: injections(w.generators, sections),
    diagnostics: w.diagnostics,
    skipped_sections: w.skipped_sections,
  };
}

/** The analysis and graph for `c`, the case the graph was built from. */
export function decodeView(w: WireTopologyView, c: TopoCase): { analysis: Analysis; graph: GraphData } {
  const busByNumber = new Map(c.buses.map((b) => [b.number, b]));
  const members = new Map(w.graph.nodes.members);
  const n = w.graph.nodes;
  const nodes: GraphNode[] = n.bus.map((bus, i) => {
    if (bus !== null) {
      const record = busByNumber.get(bus);
      return {
        id: `bus:${bus}`,
        label: `${bus} · ${record?.name.trim() ?? ""}`,
        bus,
        equipment_id: null,
        base_kv: record?.base_kv ?? null,
        member_buses: members.get(i) ?? [bus],
      };
    }
    const eq = c.equipment[n.equipment[i]!];
    return {
      id: `junction:${eq.id}`,
      label: `Transformer ${eq.circuit}`,
      bus: null,
      equipment_id: eq.id,
      base_kv: null,
      member_buses: members.get(i) ?? [],
    };
  });
  const g = w.graph.edges;
  const edges: Connection[] = g.equipment.map((e, i) => {
    const equipment_id = c.equipment[e].id;
    const winding = g.winding[i];
    return {
      id: winding === null ? `connection:${equipment_id}` : `connection:${equipment_id}:w${winding}`,
      equipment_id,
      source: nodes[g.source[i]].id,
      target: nodes[g.target[i]].id,
      winding,
    };
  });
  const { islands, bridges, degrees: d, ...counts } = w.analysis;
  const degrees = d.bus.map((bus, i) => ({ bus, degree: d.degree[i], island: d.island[i] }));
  const islandBuses = new Map<number, number[]>();
  for (const x of degrees) {
    const list = islandBuses.get(x.island);
    if (list) list.push(x.bus);
    else islandBuses.set(x.island, [x.bus]);
  }
  return {
    analysis: {
      ...counts,
      islands: islands.map((i) => ({
        id: i.id,
        buses: (islandBuses.get(i.id) ?? []).sort((a, b) => a - b),
        synthetic_nodes: i.synthetic_nodes,
      })),
      degrees,
      bridges: bridges.map((i) => edges[i]),
    },
    graph: {
      nodes,
      edges,
      zero_impedance_groups: w.graph.zero_impedance_groups,
      merge_zero_impedance: w.graph.merge_zero_impedance,
      absorbed_equipment_ids: w.graph.absorbed_equipment_ids,
    },
  };
}

export function decodeTopology(w: WireTopology): TopologySnapshot {
  const c = decodeCase(w.case);
  return { case: c, ...decodeView(w, c) };
}
