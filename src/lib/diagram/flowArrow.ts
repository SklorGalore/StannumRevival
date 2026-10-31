import type { Pt } from "./router";

/** Corner points of an SVG path made of straight runs (as React Flow's step
 *  path is with no corner radius). Repeated and in-line points are dropped. */
export function pathPoints(d: string): Pt[] {
  const nums = (d.match(/-?\d*\.?\d+(?:e[-+]?\d+)?/gi) ?? []).map(Number);
  const pts: Pt[] = [];
  for (let i = 0; i + 1 < nums.length; i += 2) {
    const p = { x: nums[i], y: nums[i + 1] };
    const last = pts[pts.length - 1];
    if (last && Math.abs(last.x - p.x) < 0.01 && Math.abs(last.y - p.y) < 0.01) continue;
    const prev = pts[pts.length - 2];
    if (prev && last && ((prev.x === last.x && last.x === p.x) || (prev.y === last.y && last.y === p.y))) pts.pop();
    pts.push(p);
  }
  return pts;
}

/** Unit step along a segment's main axis, so the arrow always lies along a
 *  wire run (never diagonal), flipped when the flow runs the other way. */
export function flowDirection(a: Pt, b: Pt, reverse: boolean): Pt {
  const sx = b.x - a.x;
  const sy = b.y - a.y;
  const flip = reverse ? -1 : 1;
  if (Math.abs(sx) >= Math.abs(sy)) return { x: (Math.sign(sx) || 1) * flip, y: 0 };
  return { x: 0, y: Math.sign(sy) * flip };
}

/** A filled triangle at `at` pointing along the unit step `dir`, scaled by `k`. */
export function arrowPath(at: Pt, dir: Pt, k = 1): string {
  const { x: dx, y: dy } = dir;
  const tip = { x: at.x + dx * 5 * k, y: at.y + dy * 5 * k };
  const left = { x: at.x - (dx + dy) * 4 * k, y: at.y + (dx - dy) * 4 * k };
  const right = { x: at.x + (dy - dx) * 4 * k, y: at.y - (dy + dx) * 4 * k };
  return `M${tip.x},${tip.y} L${left.x},${left.y} L${right.x},${right.y} Z`;
}

/** Length of a wire through its corners. */
export function polylineLength(pts: Pt[]): number {
  let len = 0;
  for (let i = 1; i < pts.length; i++) len += Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
  return len;
}

/**
 * Where the MW arrow and the Mvar arrow sit when both are drawn: either side
 * of the middle of the run, so they never overlap whichever way each points.
 */
export function reactiveArrowAt(mid: Pt, a: Pt, b: Pt): { p: Pt; q: Pt } {
  const u = flowDirection(a, b, false);
  return { p: { x: mid.x - u.x * 8, y: mid.y - u.y * 8 }, q: { x: mid.x + u.x * 8, y: mid.y + u.y * 8 } };
}

/**
 * Speed of moving flow arrows in px/s: slow for a lightly used branch, fast
 * near its rating. Without a rating the flow is measured against the largest
 * flow in the drawing.
 */
export function flowSpeed(loadingPct: number | null | undefined, p: number, maxFlow: number): number {
  const share = loadingPct != null ? loadingPct / 100 : maxFlow > 0 ? Math.abs(p) / maxFlow : 0.5;
  return 15 + 45 * Math.min(1, Math.max(0, share));
}
