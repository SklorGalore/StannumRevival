// Helpers for the Records module: filter fields from a table's columns, rule
// matching on a record, and which table lists an element.
import { distinctValues, matchesValue, ruleReady, type FieldDefinition, type FieldValue, type FilterRule } from "../../lib/filters";
import { elementSections, elementTable } from "../../lib/elements";
import type { CaseRecord, RecordBook, RecordColumnKind, RecordTable } from "../../lib/analysis/types";

/** Columns with this many distinct values or fewer filter by picking a value. */
export const CATEGORY_LIMIT = 40;

/** One filter field per column; keys are column positions. */
export function recordFields(table: RecordTable): FieldDefinition[] {
  return table.columns.map((c, i) => {
    let type: FieldDefinition["type"] = c.kind === "text" ? "text" : "number";
    if (c.kind !== "number") {
      const seen = new Set<FieldValue>();
      for (const r of table.rows) {
        seen.add(r.cells[i] ?? null);
        if (seen.size > CATEGORY_LIMIT) break;
      }
      if (seen.size <= CATEGORY_LIMIT) type = "category";
    }
    return { key: String(i), label: c.name, type };
  });
}

/** Distinct values of one column, for a category filter. */
export function columnOptions(table: RecordTable, key: string): FieldValue[] {
  const i = Number(key);
  return distinctValues(table.rows.map((r) => r.cells[i] ?? null));
}

/** Whether a record passes every finished rule. */
export function passes(record: CaseRecord, rules: FilterRule[]): boolean {
  return rules.every((rule) => !ruleReady(rule) || matchesValue(record.cells[Number(rule.field)] ?? null, rule));
}

/** The table that lists an element, if the case has one. */
export function sectionFor(book: RecordBook, element: string): string | null {
  const ids = book.source === "raw" ? elementSections(element) : [elementTable(element)];
  // A DC line may be two-terminal or VSC: prefer the section that has records.
  const has = (id: string | null, filled: boolean) => !!id && book.sections.some((s) => s.id === id && (!filled || s.count > 0));
  return ids.find((id) => has(id, true)) ?? ids.find((id) => has(id, false)) ?? null;
}

/** The records as plain objects keyed by column name, for CSV export. */
export function csvRows(table: Pick<RecordTable, "columns">, rows: CaseRecord[]): Record<string, unknown>[] {
  return rows.map((r) => {
    const out: Record<string, unknown> = {};
    table.columns.forEach((c, i) => (out[c.name] = r.cells[i]));
    if (r.line > 0) out["RAW line"] = r.line;
    return out;
  });
}

/** Why a typed value doesn't fit its column, or null when it does (the backend checks again). */
export function cellProblem(kind: RecordColumnKind, text: string): string | null {
  const t = text.trim();
  if (kind === "text") return /[\r\n]/.test(text) ? "Text can't span lines." : null;
  if (kind === "integer") return /^[+-]?\d+$/.test(t) ? null : "Enter a whole number.";
  return t !== "" && Number.isFinite(Number(t)) ? null : "Enter a number.";
}
