import { describe, expect, it } from "vitest";
import { flowWidth, voltageBand } from "./detail";

describe("result marks", () => {
  it("bands bus voltages against their limits", () => {
    const limits = { vmin: 0.95, vmax: 1.05 };
    expect(voltageBand({ ...limits, vm: 1.0, violation: 0 })).toBeNull();
    expect(voltageBand({ ...limits, vm: 0.955, violation: 0 })).toBe("near");
    expect(voltageBand({ ...limits, vm: 1.045, violation: 0 })).toBe("near");
    expect(voltageBand({ ...limits, vm: 0.93, violation: 0.02 })).toBe("past");
  });

  it("ignores missing limits", () => {
    expect(voltageBand({ vmin: 0, vmax: 0, vm: 1.0, violation: 0 })).toBeNull();
  });

  it("weights wires by loading, capped", () => {
    expect(flowWidth(50, 10, 100, 2)).toBe(4);
    expect(flowWidth(300, 10, 100, 2)).toBeCloseTo(6.8);
  });

  it("weights unrated wires by their share of the largest flow", () => {
    expect(flowWidth(null, -50, 200, 2)).toBe(3);
    expect(flowWidth(null, 50, 0, 2)).toBe(2);
  });
});
