import { describe, expect, it } from "vitest";
import { BoxIndex, bends, isOrthogonal, moveSegment, pinEnds, routeWire, type Box, type Pt } from "./router";

const PAD = 10;

/** True when any segment of the route passes through the inside of a box. */
function hits(route: Pt[], b: Box, pad = 0) {
  const x0 = b.x - pad;
  const x1 = b.x + b.w + pad;
  const y0 = b.y - pad;
  const y1 = b.y + b.h + pad;
  for (let i = 0; i < route.length - 1; i++) {
    const p = route[i];
    const q = route[i + 1];
    const lx = Math.min(p.x, q.x);
    const hx = Math.max(p.x, q.x);
    const ly = Math.min(p.y, q.y);
    const hy = Math.max(p.y, q.y);
    if (hx > x0 && lx < x1 && hy > y0 && ly < y1) return true;
  }
  return false;
}

describe("routeWire", () => {
  it("runs straight when the ends line up", () => {
    const r = routeWire({ x: 100, y: 0, side: "bottom" }, { x: 100, y: 200, side: "top" }, []);
    expect(r.points).toEqual([
      { x: 100, y: 0 },
      { x: 100, y: 200 },
    ]);
    expect(r.blocked).toBe(false);
  });

  it("uses two bends for offset ends facing each other", () => {
    const r = routeWire({ x: 100, y: 0, side: "bottom" }, { x: 200, y: 200, side: "top" }, []);
    expect(isOrthogonal(r.points)).toBe(true);
    expect(bends(r.points)).toBe(2);
  });

  it("uses one bend between a bottom and a left terminal", () => {
    const r = routeWire({ x: 100, y: 0, side: "bottom" }, { x: 300, y: 200, side: "left" }, []);
    expect(bends(r.points)).toBe(1);
    expect(r.points[1]).toEqual({ x: 100, y: 200 });
  });

  it("detours around a symbol in the way", () => {
    const sym: Box = { x: 80, y: 80, w: 40, h: 60 };
    const r = routeWire({ x: 100, y: 0, side: "bottom" }, { x: 100, y: 240, side: "top" }, [sym]);
    expect(isOrthogonal(r.points)).toBe(true);
    expect(hits(r.points, sym, PAD - 0.01)).toBe(false);
    expect(r.blocked).toBe(false);
    expect(bends(r.points)).toBe(4);
  });

  it("detours around a wide label as well as the symbol", () => {
    const sym: Box = { x: 80, y: 80, w: 40, h: 60 };
    const label: Box = { x: 122, y: 90, w: 90, h: 30 };
    const r = routeWire({ x: 140, y: 0, side: "bottom" }, { x: 140, y: 240, side: "top" }, [sym, label]);
    expect(hits(r.points, sym)).toBe(false);
    expect(hits(r.points, label)).toBe(false);
  });

  it("leaves a terminal in its own direction, clear of its symbol", () => {
    // A generator: terminal on top, label to the right. The target is below it.
    const gen: Box = { x: 0, y: 0, w: 40, h: 80 };
    const r = routeWire({ x: 20, y: 0, side: "top" }, { x: 20, y: 200, side: "top" }, [gen]);
    expect(r.points[1].x).toBe(20);
    expect(r.points[1].y).toBeLessThan(0);
    expect(hits(r.points.slice(1), gen)).toBe(false);
  });

  it("doesn't cut through a bus bar just below the symbol", () => {
    const xfmr: Box = { x: 0, y: 0, w: 40, h: 100 };
    const bar: Box = { x: -100, y: 124, w: 300, h: 6 };
    const r = routeWire({ x: 20, y: 100, side: "bottom" }, { x: -200, y: 300, side: "top" }, [xfmr, bar]);
    expect(hits(r.points, bar)).toBe(false);
    expect(hits(r.points.slice(1), xfmr)).toBe(false);
  });

  it("goes round a thin bar sitting right under the terminal", () => {
    // Only 6 px between the symbol and the bar: no room for clearance.
    const xfmr: Box = { x: 0, y: 0, w: 40, h: 100 };
    const bar: Box = { x: -100, y: 106, w: 300, h: 6 };
    const r = routeWire({ x: 20, y: 100, side: "bottom" }, { x: -200, y: 300, side: "top" }, [xfmr, bar]);
    expect(hits(r.points, bar)).toBe(false);
    expect(r.blocked).toBe(false);
  });

  it("counts wires just off the grid as on it", () => {
    // A layout wire at x = 101: running down it should still cost extra.
    const along = [
      { x: 101, y: -40 },
      { x: 101, y: 240 },
    ];
    const r = routeWire({ x: 100, y: 0, side: "bottom" }, { x: 100, y: 200, side: "top" }, [], [along]);
    expect(r.points.length).toBeGreaterThan(2);
  });

  it("handles terminals on every side", () => {
    const sides = ["top", "bottom", "left", "right"] as const;
    for (const sa of sides)
      for (const sb of sides) {
        const r = routeWire({ x: 0, y: 0, side: sa }, { x: 300, y: 160, side: sb }, []);
        expect(isOrthogonal(r.points), `${sa}→${sb}`).toBe(true);
        expect(r.points[0]).toEqual({ x: 0, y: 0 });
        expect(r.points[r.points.length - 1]).toEqual({ x: 300, y: 160 });
        // The first step leaves along the terminal's side.
        const d = { x: r.points[1].x - r.points[0].x, y: r.points[1].y - r.points[0].y };
        const want = { top: d.y < 0, bottom: d.y > 0, left: d.x < 0, right: d.x > 0 }[sa];
        expect(want, `${sa}→${sb} first step`).toBe(true);
      }
  });

  it("still returns a path when the target is boxed in", () => {
    const wall: Box[] = [
      { x: 140, y: 140, w: 120, h: 20 },
      { x: 140, y: 240, w: 120, h: 20 },
      { x: 140, y: 140, w: 20, h: 120 },
      { x: 240, y: 140, w: 20, h: 120 },
    ];
    const r = routeWire({ x: 0, y: 0, side: "bottom" }, { x: 200, y: 200, side: "top" }, wall);
    expect(r.blocked).toBe(true);
    expect(isOrthogonal(r.points)).toBe(true);
    expect(r.points[r.points.length - 1]).toEqual({ x: 200, y: 200 });
  });

  it("goes the long way round a box with a gap behind it", () => {
    // A U-shaped wall open at the far side: the route must leave the margin.
    const wall: Box[] = [
      { x: -200, y: 60, w: 400, h: 20 },
      { x: -200, y: -200, w: 20, h: 260 },
      { x: 180, y: -200, w: 20, h: 260 },
    ];
    const r = routeWire({ x: 0, y: 0, side: "bottom" }, { x: 0, y: 160, side: "top" }, wall);
    expect(r.blocked).toBe(false);
    for (const w of wall) expect(hits(r.points, w)).toBe(false);
  });

  it("prefers crossing a wire to running along it", () => {
    const other = [
      { x: 60, y: 100 },
      { x: 60, y: -100 },
    ];
    const r = routeWire({ x: 0, y: 0, side: "right" }, { x: 120, y: 0, side: "left" }, [], [other]);
    expect(r.points).toEqual([
      { x: 0, y: 0 },
      { x: 120, y: 0 },
    ]);
    // Running down the same line as another wire costs more than stepping aside.
    const along = [
      { x: 100, y: -40 },
      { x: 100, y: 240 },
    ];
    const r2 = routeWire({ x: 100, y: 0, side: "bottom" }, { x: 100, y: 200, side: "top" }, [], [along]);
    expect(r2.points.length).toBeGreaterThan(2);
  });

  it("routes a wire on a case300-sized drawing in under 10 ms", () => {
    // 700 symbols in rows, like a generated whole-case drawing.
    const boxes: Box[] = [];
    let seed = 7;
    const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
    for (let row = 0; row < 14; row++)
      for (let col = 0; col < 50; col++)
        boxes.push({ x: col * 160 + rnd() * 40, y: row * 220 + rnd() * 40, w: 40 + rnd() * 60, h: 60 + rnd() * 40 });
    const index = new BoxIndex(boxes);
    const wires: Pt[][] = [];
    for (let k = 0; k < 800; k++) {
      const x = Math.round(rnd() * 400) * 20;
      const y = Math.round(rnd() * 150) * 20;
      wires.push([
        { x, y },
        { x, y: y + 200 },
        { x: x + 300, y: y + 200 },
      ]);
    }
    const runs = 60;
    const t0 = performance.now();
    for (let k = 0; k < runs; k++) {
      // Typical wires reach a few rows and columns away.
      const sx = 10 + Math.floor(rnd() * 45) * 160;
      const sy = 10 + Math.floor(rnd() * 12) * 220;
      const tx = sx + (Math.floor(rnd() * 7) - 3) * 160;
      const ty = sy + Math.floor(1 + rnd() * 2) * 220;
      routeWire({ x: sx, y: sy, side: "bottom" }, { x: tx, y: ty, side: "top" }, index, wires);
    }
    const each = (performance.now() - t0) / runs;
    expect(each).toBeLessThan(10);
  });
});

describe("moveSegment", () => {
  const z = [
    { x: 0, y: 0 },
    { x: 0, y: 100 },
    { x: 200, y: 100 },
    { x: 200, y: 200 },
  ];

  it("slides a middle segment and stretches its neighbours", () => {
    expect(moveSegment(z, 1, 40)).toEqual([
      { x: 0, y: 0 },
      { x: 0, y: 140 },
      { x: 200, y: 140 },
      { x: 200, y: 200 },
    ]);
  });

  it("keeps a stub on the terminal when moving an end segment", () => {
    const out = moveSegment(z, 0, 60);
    expect(out[0]).toEqual({ x: 0, y: 0 });
    expect(out[1]).toEqual({ x: 0, y: 10 });
    expect(out[2]).toEqual({ x: 60, y: 10 });
    expect(isOrthogonal(out)).toBe(true);
    expect(out[out.length - 1]).toEqual({ x: 200, y: 200 });
  });

  it("splits a straight wire into a jog", () => {
    const out = moveSegment(
      [
        { x: 0, y: 0 },
        { x: 0, y: 200 },
      ],
      0,
      40
    );
    expect(isOrthogonal(out)).toBe(true);
    expect(out[0]).toEqual({ x: 0, y: 0 });
    expect(out[out.length - 1]).toEqual({ x: 0, y: 200 });
    expect(out.some((p) => p.x === 40)).toBe(true);
  });
});

describe("pinEnds", () => {
  it("follows a moved end and stays orthogonal", () => {
    const route = [
      { x: 0, y: 0 },
      { x: 0, y: 100 },
      { x: 200, y: 100 },
      { x: 200, y: 200 },
    ];
    const out = pinEnds(route, { x: 20, y: 0 }, { x: 200, y: 240 });
    expect(out).toEqual([
      { x: 20, y: 0 },
      { x: 20, y: 100 },
      { x: 200, y: 100 },
      { x: 200, y: 240 },
    ]);
    expect(isOrthogonal(out)).toBe(true);
  });
});
