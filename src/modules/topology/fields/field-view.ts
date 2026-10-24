// Ported from nodal's web/field-view.ts.
import type { Bus, Equipment, GraphData, GraphNode, Snapshot } from './types';

export type FieldTarget = 'bus' | 'equipment';
export type FieldValue = string | number | null;
export interface FieldDefinition { key: string; label: string; type: 'number' | 'text' | 'category' }
export type FilterOperator = 'eq' | 'neq' | 'contains' | 'gt' | 'gte' | 'lt' | 'lte' | 'between' | 'missing' | 'present';
export interface FilterRule { id: string; target: FieldTarget; field: string; operator: FilterOperator; value: string; upper?: string }

export const BUS_FIELDS: FieldDefinition[] = [
  { key: 'number', label: 'Bus number', type: 'number' },
  { key: 'name', label: 'Name', type: 'text' },
  { key: 'base_kv', label: 'Base kV', type: 'number' },
  { key: 'area', label: 'Area', type: 'category' },
  { key: 'zone', label: 'Zone', type: 'category' },
  { key: 'bus_type', label: 'Bus type', type: 'category' },
  { key: 'voltage_pu', label: 'Voltage (p.u.)', type: 'number' },
  { key: 'island', label: 'Island', type: 'category' },
  { key: 'degree', label: 'Degree', type: 'number' },
  { key: 'articulation', label: 'Articulation', type: 'category' },
  { key: 'status', label: 'Status', type: 'category' },
];

export const EQUIPMENT_FIELDS: FieldDefinition[] = [
  { key: 'kind', label: 'Equipment type', type: 'category' },
  { key: 'name', label: 'Name', type: 'text' },
  { key: 'circuit', label: 'Circuit', type: 'text' },
  { key: 'status', label: 'RAW status', type: 'category' },
  { key: 'topology', label: 'Topology status', type: 'category' },
];

interface Lookup {
  buses: Map<number, Bus>;
  equipment: Map<string, Equipment>;
  nodesByBus: Map<number, GraphNode>;
  degrees: Map<number, { degree: number; island: number }>;
  articulations: Set<number>;
  activeEquipment: Set<string>;
  absorbedEquipment: Set<string>;
}

const lookupCache = new WeakMap<Snapshot, Lookup>();

function lookup(snapshot: Snapshot): Lookup {
  const cached = lookupCache.get(snapshot);
  if (cached) return cached;
  const nodesByBus = new Map<number, GraphNode>();
  for (const node of snapshot.graph.nodes) {
    if (node.bus !== null) nodesByBus.set(node.bus, node);
    for (const member of node.member_buses ?? []) nodesByBus.set(member, node);
  }
  const state: Lookup = {
    buses: new Map(snapshot.case.buses.map(bus => [bus.number, bus])),
    equipment: new Map(snapshot.case.equipment.map(item => [item.id, item])),
    nodesByBus,
    degrees: new Map(snapshot.analysis.degrees.map(item => [item.bus, { degree: item.degree, island: item.island }])),
    articulations: new Set(snapshot.analysis.articulation_buses),
    activeEquipment: new Set(snapshot.graph.edges.map(edge => edge.equipment_id)),
    absorbedEquipment: new Set(snapshot.graph.absorbed_equipment_ids),
  };
  lookupCache.set(snapshot, state);
  return state;
}

function busId(id: number | string): number | null {
  if (typeof id === 'number') return Number.isFinite(id) ? id : null;
  const match = /^bus:(-?\d+)$/.exec(id);
  if (match) return Number(match[1]);
  if (/^-?\d+$/.test(id)) return Number(id);
  return null;
}

function unique(values: FieldValue[]): FieldValue[] {
  return [...new Set(values)];
}

function busSourceValue(bus: Bus, field: string): FieldValue {
  switch (field) {
    case 'number': return bus.number;
    case 'name': return bus.name.trim();
    case 'base_kv': return bus.base_kv;
    case 'area': return bus.area;
    case 'zone': return bus.zone;
    case 'bus_type': return ({ 1: 'PQ', 2: 'PV', 3: 'Swing', 4: 'Disconnected' } as Record<number, string>)[bus.bus_type] ?? String(bus.bus_type);
    case 'voltage_pu': return bus.voltage_pu;
    default: return null;
  }
}

function valuesFor(state: Lookup, target: FieldTarget, id: number | string, field: string): FieldValue[] {
  if (target === 'equipment') {
    const equipment = state.equipment.get(String(id));
    if (!equipment) return [];
    switch (field) {
      case 'kind': return [equipment.kind];
      case 'name': return [equipment.name.trim()];
      case 'circuit': return [equipment.circuit];
      case 'status': return [equipment.status];
      case 'topology': return [state.activeEquipment.has(equipment.id) ? 'Active' : state.absorbedEquipment.has(equipment.id) ? 'Absorbed' : 'Excluded'];
      default: return [];
    }
  }

  const number = busId(id);
  if (number === null) return [];
  const bus = state.buses.get(number);
  if (!bus) return [];
  if (field === 'status') return [state.nodesByBus.has(number) ? 'Active' : 'Excluded'];
  const node = state.nodesByBus.get(number);
  if (field === 'degree' || field === 'island' || field === 'articulation') {
    if (!node) return [field === 'articulation' ? 'No' : null];
    const representative = node.bus ?? number;
    if (field === 'articulation') return [state.articulations.has(representative) ? 'Yes' : 'No'];
    const degree = state.degrees.get(representative);
    return [degree ? degree[field] : null];
  }
  return [busSourceValue(bus, field)];
}

export function fieldValues(snapshot: Snapshot, target: FieldTarget, id: number | string, field: string): FieldValue[] {
  const state = lookup(snapshot);
  if (target !== 'bus') return valuesFor(state, target, id, field);
  const number = busId(id);
  if (number === null) return [];
  const node = state.nodesByBus.get(number);
  const derived = field === 'status' || field === 'degree' || field === 'island' || field === 'articulation';
  if (derived || !node) return valuesFor(state, target, number, field);
  const members = node.member_buses?.length ? node.member_buses : [node.bus ?? number];
  return unique(members.map(member => {
    const bus = state.buses.get(member);
    return bus ? busSourceValue(bus, field) : null;
  }));
}

function numeric(value: string): number | null {
  if (!value.trim()) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function matchesValue(actual: FieldValue, rule: FilterRule): boolean {
  if (rule.operator === 'missing') return actual === null;
  if (rule.operator === 'present') return actual !== null;
  if (actual === null) return false;
  if (typeof actual === 'number') {
    const value = numeric(rule.value);
    if (value === null) return false;
    switch (rule.operator) {
      case 'eq': return actual === value;
      case 'neq': return actual !== value;
      case 'gt': return actual > value;
      case 'gte': return actual >= value;
      case 'lt': return actual < value;
      case 'lte': return actual <= value;
      case 'between': {
        const upper = rule.upper === undefined ? null : numeric(rule.upper);
        return upper !== null && upper >= value && actual >= value && actual <= upper;
      }
      default: return false;
    }
  }
  const text = String(actual);
  switch (rule.operator) {
    case 'eq': return text === rule.value;
    case 'neq': return text !== rule.value;
    case 'contains': return text.toLocaleLowerCase().includes(rule.value.toLocaleLowerCase());
    default: return false;
  }
}

const BUS_DERIVED_FIELDS = new Set(['degree', 'island', 'articulation', 'status']);

function matchesBusWithLookup(state: Lookup, bus: number, rules: FilterRule[]): boolean {
  const applicable = rules.filter(rule => rule.target === 'bus');
  if (!applicable.length) return true;
  const node = state.nodesByBus.get(bus);
  const sourceRules = applicable.filter(rule => !BUS_DERIVED_FIELDS.has(rule.field));
  const derivedRules = applicable.filter(rule => BUS_DERIVED_FIELDS.has(rule.field));
  if (derivedRules.some(rule => !valuesFor(state, 'bus', bus, rule.field).some(value => matchesValue(value, rule)))) return false;
  if (!sourceRules.length) return true;
  const members = node?.member_buses?.length ? node.member_buses : [node?.bus ?? bus];
  return members.some(member => {
    const values = sourceRules.map(rule => valuesFor(state, 'bus', member, rule.field)[0]);
    return sourceRules.every((rule, index) => matchesValue(values[index] ?? null, rule));
  });
}

function matchesEquipmentWithLookup(state: Lookup, id: string, rules: FilterRule[]): boolean {
  const applicable = rules.filter(rule => rule.target === 'equipment');
  return applicable.every(rule => valuesFor(state, 'equipment', id, rule.field).some(value => matchesValue(value, rule)));
}

export function matchesBus(snapshot: Snapshot, bus: number, rules: FilterRule[]): boolean {
  return matchesBusWithLookup(lookup(snapshot), bus, rules);
}

export function matchesEquipment(snapshot: Snapshot, id: string, rules: FilterRule[]): boolean {
  return matchesEquipmentWithLookup(lookup(snapshot), id, rules);
}

export function fieldOptions(snapshot: Snapshot, target: FieldTarget, field: string): FieldValue[] {
  const state = lookup(snapshot);
  const values: FieldValue[] = [];
  if (target === 'bus') {
    for (const bus of snapshot.case.buses) values.push(...valuesFor(state, target, bus.number, field));
  } else {
    for (const item of snapshot.case.equipment) values.push(...valuesFor(state, target, item.id, field));
  }
  const distinct = unique(values);
  return distinct.sort((a, b) => {
    if (a === null) return b === null ? 0 : 1;
    if (b === null) return -1;
    if (typeof a === 'number' && typeof b === 'number') return a - b;
    return String(a).localeCompare(String(b), undefined, { sensitivity: 'variant', numeric: true });
  });
}

export function filterGraph(snapshot: Snapshot, graph: GraphData, rules: FilterRule[]): GraphData {
  if (!rules.length) return graph;
  const state = lookup(snapshot);
  const selectedNodes = graph.nodes.filter(node => {
    if (node.bus === null) return true;
    return matchesBusWithLookup(state, node.bus, rules);
  });
  const selectedNodeIds = new Set(selectedNodes.map(node => node.id));
  const edges = graph.edges.filter(edge => selectedNodeIds.has(edge.source)
    && selectedNodeIds.has(edge.target)
    && matchesEquipmentWithLookup(state, edge.equipment_id, rules));

  const junctionTerminalCounts = new Map<string, number>();
  const nodesById = new Map(graph.nodes.map(node => [node.id, node]));
  for (const edge of edges) {
    for (const id of [edge.source, edge.target]) {
      const node = nodesById.get(id);
      if (node?.bus === null) junctionTerminalCounts.set(id, (junctionTerminalCounts.get(id) ?? 0) + 1);
    }
  }
  const keptNodes = selectedNodes.filter(node => node.bus !== null || (junctionTerminalCounts.get(node.id) ?? 0) >= 2);
  const keptIds = new Set(keptNodes.map(node => node.id));
  return {
    ...graph,
    nodes: keptNodes,
    edges: edges.filter(edge => keptIds.has(edge.source) && keptIds.has(edge.target)),
  };
}
