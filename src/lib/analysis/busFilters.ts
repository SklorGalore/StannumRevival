// Filter fields for result tables. Every table row belongs to one or two
// buses, so one set of bus rules narrows buses, generators and branches alike.
import { distinctValues, matchesValue, ruleReady, type FieldDefinition, type FieldValue, type FilterRule } from "../filters";
import type { BusRow } from "./types";

export const RESULT_BUS_FIELDS: FieldDefinition[] = [
  { key: "area", label: "Area", type: "category" },
  { key: "base_kv", label: "Base kV", type: "category" },
  { key: "kind", label: "Bus type", type: "category" },
  { key: "number", label: "Bus number", type: "number" },
  { key: "name", label: "Bus name", type: "text" },
  { key: "vm", label: "Voltage (p.u.)", type: "number" },
];

export function busFieldValue(bus: BusRow, field: string): FieldValue {
  switch (field) {
    case "area": return bus.area;
    case "base_kv": return bus.base_kv;
    case "kind": return bus.kind;
    case "number": return bus.id;
    case "name": return bus.name.trim() || null;
    case "vm": return bus.vm;
    default: return null;
  }
}

/** The buses passing every finished rule, or null when no rule applies. */
export function passingBuses(buses: BusRow[], rules: FilterRule[]): Set<number> | null {
  const ready = rules.filter(ruleReady);
  if (!ready.length) return null;
  return new Set(buses.filter((b) => ready.every((r) => matchesValue(busFieldValue(b, r.field), r))).map((b) => b.id));
}

export function busFieldOptions(buses: BusRow[], field: string): FieldValue[] {
  return distinctValues(buses.map((b) => busFieldValue(b, field)));
}
