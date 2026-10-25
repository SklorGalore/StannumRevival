// Orthogonal wire router. Finds a right-angle path between two terminals on
// the drawing grid that keeps clear of symbol and label boxes, with a cost
// for each bend and for crossing or running along other wires.
//
// Pure geometry: no React, no DOM. The canvas measures the boxes and calls
// `routeWire` for each wire that needs a new path (see useWireRouting.ts).

export type Pt = { x: number; y: number };

/** How far a wire's end may drift from its stored route before the route
 *  stops being drawn and the wire is rerouted. */
export const ROUTE_TOLERANCE = 6;
export type Side = "top" | "bottom" | "left" | "right";

/** A terminal: where the wire ends, and the side of its symbol it leaves from. */
export interface Terminal extends Pt {
  side: Side;
}

/** An obstacle in flow coordinates, unpadded. */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface RouteOptions {
  /** Grid pitch the route runs on. */
  grid?: number;
  /** Clearance kept around every box. */
  pad?: number;
  /** Extra length a bend is worth avoiding. */
  bendCost?: number;
  /** Cost of crossing another wire. */
  crossCost?: number;
  /** Cost per step of running on top of another wire. */
  overlapCost?: number;
  /** Cost per grid point inside a box's clearance. */
  padCost?: number;
  /** Cost per step through a box itself. Boxes are soft so a route always exists. */
  blockedCost?: number;
  /** How far beyond the two ends the search may go, in px. */
  margin?: number;
}

const DEFAULTS: Required<RouteOptions> = {
  grid: 20,
  pad: 10,
  bendCost: 40,
  crossCost: 30,
  overlapCost: 120,
  padCost: 300,
  blockedCost: 20000,
  margin: 120,
};

const DIRS: Record<Side, Pt> = {
  top: { x: 0, y: -1 },
  bottom: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
  right: { x: 1, y: 0 },
};
/** Step directions by index: right, down, left, up. Opposites differ by 2. */
const STEP = [
  { x: 1, y: 0 },
  { x: 0, y: 1 },
  { x: -1, y: 0 },
  { x: 0, y: -1 },
];
const dirIndex = (d: Pt) => (d.x > 0 ? 0 : d.y > 0 ? 1 : d.x < 0 ? 2 : 3);

// ── Spatial index of boxes ───────────────────────────────────────────────────

/** Buckets boxes by a coarse grid so a route only looks at boxes near it. */
export class BoxIndex {
  private cells = new Map<number, Box[]>();
  readonly size: number;
  constructor(boxes: Box[] = [], size = 200) {
    this.size = size;
    for (const b of boxes) this.add(b);
  }
  private key(i: number, j: number) {
    return (i + 32768) * 65536 + (j + 32768);
  }
  add(b: Box) {
    const s = this.size;
    for (let i = Math.floor(b.x / s); i <= Math.floor((b.x + b.w) / s); i++)
      for (let j = Math.floor(b.y / s); j <= Math.floor((b.y + b.h) / s); j++) {
        const k = this.key(i, j);
        const list = this.cells.get(k);
        if (list) list.push(b);
        else this.cells.set(k, [b]);
      }
  }
  /** Boxes overlapping a rectangle, each once. */
  query(x0: number, y0: number, x1: number, y1: number): Box[] {
    const s = this.size;
    const out = new Set<Box>();
    for (let i = Math.floor(x0 / s); i <= Math.floor(x1 / s); i++)
      for (let j = Math.floor(y0 / s); j <= Math.floor(y1 / s); j++)
        for (const b of this.cells.get(this.key(i, j)) ?? [])
          if (b.x <= x1 && b.x + b.w >= x0 && b.y <= y1 && b.y + b.h >= y0) out.add(b);
    return [...out];
  }
}

// ── Small helpers ────────────────────────────────────────────────────────────

/** Drop repeated points and the middle of straight runs. */
export function simplify(pts: Pt[]): Pt[] {
  const out: Pt[] = [];
  for (const p of pts) {
    const last = out[out.length - 1];
    if (last && Math.abs(last.x - p.x) < 0.01 && Math.abs(last.y - p.y) < 0.01) continue;
    if (out.length >= 2) {
      const a = out[out.length - 2];
      const b = last;
      const straight =
        (Math.abs(a.x - b.x) < 0.01 && Math.abs(b.x - p.x) < 0.01) ||
        (Math.abs(a.y - b.y) < 0.01 && Math.abs(b.y - p.y) < 0.01);
      if (straight) out.pop();
    }
    out.push({ x: p.x, y: p.y });
  }
  return out;
}

/** True when every segment is horizontal or vertical. */
export function isOrthogonal(pts: Pt[]): boolean {
  for (let i = 0; i < pts.length - 1; i++)
    if (Math.abs(pts[i].x - pts[i + 1].x) > 0.5 && Math.abs(pts[i].y - pts[i + 1].y) > 0.5) return false;
  return true;
}

export function bends(pts: Pt[]): number {
  return Math.max(0, simplify(pts).length - 2);
}

/** Move a route's ends to new terminal positions, keeping the first and last
 *  segments straight by sliding their inner points along. */
export function pinEnds(route: Pt[], s: Pt, t: Pt): Pt[] {
  const pts = route.map((p) => ({ ...p }));
  const pin = (i: number, j: number, to: Pt) => {
    const vertical = Math.abs(pts[i].x - pts[j].x) < 0.5;
    pts[i] = { ...to };
    if (pts.length > 2) {
      if (vertical) pts[j].x = to.x;
      else pts[j].y = to.y;
    }
  };
  pin(0, 1, s);
  pin(pts.length - 1, pts.length - 2, t);
  return pts;
}

/** Slide segment `k` (points k and k+1) sideways by `delta`. A segment that
 *  ends on a terminal keeps a short stub there so the end stays attached. */
export function moveSegment(route: Pt[], k: number, delta: number, stub = 10): Pt[] {
  const n = route.length;
  const pts = route.map((p) => ({ ...p }));
  if (k < 0 || k >= n - 1 || !delta) return pts;
  const vertical = Math.abs(pts[k].x - pts[k + 1].x) < 0.5;
  const shift = (p: Pt) => (vertical ? { x: p.x + delta, y: p.y } : { x: p.x, y: p.y + delta });
  // A point `stub` px from `from` toward `to` (at most halfway).
  const toward = (from: Pt, to: Pt) => {
    const len = Math.abs(to.x - from.x) + Math.abs(to.y - from.y);
    const f = len > 0 ? Math.min(stub, len / 2) / len : 0;
    return { x: from.x + (to.x - from.x) * f, y: from.y + (to.y - from.y) * f };
  };
  const out: Pt[] = pts.slice(0, k);
  if (k === 0) {
    const s = toward(pts[0], pts[1]);
    out.push(pts[0], s, shift(s));
  } else out.push(shift(pts[k]));
  if (k + 1 === n - 1) {
    const s = toward(pts[n - 1], pts[n - 2]);
    out.push(shift(s), s, pts[n - 1]);
  } else out.push(shift(pts[k + 1]), ...pts.slice(k + 2));
  return simplify(out);
}

// ── Binary heap keyed by f-score ─────────────────────────────────────────────

class Heap {
  private f: number[] = [];
  private v: number[] = [];
  get size() {
    return this.v.length;
  }
  peek() {
    return this.f[0];
  }
  push(f: number, v: number) {
    const fs = this.f;
    const vs = this.v;
    let i = vs.length;
    fs.push(f);
    vs.push(v);
    while (i > 0) {
      const p = (i - 1) >> 1;
      if (fs[p] <= f) break;
      fs[i] = fs[p];
      vs[i] = vs[p];
      i = p;
    }
    fs[i] = f;
    vs[i] = v;
  }
  pop(): number {
    const fs = this.f;
    const vs = this.v;
    const top = vs[0];
    const lf = fs.pop()!;
    const lv = vs.pop()!;
    const n = vs.length;
    if (n) {
      let i = 0;
      for (;;) {
        let c = 2 * i + 1;
        if (c >= n) break;
        if (c + 1 < n && fs[c + 1] < fs[c]) c++;
        if (fs[c] >= lf) break;
        fs[i] = fs[c];
        vs[i] = vs[c];
        i = c;
      }
      fs[i] = lf;
      vs[i] = lv;
    }
    return top;
  }
}

// ── The router ───────────────────────────────────────────────────────────────

function lowerBound(a: number[], x: number) {
  let lo = 0;
  let hi = a.length;
  while (lo < hi) {
    const m = (lo + hi) >> 1;
    if (a[m] < x) lo = m + 1;
    else hi = m;
  }
  return lo;
}

/** Index of the coordinate closest to `v`, if within `reach`; else -1.
 *  Wires off the grid (from the layout) count as on the nearest line. */
function nearest(a: number[], v: number, reach: number) {
  const k = lowerBound(a, v);
  let best = -1;
  for (const c of [k - 1, k]) {
    if (c < 0 || c >= a.length) continue;
    const dist = Math.abs(a[c] - v);
    if (dist < reach && (best < 0 || dist < Math.abs(a[best] - v))) best = c;
  }
  return best;
}

/** Sorted unique coordinates: grid lines in [lo, hi] plus the extras. */
function axis(lo: number, hi: number, grid: number, extra: number[]): number[] {
  const out = new Set<number>();
  for (let v = Math.floor(lo / grid) * grid; v <= hi; v += grid) out.add(v);
  for (const e of extra) out.add(Math.round(e * 100) / 100);
  return [...out].sort((a, b) => a - b);
}

/** True when a point is strictly inside a box grown by `pad`. */
const inside = (p: Pt, b: Box, pad: number) =>
  p.x > b.x - pad && p.x < b.x + b.w + pad && p.y > b.y - pad && p.y < b.y + b.h + pad;

/** Where the wire first reaches a grid line after leaving its own symbol.
 *  It clears the symbol's padding when there is room, else just the symbol
 *  itself, so a symbol dropped close to a bus doesn't push the wire through
 *  the bar. */
function escapePoint(t: Terminal, index: BoxIndex, o: Required<RouteOptions>): Pt {
  const d = DIRS[t.side];
  const g = o.grid;
  const around = index.query(t.x - o.pad - 1, t.y - o.pad - 1, t.x + o.pad + 1, t.y + o.pad + 1);
  // The symbol the terminal sits on (it's on that box's edge).
  const owners = around.filter((b) => inside(t, b, 0.5));
  const at = (clear: number, snap: boolean): Pt => {
    let reach = Math.max(1, clear);
    for (const b of owners) {
      const out =
        d.x > 0 ? b.x + b.w + clear - t.x : d.x < 0 ? t.x - b.x + clear : d.y > 0 ? b.y + b.h + clear - t.y : t.y - b.y + clear;
      reach = Math.max(reach, out);
    }
    const out = (v: number, dir: number) =>
      !snap ? v + dir * reach : dir > 0 ? Math.ceil((v + reach) / g) * g : Math.floor((v - reach) / g) * g;
    return d.x ? { x: out(t.x, d.x), y: t.y } : { x: t.x, y: out(t.y, d.y) };
  };
  const free = (p: Pt) =>
    !index
      .query(p.x - o.pad, p.y - o.pad, p.x + o.pad, p.y + o.pad)
      .some((b) => !owners.includes(b) && inside(p, b, o.pad));
  // On the grid with full clearance if there's room; else off the grid,
  // as close to the symbol as it takes to stay out of its neighbours.
  const tries = [at(o.pad, true), at(0, true), at(o.pad, false), at(o.pad / 2, false), at(2, false)];
  return tries.find(free) ?? tries[tries.length - 1];
}

export interface RouteResult {
  points: Pt[];
  cost: number;
  /** True when no clear path was found and the route cuts through a box. */
  blocked: boolean;
}

/** Shortest right-angle path from `a` to `b` around `obstacles`, avoiding
 *  `wires` (other routes) where it can. Endpoints are included. */
export function routeWire(
  a: Terminal,
  b: Terminal,
  obstacles: BoxIndex | Box[],
  wires: Pt[][] = [],
  options: RouteOptions = {}
): RouteResult {
  const o = { ...DEFAULTS, ...options };
  const index = obstacles instanceof BoxIndex ? obstacles : new BoxIndex(obstacles);
  const ea = escapePoint(a, index, o);
  const eb = escapePoint(b, index, o);
  let best = search(a, b, ea, eb, index, wires, o, o.margin);
  // Boxed in: look further afield before settling for a path through a box.
  if (best.blocked) {
    const wider = search(a, b, ea, eb, index, wires, o, o.margin * 4);
    if (wider.cost < best.cost) best = wider;
  }
  return best;
}

function search(
  a: Terminal,
  b: Terminal,
  ea: Pt,
  eb: Pt,
  index: BoxIndex,
  wires: Pt[][],
  o: Required<RouteOptions>,
  margin: number
): RouteResult {
  const x0 = Math.min(ea.x, eb.x) - margin;
  const x1 = Math.max(ea.x, eb.x) + margin;
  const y0 = Math.min(ea.y, eb.y) - margin;
  const y1 = Math.max(ea.y, eb.y) + margin;
  const xs = axis(x0, x1, o.grid, [ea.x, eb.x]);
  const ys = axis(y0, y1, o.grid, [ea.y, eb.y]);
  const nx = xs.length;
  const ny = ys.length;
  const cells = nx * ny;

  // Grid points strictly inside a box's clearance, and steps that cut
  // through a box itself (a bus bar can be thinner than a grid step).
  const blocked = new Uint8Array(cells);
  const cutH = new Uint8Array(cells);
  const cutV = new Uint8Array(cells);
  for (const bx of index.query(x0, y0, x1, y1)) {
    const bx1 = bx.x + bx.w;
    const by1 = bx.y + bx.h;
    for (let j = lowerBound(ys, bx.y); j < ny && ys[j] < by1; j++) {
      if (ys[j] <= bx.y) continue;
      for (let i = Math.max(0, lowerBound(xs, bx.x) - 1); i < nx - 1 && xs[i] < bx1; i++)
        if (xs[i + 1] > bx.x) cutH[j * nx + i] = 1;
    }
    for (let i = lowerBound(xs, bx.x); i < nx && xs[i] < bx1; i++) {
      if (xs[i] <= bx.x) continue;
      for (let j = Math.max(0, lowerBound(ys, bx.y) - 1); j < ny - 1 && ys[j] < by1; j++)
        if (ys[j + 1] > bx.y) cutV[j * nx + i] = 1;
    }
    const lx = bx.x - o.pad;
    const hx = bx.x + bx.w + o.pad;
    const ly = bx.y - o.pad;
    const hy = bx.y + bx.h + o.pad;
    const i0 = lowerBound(xs, lx);
    const j0 = lowerBound(ys, ly);
    for (let j = j0; j < ny && ys[j] < hy; j++) {
      if (ys[j] <= ly) continue;
      for (let i = i0; i < nx && xs[i] < hx; i++) if (xs[i] > lx) blocked[j * nx + i] = 1;
    }
  }

  // Other wires: 1 where a horizontal run passes a point, 2 for vertical.
  const used = new Uint8Array(cells);
  const xAt = new Map(xs.map((v, i) => [v, i]));
  const yAt = new Map(ys.map((v, j) => [v, j]));
  const key = (v: number) => Math.round(v * 100) / 100;
  for (const w of wires)
    for (let k = 0; k < w.length - 1; k++) {
      const p = w[k];
      const q = w[k + 1];
      if (Math.abs(p.y - q.y) < 0.5) {
        const j = nearest(ys, p.y, o.pad);
        if (j < 0) continue;
        const lo = Math.min(p.x, q.x);
        const hi = Math.max(p.x, q.x);
        for (let i = lowerBound(xs, lo); i < nx && xs[i] <= hi; i++) used[j * nx + i] |= 1;
      } else if (Math.abs(p.x - q.x) < 0.5) {
        const i = nearest(xs, p.x, o.pad);
        if (i < 0) continue;
        const lo = Math.min(p.y, q.y);
        const hi = Math.max(p.y, q.y);
        for (let j = lowerBound(ys, lo); j < ny && ys[j] <= hi; j++) used[j * nx + i] |= 2;
      }
    }

  const si = xAt.get(key(ea.x))!;
  const sj = yAt.get(key(ea.y))!;
  const ti = xAt.get(key(eb.x))!;
  const tj = yAt.get(key(eb.y))!;
  const startDir = dirIndex(DIRS[a.side]);
  // Arriving at the target's escape point, the wire heads into the terminal.
  const endDir = (dirIndex(DIRS[b.side]) + 2) % 4;

  const g = new Float64Array(cells * 4).fill(Infinity);
  const parent = new Int32Array(cells * 4).fill(-1);
  const heap = new Heap();
  // Distance left, plus a bend if the target is off both axes. The small
  // weight breaks ties toward the target so open areas aren't flooded.
  const h = (i: number, j: number) => {
    const dx = Math.abs(xs[i] - eb.x);
    const dy = Math.abs(ys[j] - eb.y);
    return (dx + dy) * 1.001 + (dx > 0.01 && dy > 0.01 ? o.bendCost : 0);
  };
  const start = (sj * nx + si) * 4 + startDir;
  g[start] = blocked[sj * nx + si] ? o.padCost : 0;
  heap.push(g[start] + h(si, sj), start);

  let bestCost = Infinity;
  let bestState = -1;
  while (heap.size) {
    if (heap.peek() >= bestCost) break;
    const s = heap.pop();
    const d = s & 3;
    const c = s >> 2;
    const i = c % nx;
    const j = (c - i) / nx;
    const gs = g[s];
    if (i === ti && j === tj) {
      const total = gs + (d === endDir ? 0 : d === (endDir + 2) % 4 ? 2 * o.bendCost : o.bendCost);
      if (total < bestCost) {
        bestCost = total;
        bestState = s;
      }
      continue;
    }
    for (let nd = 0; nd < 4; nd++) {
      if (nd === (d + 2) % 4) continue;
      const ni = i + STEP[nd].x;
      const nj = j + STEP[nd].y;
      if (ni < 0 || nj < 0 || ni >= nx || nj >= ny) continue;
      const nc = nj * nx + ni;
      const horizontal = nd % 2 === 0;
      let cost = horizontal ? Math.abs(xs[ni] - xs[i]) : Math.abs(ys[nj] - ys[j]);
      if (nd !== d) cost += o.bendCost;
      if (blocked[nc]) cost += o.padCost;
      if (horizontal ? cutH[j * nx + Math.min(i, ni)] : cutV[Math.min(j, nj) * nx + i]) cost += o.blockedCost;
      const mine = horizontal ? 1 : 2;
      const theirs = horizontal ? 2 : 1;
      if (used[nc] & mine && used[c] & mine) cost += o.overlapCost;
      else if (used[nc] & theirs) cost += o.crossCost;
      const ns = nc * 4 + nd;
      const ng = gs + cost;
      if (ng < g[ns]) {
        g[ns] = ng;
        parent[ns] = s;
        heap.push(ng + h(ni, nj), ns);
      }
    }
  }

  const cellsPath: Pt[] = [];
  let through = false;
  for (let s = bestState; s >= 0; s = parent[s]) {
    const c = s >> 2;
    const i = c % nx;
    const j = (c - i) / nx;
    const p = parent[s];
    if (p >= 0) {
      const pc = p >> 2;
      const pi = pc % nx;
      const pj = (pc - pi) / nx;
      if (pj === j ? cutH[j * nx + Math.min(i, pi)] : cutV[Math.min(j, pj) * nx + i]) through = true;
    }
    cellsPath.push({ x: xs[i], y: ys[j] });
  }
  cellsPath.reverse();
  if (!cellsPath.length) cellsPath.push({ x: ea.x, y: eb.y });
  const points = simplify([{ x: a.x, y: a.y }, ea, ...cellsPath, eb, { x: b.x, y: b.y }]);
  return { points, cost: bestCost, blocked: through };
}
