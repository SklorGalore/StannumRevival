import { describe, expect, it } from "vitest";
import { T3, T3_TERMINALS, t3Terminal } from "./transformer3";

describe("three-winding transformer symbol", () => {
  it("puts its terminals on the 20 px grid", () => {
    for (const t of T3_TERMINALS) {
      expect(t.x % 20).toBe(0);
      expect(t.y % 20).toBe(0);
    }
  });

  it("has one terminal on top upright and two upside down", () => {
    const up = (r: 0 | 180) => T3_TERMINALS.filter((t) => t3Terminal(t.id, r).up).map((t) => t.id);
    expect(up(0)).toEqual(["x3-a"]);
    expect(up(180).sort()).toEqual(["x3-b", "x3-c"]);
    // Upside down, the left winding terminal lands on the right.
    expect(t3Terminal("x3-b", 180)).toEqual({ x: 60, y: 0, up: true });
    expect(t3Terminal("x3-a", 180)).toEqual({ x: 40, y: 100, up: false });
  });

  it("meets each bottom stem on its circle", () => {
    const { left } = T3.circles;
    expect(Math.hypot(20 - left.x, T3.legY - left.y)).toBeCloseTo(T3.r, 6);
  });
});
