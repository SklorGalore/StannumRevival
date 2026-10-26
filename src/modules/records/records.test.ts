import { describe, expect, it } from "vitest";
import type { RecordBook, RecordTable } from "../../lib/analysis/types";
import { cellProblem, columnOptions, csvRows, passes, recordFields, sectionFor } from "./records";

const table: RecordTable = {
  id: "BUS",
  label: "Buses",
  columns: [
    { name: "I", kind: "integer", editable: false },
    { name: "NAME", kind: "text", editable: true },
    { name: "BASKV", kind: "number", editable: true },
    { name: "AREA", kind: "integer", editable: true },
  ],
  rows: [
    { line: 5, element: "bus:101", cells: [101, "NORTH", 138, 1] },
    { line: 6, element: "bus:102", cells: [102, "SOUTH", 345, 2] },
    { line: 7, element: "bus:103", cells: [103, "EAST", 138, 2] },
  ],
  skipped: 0,
  note: null,
};

describe("records helpers", () => {
  it("makes few-valued columns categories and keeps decimals numeric", () => {
    const fields = recordFields(table);
    expect(fields.map((f) => f.type)).toEqual(["category", "category", "number", "category"]);
    expect(fields[3]).toMatchObject({ key: "3", label: "AREA" });
    expect(columnOptions(table, "3")).toEqual([1, 2]);
  });

  it("matches records against rules by column", () => {
    const area2 = { id: "r1", target: "bus" as const, field: "3", operator: "eq" as const, value: "2" };
    const highKv = { id: "r2", target: "bus" as const, field: "2", operator: "gt" as const, value: "200" };
    expect(table.rows.filter((r) => passes(r, [area2])).map((r) => r.cells[0])).toEqual([102, 103]);
    expect(table.rows.filter((r) => passes(r, [area2, highKv])).map((r) => r.cells[0])).toEqual([102]);
    // An unfinished rule doesn't hide anything yet.
    expect(table.rows.filter((r) => passes(r, [{ ...highKv, value: "" }]))).toHaveLength(3);
  });

  it("finds the table that lists an element", () => {
    const raw: RecordBook = { source: "raw", revision: 35, note: null, editable: true, sections: [{ id: "BUS", label: "Buses", count: 3, skipped: 0 }, { id: "BRANCH", label: "Branches", count: 0, skipped: 0 }] };
    expect(sectionFor(raw, "bus:101")).toBe("BUS");
    expect(sectionFor(raw, "line:1:2:1")).toBe("BRANCH");
    expect(sectionFor(raw, "generator:1:1")).toBeNull();
    const model: RecordBook = { source: "model", revision: null, note: null, editable: true, sections: [{ id: "branch", label: "Branches", count: 1, skipped: 0 }] };
    expect(sectionFor(model, "transformer:1:2:1")).toBe("branch");
  });

  it("exports rows by column name with their RAW line", () => {
    expect(csvRows(table, table.rows.slice(0, 1))).toEqual([{ I: 101, NAME: "NORTH", BASKV: 138, AREA: 1, "RAW line": 5 }]);
  });

  it("checks a typed value against the column's kind", () => {
    expect(cellProblem("integer", "12")).toBeNull();
    expect(cellProblem("integer", "1.5")).toMatch(/whole number/);
    expect(cellProblem("number", "-1.2e3")).toBeNull();
    expect(cellProblem("number", "abc")).toMatch(/number/);
    expect(cellProblem("number", "")).toMatch(/number/);
    expect(cellProblem("text", "")).toBeNull();
  });
});
