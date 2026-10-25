// Field filters shared by Topology and the result tables: a rule compares one
// field with a value, and a row passes when it passes every rule.
// The matching rules come from nodal's web/field-view.ts.

export type FieldValue = string | number | null;
export interface FieldDefinition {
  key: string;
  label: string;
  type: "number" | "text" | "category";
}
export type FilterOperator = "eq" | "neq" | "contains" | "gt" | "gte" | "lt" | "lte" | "between" | "missing" | "present";
export type FieldTarget = "bus" | "equipment";
export interface FilterRule {
  id: string;
  target: FieldTarget;
  field: string;
  operator: FilterOperator;
  value: string;
  upper?: string;
}

export const OPERATORS: Record<FieldDefinition["type"], { id: FilterOperator; label: string }[]> = {
  number: [
    { id: "eq", label: "=" },
    { id: "neq", label: "≠" },
    { id: "gt", label: ">" },
    { id: "gte", label: "≥" },
    { id: "lt", label: "<" },
    { id: "lte", label: "≤" },
    { id: "between", label: "between" },
    { id: "missing", label: "is missing" },
    { id: "present", label: "is present" },
  ],
  text: [
    { id: "contains", label: "contains" },
    { id: "eq", label: "is" },
    { id: "neq", label: "is not" },
    { id: "missing", label: "is missing" },
    { id: "present", label: "is present" },
  ],
  category: [
    { id: "eq", label: "is" },
    { id: "neq", label: "is not" },
    { id: "missing", label: "is missing" },
    { id: "present", label: "is present" },
  ],
};

/** Whether a rule needs a value typed or picked. */
export const needsValue = (op: FilterOperator) => op !== "missing" && op !== "present";

/** Whether a rule is complete enough to apply. */
export function ruleReady(rule: FilterRule): boolean {
  if (!needsValue(rule.operator)) return true;
  if (!rule.value.trim()) return false;
  return rule.operator !== "between" || !!rule.upper?.trim();
}

function numeric(value: string): number | null {
  if (!value.trim()) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function matchesValue(actual: FieldValue, rule: FilterRule): boolean {
  if (rule.operator === "missing") return actual === null;
  if (rule.operator === "present") return actual !== null;
  if (actual === null) return false;
  if (typeof actual === "number") {
    const value = numeric(rule.value);
    if (value === null) return false;
    switch (rule.operator) {
      case "eq": return actual === value;
      case "neq": return actual !== value;
      case "gt": return actual > value;
      case "gte": return actual >= value;
      case "lt": return actual < value;
      case "lte": return actual <= value;
      case "between": {
        const upper = rule.upper === undefined ? null : numeric(rule.upper);
        return upper !== null && upper >= value && actual >= value && actual <= upper;
      }
      default: return false;
    }
  }
  const text = String(actual);
  switch (rule.operator) {
    case "eq": return text === rule.value;
    case "neq": return text !== rule.value;
    case "contains": return text.toLocaleLowerCase().includes(rule.value.toLocaleLowerCase());
    default: return false;
  }
}

/** A rule in words, for its chip: "Area is 2", "Base kV between 100 and 200". */
export function ruleLabel(rule: FilterRule, fields: FieldDefinition[]): string {
  const def = fields.find((f) => f.key === rule.field);
  const op = def ? OPERATORS[def.type].find((o) => o.id === rule.operator)?.label : rule.operator;
  const name = def?.label ?? rule.field;
  if (!needsValue(rule.operator)) return `${name} ${op}`;
  if (rule.operator === "between") return `${name} between ${rule.value} and ${rule.upper ?? ""}`;
  return `${name} ${op} ${rule.value}`;
}

/** Distinct values of a field, sorted, missing last. */
export function distinctValues(values: Iterable<FieldValue>): FieldValue[] {
  return [...new Set(values)].sort((a, b) => {
    if (a === null) return b === null ? 0 : 1;
    if (b === null) return -1;
    if (typeof a === "number" && typeof b === "number") return a - b;
    return String(a).localeCompare(String(b), undefined, { sensitivity: "variant", numeric: true });
  });
}

let nextRule = 1;

export function newRule(fields: FieldDefinition[], target: FieldTarget = "bus"): FilterRule {
  const field = fields[0];
  return { id: `rule-${nextRule++}`, target, field: field.key, operator: OPERATORS[field.type][0].id, value: "" };
}
