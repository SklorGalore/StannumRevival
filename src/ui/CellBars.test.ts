import { describe, expect, it } from "vitest";
import { trackPosition } from "./CellBars";

describe("trackPosition", () => {
  it("places a value on its track, clamped", () => {
    expect(trackPosition(1.0, 0.9, 1.1)).toBeCloseTo(0.5);
    expect(trackPosition(0.8, 0.9, 1.1)).toBe(0);
    expect(trackPosition(1.3, 0.9, 1.1)).toBe(1);
  });

  it("centres a value on an empty track", () => {
    expect(trackPosition(5, 1, 1)).toBe(0.5);
  });
});
