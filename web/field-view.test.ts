import { describe, expect, it } from 'vitest';
import { fieldOptions, fieldValues, filterGraph, matchesBus, matchesEquipment } from './field-view';
import type { Snapshot } from './types';

const source = (line: number) => ({ line, section: 'BUS', fields: {}, raw_lines: [] });

function snapshot(): Snapshot {
  const buses = [
    { number: 1, name: 'Alpha', base_kv: 138, bus_type: 1, area: 1, zone: 10, voltage_pu: 1.01, source: source(1) },
    { number: 2, name: 'Beta', base_kv: 138, bus_type: 2, area: 2, zone: 10, voltage_pu: null, source: source(2) },
    { number: 3, name: 'Gamma', base_kv: 69, bus_type: 4, area: null, zone: 20, voltage_pu: 0, source: source(3) },
    { number: 4, name: 'Delta', base_kv: 69, bus_type: 3, area: 1, zone: 20, voltage_pu: 1, source: source(4) },
  ];
  const equipment = [
    { id: 'line:1:2:1', kind: 'line' as const, name: 'Alpha-Beta', circuit: '1', buses: [1, 2], status: 1, active_terminals: [true, true], source: source(5) },
    { id: 'transformer:1:3:1', kind: 'transformer' as const, name: 'Three winding', circuit: '1', buses: [1, 3, 4], status: 0, active_terminals: [true, true, true], source: source(6) },
    { id: 'switch:2:4:1', kind: 'switch' as const, name: 'Tie', circuit: '1', buses: [2, 4], status: 0, active_terminals: [false, false], source: source(7) },
    { id: 'line:1:2:2', kind: 'line' as const, name: 'Parallel', circuit: '2', buses: [1, 2], status: 1, active_terminals: [true, true], source: source(8) },
  ];
  const graph = {
    nodes: [
      { id: 'bus:1', label: '1 · 2', bus: 1, equipment_id: null, base_kv: 138, member_buses: [1, 2] },
      { id: 'bus:3', label: '3', bus: 3, equipment_id: null, base_kv: 69, member_buses: [3] },
      { id: 'bus:4', label: '4', bus: 4, equipment_id: null, base_kv: 69, member_buses: [4] },
      { id: 'junction:transformer:1:3:1', label: 'T', bus: null, equipment_id: 'transformer:1:3:1', base_kv: null, member_buses: [] },
    ],
    edges: [
      { id: 'a', equipment_id: 'line:1:2:1', source: 'bus:1', target: 'bus:1', winding: null },
      { id: 'b', equipment_id: 'transformer:1:3:1', source: 'bus:1', target: 'junction:transformer:1:3:1', winding: 1 },
      { id: 'c', equipment_id: 'transformer:1:3:1', source: 'bus:3', target: 'junction:transformer:1:3:1', winding: 2 },
      { id: 'd', equipment_id: 'transformer:1:3:1', source: 'bus:4', target: 'junction:transformer:1:3:1', winding: 3 },
      { id: 'f', equipment_id: 'line:1:2:2', source: 'bus:1', target: 'bus:1', winding: null },
    ],
    zero_impedance_groups: [[1, 2]], merge_zero_impedance: true, absorbed_equipment_ids: ['switch:2:4:1'],
  };
  return {
    case: { revision: 35, base_mva: 100, frequency_hz: 60, zero_impedance_threshold: 0, titles: [], encoding: 'UTF-8', buses, equipment, loads: [], generators: [], diagnostics: [], skipped_sections: {} },
    graph,
    analysis: { active_buses: 3, original_active_buses: 4, absorbed_equipment: 1, excluded_buses: 0, active_equipment: 3, excluded_equipment: 0, synthetic_junctions: 1, islands: [{ id: 1, buses: [1, 3, 4], synthetic_nodes: ['junction:transformer:1:3:1'] }], degrees: [{ bus: 1, degree: 2, island: 1 }, { bus: 3, degree: 1, island: 1 }, { bus: 4, degree: 2, island: 1 }], bridges: [], articulation_buses: [1], synthetic_articulations: [] },
  };
}

describe('field values and filtering', () => {
  it('returns merged member source values and representative derived values', () => {
    const data = snapshot();
    expect(fieldValues(data, 'bus', 1, 'area')).toEqual([1, 2]);
    expect(fieldValues(data, 'bus', 'bus:1', 'degree')).toEqual([2]);
    expect(fieldValues(data, 'bus', 1, 'articulation')).toEqual(['Yes']);
    expect(fieldValues(data, 'equipment', 'transformer:1:3:1', 'topology')).toEqual(['Active']);
    expect(fieldValues(data, 'bus', 3, 'voltage_pu')).toEqual([0]);
  });

  it('matches numeric, inclusive range, text, missing and AND rules without coercing blank to zero', () => {
    const data = snapshot();
    expect(matchesBus(data, 1, [
      { id: 'a', target: 'bus', field: 'base_kv', operator: 'between', value: '100', upper: '140' },
      { id: 'b', target: 'bus', field: 'name', operator: 'contains', value: 'lph' },
    ])).toBe(true);
    expect(matchesBus(data, 2, [{ id: 'a', target: 'bus', field: 'voltage_pu', operator: 'missing', value: '' }])).toBe(true);
    expect(matchesBus(data, 2, [{ id: 'a', target: 'bus', field: 'voltage_pu', operator: 'eq', value: '0' }])).toBe(false);
    expect(matchesBus(data, 3, [{ id: 'a', target: 'bus', field: 'voltage_pu', operator: 'eq', value: '0' }])).toBe(true);
    expect(matchesBus(data, 1, [{ id: 'a', target: 'bus', field: 'base_kv', operator: 'gt', value: 'bad' }])).toBe(false);
    expect(matchesBus(data, 1, [{ id: 'a', target: 'bus', field: 'base_kv', operator: 'between', value: '140', upper: '100' }])).toBe(false);
  });

  it('builds field lookups for each snapshot identity so refreshed topology is not stale', () => {
    const initial = snapshot();
    const refreshed: Snapshot = {
      ...initial,
      graph: { ...initial.graph, nodes: initial.graph.nodes.filter(node => node.id !== 'bus:3'), edges: initial.graph.edges.filter(edge => edge.source !== 'bus:3' && edge.target !== 'bus:3') },
      analysis: { ...initial.analysis, degrees: initial.analysis.degrees.filter(item => item.bus !== 3) },
    };
    expect(fieldValues(initial, 'bus', 3, 'status')).toEqual(['Active']);
    expect(fieldValues(refreshed, 'bus', 3, 'status')).toEqual(['Excluded']);
  });

  it('requires merged source rules to match one original member and uses the full option domain', () => {
    const data = snapshot();
    expect(matchesBus(data, 1, [
      { id: 'a', target: 'bus', field: 'area', operator: 'eq', value: '1' },
      { id: 'b', target: 'bus', field: 'name', operator: 'eq', value: 'Beta' },
    ])).toBe(false);
    expect(matchesBus(data, 1, [
      { id: 'a', target: 'bus', field: 'area', operator: 'eq', value: '2' },
      { id: 'b', target: 'bus', field: 'name', operator: 'eq', value: 'Beta' },
    ])).toBe(true);
    expect(fieldOptions(data, 'bus', 'area')).toEqual([1, 2, null]);
  });

  it('matches equipment RAW status and topology as separate categories', () => {
    const data = snapshot();
    expect(matchesEquipment(data, 'switch:2:4:1', [{ id: 'a', target: 'equipment', field: 'status', operator: 'eq', value: '0' }])).toBe(true);
    expect(fieldValues(data, 'equipment', 'switch:2:4:1', 'topology')).toEqual(['Absorbed']);
    expect(matchesEquipment(data, 'switch:2:4:1', [{ id: 'a', target: 'equipment', field: 'topology', operator: 'eq', value: 'Absorbed' }])).toBe(true);
    expect(matchesEquipment(data, 'switch:2:4:1', [{ id: 'a', target: 'equipment', field: 'topology', operator: 'eq', value: 'Active' }])).toBe(false);
  });

  it('filters buses and equipment while preserving surviving parallel edges and graph metadata', () => {
    const data = snapshot();
    const parallel = filterGraph(data, data.graph, [{ id: 'a', target: 'equipment', field: 'kind', operator: 'eq', value: 'line' }]);
    expect(parallel.edges.map(edge => [edge.id, edge.equipment_id])).toEqual([['a', 'line:1:2:1'], ['f', 'line:1:2:2']]);
    expect(parallel.absorbed_equipment_ids).toEqual(data.graph.absorbed_equipment_ids);
    const graph = filterGraph(data, data.graph, [{ id: 'a', target: 'equipment', field: 'name', operator: 'eq', value: 'Parallel' }]);
    expect(graph.edges.map(edge => edge.id)).toEqual(['f']);
    expect(graph.nodes.map(node => node.id)).toEqual(['bus:1', 'bus:3', 'bus:4']);
    expect(graph.merge_zero_impedance).toBe(true);
    expect(graph.zero_impedance_groups).toEqual([[1, 2]]);
    const busFiltered = filterGraph(data, data.graph, [{ id: 'b', target: 'bus', field: 'number', operator: 'eq', value: '3' }]);
    expect(busFiltered.nodes.map(node => node.id)).toEqual(['bus:3']);
    expect(busFiltered.edges).toEqual([]);
  });

  it('drops a three winding junction below two surviving terminals and preserves the unfiltered graph exactly', () => {
    const data = snapshot();
    expect(filterGraph(data, data.graph, [])).toBe(data.graph);
    const graph = filterGraph(data, data.graph, [{ id: 'a', target: 'bus', field: 'number', operator: 'eq', value: '3' }]);
    expect(graph.nodes.map(node => node.id)).toEqual(['bus:3']);
    expect(graph.edges).toEqual([]);
  });
});
