import { describe, expect, it } from "vitest";
import type { ContingencyDetail, TopologySnapshot } from "../analysis/types";
import { defaultColorSettings } from "../../modules/topology/fields/field-colors";
import { LAYER_COLORS, resolveLayers, type LayerInput } from "./layers";

const source = (line: number) => ({ line, section: 'BUS', fields: {}, raw_lines: [] });

function snapshot(): TopologySnapshot {
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
  return ({
    case: { revision: 35, base_mva: 100, frequency_hz: 60, zero_impedance_threshold: 0, titles: [], encoding: 'UTF-8', buses, equipment, loads: [], generators: [], diagnostics: [], skipped_sections: {} },
    graph,
    analysis: { active_buses: 3, original_active_buses: 4, absorbed_equipment: 1, excluded_buses: 0, active_equipment: 3, excluded_equipment: 0, synthetic_junctions: 1, islands: [{ id: 1, buses: [1, 3, 4], synthetic_nodes: ['junction:transformer:1:3:1'] }], degrees: [{ bus: 1, degree: 2, island: 1 }, { bus: 3, degree: 1, island: 1 }, { bus: 4, degree: 2, island: 1 }], bridges: [], articulation_buses: [1], synthetic_articulations: [] },
  } as unknown) as TopologySnapshot;
}

const off = { fields: false, islands: false, merged: false, critical: false, path: false, contingency: false };
const input = (on: Partial<LayerInput["on"]>, extra: Partial<LayerInput> = {}): LayerInput => ({
  on: { ...off, ...on },
  path: null,
  rules: [],
  colors: defaultColorSettings(),
  ...extra,
});

describe("layers", () => {
  it("marks nothing with every layer off", () => {
    const { marks, legends } = resolveLayers(snapshot(), input({}));
    expect(marks.buses.size + marks.elements.size).toBe(0);
    expect(legends).toEqual([]);
  });

  it("rings cut buses", () => {
    const { marks, legends } = resolveLayers(snapshot(), input({ critical: true }));
    expect(marks.buses.get(1)).toMatchObject({ halo: LAYER_COLORS.critical });
    expect(legends.map((l) => l.id)).toEqual(["critical"]);
  });

  it("colours a path's buses and branches, over island colours", () => {
    const path = { buses: [1, 3], equipment_ids: ["transformer:1:3:1"], node_ids: [], hops: 1 } as unknown as LayerInput["path"];
    const { marks } = resolveLayers(snapshot(), input({ islands: true, path: true }, { path }));
    expect(marks.buses.get(3)).toMatchObject({ color: LAYER_COLORS.path, bold: true });
    expect(marks.elements.get("transformer:1:3:1")).toMatchObject({ color: LAYER_COLORS.path });
    // Not on the path: keeps its island colour.
    expect(marks.buses.get(4)?.color).not.toBe(LAYER_COLORS.path);
    expect(marks.buses.get(4)?.note).toBe("Island 1");
  });

  it("fades what the filters leave out", () => {
    const rules = [{ id: "r", target: "bus" as const, field: "base_kv", operator: "eq" as const, value: "69" }];
    const { marks } = resolveLayers(snapshot(), input({ fields: true }, { rules: rules as LayerInput["rules"] }));
    expect(marks.buses.get(1)?.dim).toBe(true);
    expect(marks.buses.get(3)?.dim).toBeUndefined();
  });

  it("rings zero-impedance groups", () => {
    const { marks } = resolveLayers(snapshot(), input({ merged: true }));
    expect(marks.buses.get(2)).toMatchObject({ halo: LAYER_COLORS.merged });
  });

  it("draws an open contingency: the outage, its overloads and moved flows", () => {
    const row = {
      rank: 1, element: "line:1:3:1", kind: "branch", checked: "ac", converged: true, iterations: 2, worst: null, overloads: 1,
      screened_pct: 120, note: null, worst_voltage: null, voltage_violations: 1, cut_off_buses: 0, lost_load_mw: 0, lost_generation_mw: 0, severity: 120,
    } as const;
    const change = (element: string, mw: number, base_mw: number, pct: number | null, base_pct: number | null, tripped = false) =>
      ({ element, from: 1, to: 4, mw, base_mw, pct, base_pct, tripped });
    const detail: ContingencyDetail = {
      row: { ...row },
      overloads: [],
      voltage_problems: [{ bus: 4, name: "", vm: 0.88, base_vm: 0.95, vmin: 0.9, vmax: 1.1, outside: 0.02 }],
      branches: [
        change("line:1:3:1", 0, 50, null, 40, true),
        change("line:1:4:1", 120, 70, 120, 70),
        change("line:3:4:1", -60, -40, null, null),
        change("line:3:4:2", 10, 30, null, null),
        change("line:4:9:1", 101, 100, 105.5, 105),
      ],
      buses: [],
      cut_off: [],
    };
    const { marks, legends } = resolveLayers(snapshot(), input({ contingency: true }, { contingency: { detail, limitPct: 100 } }));
    expect(marks.elements.get("line:1:3:1")).toMatchObject({ halo: LAYER_COLORS.tripped, dim: true });
    expect(marks.elements.get("line:1:4:1")).toMatchObject({ color: LAYER_COLORS.overload, bold: true });
    expect(marks.elements.get("line:3:4:1")).toMatchObject({ color: LAYER_COLORS.more });
    expect(marks.elements.get("line:3:4:2")).toMatchObject({ color: LAYER_COLORS.less });
    // Over the limit before, and less than a point worse: not marked.
    expect(marks.elements.get("line:4:9:1")).toBeUndefined();
    expect(marks.buses.get(4)).toMatchObject({ halo: LAYER_COLORS.voltage });
    expect(legends.map((l) => l.id)).toEqual(["contingency"]);
  });

  it("draws nothing for the contingency layer without an open outage", () => {
    const { marks } = resolveLayers(snapshot(), input({ contingency: true }));
    expect(marks.buses.size + marks.elements.size).toBe(0);
  });
});
