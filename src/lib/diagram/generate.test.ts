import { describe, expect, it } from "vitest";
import { selectBuses, type Scope } from "./generate";
import type { CaseNetwork, NetworkBus } from "../analysis/types";
import type { FilterRule } from "../filters";

const bus = (id: number, base_kv: number, area: number, zone: number, owner: number, star = false): NetworkBus => ({
  id, name: `B${id}`, base_kv, area, zone, owner, kind: "PQ", pd: 0, qd: 0, gs: 0, bs: 0, star,
});
const line = (index: number, from: number, to: number) => ({
  index, element: `line:${from}:${to}:1`, circuit: "1", winding: 0, from, to,
  online: true, transformer: false, rate_a: 100, tap: 1, shift: 0,
});
// 1 -(345)- 2 -(345)- 3, with a 138 kV bus 4 off bus 2 and bus 5 off bus 4.
const net: CaseNetwork = {
  base_mva: 100,
  buses: [bus(1, 345, 1, 10, 7), bus(2, 345, 1, 11, 7), bus(3, 345, 2, 11, 8), bus(4, 138, 2, 12, 8), bus(5, 345, 2, 12, 8)],
  generators: [],
  branches: [line(0, 1, 2), line(1, 2, 3), line(2, 2, 4), line(3, 4, 5)],
  dc_lines: [],
};
const rule = (field: string, operator: FilterRule["operator"], value: string): FilterRule => ({
  id: field, target: "bus", field, operator, value,
});
const pick = (scope: Scope) => [...selectBuses(net, scope)].sort((a, b) => a - b);

describe("selectBuses", () => {
  it("draws one area, zone or owner", () => {
    expect(pick({ kind: "area", value: 2 })).toEqual([3, 4, 5]);
    expect(pick({ kind: "zone", value: 11 })).toEqual([2, 3]);
    expect(pick({ kind: "owner", value: 7 })).toEqual([1, 2]);
  });

  it("filters the whole case and a group", () => {
    expect(pick({ kind: "all", filters: [rule("base_kv", "gte", "200")] })).toEqual([1, 2, 3, 5]);
    expect(pick({ kind: "area", value: 2, filters: [rule("zone", "eq", "12")] })).toEqual([4, 5]);
    expect(pick({ kind: "all", filters: [rule("owner", "neq", "8")] })).toEqual([1, 2]);
  });

  it("grows a local drawing only through buses that pass", () => {
    expect(pick({ kind: "around", bus: 2, depth: 2 })).toEqual([1, 2, 3, 4, 5]);
    // Bus 5 is 345 kV but only reachable through the 138 kV bus 4.
    expect(pick({ kind: "around", bus: 2, depth: 2, filters: [rule("base_kv", "eq", "345")] })).toEqual([1, 2, 3]);
    // The studied bus is drawn even when it fails the filters.
    expect(pick({ kind: "around", bus: 4, depth: 1, filters: [rule("base_kv", "eq", "345")] })).toEqual([2, 4, 5]);
  });
});
