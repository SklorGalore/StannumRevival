import { describe, expect, it } from "vitest";
import { DEFAULT_REDISPATCH_CHOICE, branchList, isBranch, redispatchLine, redispatchSettings } from "./redispatchStore";
import type { RedispatchReport } from "../lib/analysis/commands.gen";

describe("redispatch settings", () => {
  it("relieves every overload by default", () => {
    const r = redispatchSettings(DEFAULT_REDISPATCH_CHOICE);
    expect(r).toEqual({
      settings: { rating: "a", limit_pct: 100, generators: { areas: [] }, monitor: { kv_min: null }, ac_check: true },
    });
  });

  it("lists the branches to relieve", () => {
    const r = redispatchSettings({ ...DEFAULT_REDISPATCH_CHOICE, only: true, branches: "line:1:2:1, transformer:2:4:1\nline:4:5:1" });
    expect("settings" in r && r.settings.branches).toEqual(["line:1:2:1", "transformer:2:4:1", "line:4:5:1"]);
  });

  it("says what is missing", () => {
    expect(redispatchSettings({ ...DEFAULT_REDISPATCH_CHOICE, only: true })).toHaveProperty("problem");
    expect(redispatchSettings({ ...DEFAULT_REDISPATCH_CHOICE, limitPct: 0 })).toHaveProperty("problem");
    expect(redispatchSettings({ ...DEFAULT_REDISPATCH_CHOICE, limitPct: null })).toHaveProperty("problem");
  });

  it("knows branches from other elements", () => {
    expect(isBranch("line:1:2:1")).toBe(true);
    expect(isBranch("transformer:2:4:0:1")).toBe(true);
    expect(isBranch("generator:1:1")).toBe(false);
    expect(isBranch("bus:4")).toBe(false);
    expect(branchList(" a ,b;; c ")).toEqual(["a", "b", "c"]);
  });

  it("logs one line", () => {
    const report = {
      summary: { units_moved: 2, moved_mw: 11.2, over_before: 1, over_after: 0, worst_after: { name: "8 9 1", pct_after: 99.98 } },
    } as unknown as RedispatchReport;
    expect(redispatchLine(report)).toBe("Redispatched 11.2 MW across 2 generators: 1 → 0 branches over their limit; worst 8 9 1 100.0%");
  });
});
