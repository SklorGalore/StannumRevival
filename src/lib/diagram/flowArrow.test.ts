import { describe, expect, it } from "vitest";
import { getSmoothStepPath, Position } from "@xyflow/react";
import { arrowPath, flowDirection, pathPoints } from "./flowArrow";

/** Every edge of the triangle's base is square to the direction of travel. */
function triangle(d: string) {
  const n = (d.match(/-?\d*\.?\d+/g) ?? []).map(Number);
  return { tip: { x: n[0], y: n[1] }, l: { x: n[2], y: n[3] }, r: { x: n[4], y: n[5] } };
}

describe("flowDirection", () => {
  it("follows a horizontal run", () => {
    expect(flowDirection({ x: 0, y: 0 }, { x: 40, y: 0 }, false)).toEqual({ x: 1, y: 0 });
    expect(flowDirection({ x: 0, y: 0 }, { x: 40, y: 0 }, true)).toEqual({ x: -1, y: 0 });
  });
  it("follows a vertical run", () => {
    expect(flowDirection({ x: 5, y: 50 }, { x: 5, y: 10 }, false)).toEqual({ x: 0, y: -1 });
  });
  it("never points diagonally between offset ends", () => {
    // The old arrow used sign(dx), sign(dy) of the wire's ends: (1, 1) here.
    const d = flowDirection({ x: 0, y: 0 }, { x: 100, y: 30 }, false);
    expect(d.x * d.y).toBe(0);
    expect(d).toEqual({ x: 1, y: 0 });
    expect(flowDirection({ x: 0, y: 0 }, { x: 0.4, y: 80 }, false)).toEqual({ x: 0, y: 1 });
  });
});

describe("arrowPath", () => {
  it("draws a triangle whose base is square to the run", () => {
    const { tip, l, r } = triangle(arrowPath({ x: 10, y: 20 }, { x: 0, y: 1 }));
    expect(tip).toEqual({ x: 10, y: 25 });
    expect(l.y).toBe(r.y);
    expect(tip.x).toBe((l.x + r.x) / 2);
  });
});

describe("pathPoints", () => {
  it("recovers the corners of a step path", () => {
    const [d] = getSmoothStepPath({
      sourceX: 0,
      sourceY: 0,
      sourcePosition: Position.Bottom,
      targetX: 100,
      targetY: 80,
      targetPosition: Position.Top,
      borderRadius: 0,
    });
    const pts = pathPoints(d);
    expect(pts[0]).toEqual({ x: 0, y: 0 });
    expect(pts[pts.length - 1]).toEqual({ x: 100, y: 80 });
    for (let i = 0; i < pts.length - 1; i++) {
      const [a, b] = [pts[i], pts[i + 1]];
      expect(a.x === b.x || a.y === b.y).toBe(true);
    }
  });
});
