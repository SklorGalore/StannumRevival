import { describe, expect, it } from "vitest";
import { allRows, objectKey, type PageRequest, type RowSource } from "./usePagedRows";

describe("paged rows", () => {
  it("reads every matched row for export, page by page", async () => {
    const rows = Array.from({ length: 12_345 }, (_, i) => i);
    const asked: PageRequest[] = [];
    const source: RowSource<number> = {
      key: "k",
      fetch: async (r) => {
        asked.push(r);
        return { matched: rows.length, total: rows.length, offset: r.offset, rows: rows.slice(r.offset, r.offset + r.limit), found: null };
      },
    };
    const all = await allRows(source, { field: "x", descending: true }, "q");
    expect(all).toEqual(rows);
    expect(asked.map((r) => r.offset)).toEqual([0, 5000, 10000]);
    expect(asked.every((r) => r.search === "q" && r.sort?.descending)).toBe(true);
  });

  it("numbers objects once", () => {
    const a = {};
    const b = {};
    expect(objectKey(a)).toBe(objectKey(a));
    expect(objectKey(a)).not.toBe(objectKey(b));
  });
});
