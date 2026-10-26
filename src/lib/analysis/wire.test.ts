import { describe, expect, it } from "vitest";
import { decodeSolveReport, fromColumns } from "./wire";
import type { SolveReport } from "./types";

describe("fromColumns", () => {
  it("turns columns back into rows", () => {
    expect(
      fromColumns<{ id: number; name: string; loading_pct: number | null }>({
        count: 2,
        id: [1, 2],
        name: ["A", "B"],
        loading_pct: [null, 50],
      }),
    ).toEqual([
      { id: 1, name: "A", loading_pct: null },
      { id: 2, name: "B", loading_pct: 50 },
    ]);
  });

  it("handles an empty table and passes rows through", () => {
    expect(fromColumns<{ id: number }>({ count: 0, id: [] })).toEqual([]);
    const rows = [{ id: 1 }];
    expect(fromColumns(rows)).toBe(rows);
  });

  it("decodes a report and keeps the other fields", () => {
    const wire = {
      converged: true,
      buses: { count: 1, id: [7] },
      generators: { count: 0, index: [] },
      branches: { count: 1, element: ["line:1:2:1"] },
    } as unknown as SolveReport;
    const report = decodeSolveReport(wire);
    expect(report.converged).toBe(true);
    expect(report.buses).toEqual([{ id: 7 }]);
    expect(report.generators).toEqual([]);
    expect(report.branches[0].element).toBe("line:1:2:1");
  });
});
