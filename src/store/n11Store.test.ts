import { describe, expect, it } from "vitest";
import { DEFAULT_N11, n11Settings, pairRows } from "./n11Store";
import type { PairRow } from "../lib/analysis/commands.gen";

describe("N-1-1 settings", () => {
  it("takes everything out first and second, redispatching between", () => {
    expect(n11Settings(DEFAULT_N11)).toEqual({
      settings: {
        adjustment: "redispatch",
        contingencies: { rating: "a", limit_pct: 100, branches: true, generators: true },
        redispatch: { rating: "a", limit_pct: 100 },
      },
    });
  });

  it("lists the first outages", () => {
    const r = n11Settings({ ...DEFAULT_N11, listFirst: true, first: "line:1:2:1, generator:3:1", adjustment: "none" });
    expect("settings" in r && r.settings.first).toEqual(["line:1:2:1", "generator:3:1"]);
    expect("settings" in r && r.settings.adjustment).toBe("none");
  });

  it("says what is missing", () => {
    expect(n11Settings({ ...DEFAULT_N11, listFirst: true })).toHaveProperty("problem");
    expect(n11Settings({ ...DEFAULT_N11, limitPct: 0 })).toHaveProperty("problem");
    expect(n11Settings({ ...DEFAULT_N11, branches: false, generators: false })).toHaveProperty("problem");
  });

  it("exports a row per pair", () => {
    const pair = {
      rank: 1,
      first: "line:1:2:1",
      first_name: "A 1 B 2 1",
      second: "line:2:3:1",
      second_name: "B 2 C 3 1",
      checked: "ac",
      converged: true,
      overloads: 1,
      worst: { element: "line:1:3:1", name: "A 1 C 3 1", from: 1, to: 3, pct: 112.346, base_pct: 80.1, mw: 1, base_mw: 1, limit_pct: 100 },
      voltage_violations: 0,
      cut_off_buses: 0,
      lost_load_mw: 0,
      severity: 12,
    } as unknown as PairRow;
    const [row] = pairRows([pair]);
    expect(row).toMatchObject({ rank: 1, first: "line:1:2:1", second: "line:2:3:1", worst_branch: "A 1 C 3 1", worst_pct: 112.35, converged: "yes" });
  });
});
