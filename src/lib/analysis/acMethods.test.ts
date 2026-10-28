import { describe, expect, it } from "vitest";
import { AC_METHODS, controlsDetail, iterationSplit, methodLabel } from "./acMethods";
import { DEFAULT_SOLVE_SETTINGS } from "./api";

describe("AC methods", () => {
  it("defaults to Auto and offers every method once", () => {
    expect(DEFAULT_SOLVE_SETTINGS.method).toBe("auto");
    expect(new Set(AC_METHODS.map((m) => m.value)).size).toBe(5);
    expect(methodLabel("fixed_slope")).toBe("Fixed-slope decoupled (FDNS)");
  });

  it("reads reports saved before the choice as Newton", () => {
    expect(methodLabel(undefined)).toBe("Full Newton");
    expect(iterationSplit({ iterations: 4 } as never)).toBe("");
  });

  it("splits Auto's iterations into decoupled and Newton", () => {
    expect(iterationSplit({ iterations: 14, decoupled_iterations: 11 })).toBe("11 decoupled + 3 Newton");
    expect(iterationSplit({ iterations: 11, decoupled_iterations: 11 })).toBe("11 decoupled");
    expect(iterationSplit({ iterations: 5, decoupled_iterations: 0 })).toBe("");
  });

  it("says when voltage controls were locked", () => {
    const base = { voltage_control_passes: 0, shunts_locked: false, taps_locked: false };
    expect(controlsDetail(base)).toBe("Moved from their case values");
    expect(controlsDetail({ ...base, voltage_control_passes: 2 })).toBe("Moved, over 2 switching passes");
    expect(controlsDetail({ ...base, taps_locked: true })).toBe("Moved from their case values; taps locked");
    expect(controlsDetail({ ...base, shunts_locked: true, taps_locked: true })).toBe("Locked; counted from their case values");
  });
});
