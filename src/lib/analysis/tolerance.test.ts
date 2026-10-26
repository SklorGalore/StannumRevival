import { describe, expect, it } from "vitest";
import { DEFAULT_SOLVE_SETTINGS } from "./api";
import { toleranceMva, tolerancePu } from "./tolerance";

describe("mismatch tolerance in MVA", () => {
  it("defaults to 2 MVA on a 100 MVA base", () => {
    expect(toleranceMva(DEFAULT_SOLVE_SETTINGS.tolerance, 100)).toBe(2);
  });

  it("converts on a 100 MVA base without float noise", () => {
    expect(toleranceMva(1e-8, 100)).toBe(1e-6);
    expect(tolerancePu(0.1, 100)).toBe(0.001);
  });

  it("round-trips on other bases", () => {
    for (const base of [100, 1000, 33.3]) {
      for (const mva of [0.1, 0.01, 1e-6, 5]) expect(toleranceMva(tolerancePu(mva, base), base)).toBe(mva);
    }
  });

  it("falls back to 100 MVA when the base is missing", () => {
    expect(tolerancePu(0.1, 0)).toBe(0.001);
  });
});
