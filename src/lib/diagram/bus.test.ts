import { describe, expect, it } from "vitest";
import { gridAlong, tapPoint } from "./bus";
import { defaultTaps } from "../../components/nodes/shared/busTaps";
import type { SLDNode } from "../../store/types";

const bus = (x: number, y: number, length: number, orientation = "horizontal") =>
  ({ id: "b", type: "bus", position: { x, y }, data: { label: "", voltageKV: 138, length, orientation } }) as unknown as SLDNode;

describe("gridAlong", () => {
  it("puts a tap on the drawing grid, not a step from the bar's start", () => {
    // A bar starting off the grid (from a layout): the tap lands on x = 160.
    const b = bus(107, 424, 200);
    const x = gridAlong(b, { x: 163, y: 430 }, 20);
    expect(tapPoint(b, x, "bottom").x).toBe(160);
  });

  it("keeps the tap on the bar", () => {
    const b = bus(107, 424, 200);
    expect(tapPoint(b, gridAlong(b, { x: 90, y: 430 }, 20), "top").x).toBe(120);
    expect(tapPoint(b, gridAlong(b, { x: 400, y: 430 }, 20), "top").x).toBe(300);
  });

  it("runs down a vertical bar", () => {
    const b = bus(100, 213, 200, "vertical");
    expect(tapPoint(b, gridAlong(b, { x: 106, y: 258 }, 20), "top").y).toBe(260);
  });

  it("falls back to the nearest point on a bar shorter than a step", () => {
    const b = bus(101, 0, 10);
    expect(gridAlong(b, { x: 105, y: 0 }, 20)).toBe(4);
  });
});

describe("defaultTaps", () => {
  it("stays on whole grid steps when a bar is lengthened", () => {
    for (const len of [200, 220, 260, 340])
      for (const t of defaultTaps(len)) {
        expect(t.x % 20).toBe(0);
        expect(t.x).toBeLessThanOrEqual(len);
      }
    expect(defaultTaps(200).map((t) => t.x)).toEqual([100, 40, 80, 120, 160]);
  });
});
