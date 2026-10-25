import { describe, expect, it } from "vitest";
import { busFieldOptions, passingBuses } from "./busFilters";
import type { FilterRule } from "../filters";
import type { BusRow } from "./types";

const bus = (id: number, area: number, base_kv: number, vm: number): BusRow => ({
  id, name: `B${id}`, kind: "PQ", base_kv, vm, va: 0, vmin: 0.95, vmax: 1.05, pd: 0, qd: 0, area, violation: 0,
});
const buses = [bus(1, 1, 138, 1.0), bus(2, 1, 69, 0.97), bus(3, 2, 138, 1.04)];
const rule = (field: string, operator: FilterRule["operator"], value: string, upper?: string): FilterRule => ({
  id: field, target: "bus", field, operator, value, upper,
});

describe("result bus filters", () => {
  it("applies nothing until a rule is finished", () => {
    expect(passingBuses(buses, [])).toBeNull();
    expect(passingBuses(buses, [rule("area", "eq", "")])).toBeNull();
  });

  it("keeps buses passing every rule", () => {
    expect([...passingBuses(buses, [rule("area", "eq", "1")])!]).toEqual([1, 2]);
    expect([...passingBuses(buses, [rule("area", "eq", "1"), rule("base_kv", "eq", "138")])!]).toEqual([1]);
    expect([...passingBuses(buses, [rule("vm", "between", "0.96", "1.01")])!]).toEqual([1, 2]);
  });

  it("offers each value once, sorted", () => {
    expect(busFieldOptions(buses, "base_kv")).toEqual([69, 138]);
  });
});
