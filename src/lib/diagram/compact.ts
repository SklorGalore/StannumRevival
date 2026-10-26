// Rows for a local drawing. Buses are grouped by how many hops they are from
// the element being studied, and each hop is folded into as many rows as it
// takes for the whole drawing to come out close to the shape of the screen.
// With one studied bus, its neighbours are split between the rows above it
// and the rows below it, so it ends up in the middle; with a branch, the
// from-side grows upwards and the to-side downwards from the branch itself.

export interface HopGroup {
  /** Above the studied element, the element itself, or below it. */
  side: "up" | "centre" | "down";
  hop: number;
  /** In the order they were reached, so children stay near their parents. */
  buses: number[];
}

export interface Hops {
  groups: HopGroup[];
  hop: Map<number, number>;
}

/**
 * Group buses by hop count from `centres`, top to bottom. `links` are the
 * branches between them. With `twoSided` and a single centre, its
 * neighbourhood is shared between the two sides by `width`; with several
 * centres, everything reached first from the first one goes above.
 */
export function hopGroups(
  buses: number[],
  links: { a: number; b: number }[],
  centres: number[],
  width: (bus: number) => number,
  twoSided: boolean,
): Hops {
  const adj = new Map<number, number[]>(buses.map((b) => [b, []]));
  for (const { a, b } of links) {
    if (a === b) continue;
    adj.get(a)?.push(b);
    adj.get(b)?.push(a);
  }
  const hop = new Map<number, number>();
  const origin = new Map<number, number>();
  /** The neighbour of a centre each bus was reached through. */
  const via = new Map<number, number>();
  const order = new Map<number, number>();
  const seeds = centres.filter((c) => adj.has(c));
  if (!seeds.length && buses.length) seeds.push(buses[0]);
  const queue: number[] = [];
  seeds.forEach((c, i) => {
    if (hop.has(c)) return;
    hop.set(c, 0);
    origin.set(c, i);
    order.set(c, order.size);
    queue.push(c);
  });
  const walk = () => {
    while (queue.length) {
      const id = queue.shift()!;
      for (const n of adj.get(id)!) {
        if (hop.has(n)) continue;
        hop.set(n, hop.get(id)! + 1);
        origin.set(n, origin.get(id)!);
        via.set(n, hop.get(id) === 0 ? n : via.get(id)!);
        order.set(n, order.size);
        queue.push(n);
      }
    }
  };
  walk();
  // Parts not joined to the centres (rare in a local drawing) go below the rest.
  for (const b of buses) {
    if (hop.has(b)) continue;
    const deepest = Math.max(0, ...hop.values());
    hop.set(b, deepest + 1);
    origin.set(b, seeds.length > 1 ? 1 : 0);
    via.set(b, b);
    order.set(b, order.size);
    queue.push(b);
    walk();
  }

  const side = new Map<number, "up" | "down">();
  if (seeds.length > 1) {
    for (const b of buses) side.set(b, origin.get(b) === 0 ? "up" : "down");
  } else {
    // Share the centre's neighbourhoods between the sides, heaviest first,
    // each to the side carrying less so far. One neighbourhood goes below.
    const weight = new Map<number, number>();
    for (const b of buses) {
      const v = via.get(b);
      if (v != null) weight.set(v, (weight.get(v) ?? 0) + width(b));
    }
    const sideOf = new Map<number, "up" | "down">();
    let up = 0;
    let down = 0;
    const heaviest = [...weight].sort((a, b) => b[1] - a[1] || order.get(a[0])! - order.get(b[0])!);
    for (const [v, w] of heaviest) {
      if (twoSided && up < down) {
        sideOf.set(v, "up");
        up += w;
      } else {
        sideOf.set(v, "down");
        down += w;
      }
    }
    for (const b of buses) side.set(b, sideOf.get(via.get(b)!) ?? "down");
  }

  const byOrder = (a: number, b: number) => order.get(a)! - order.get(b)!;
  const deepest = Math.max(0, ...hop.values());
  const groups: HopGroup[] = [];
  const pick = (s: "up" | "down", h: number) =>
    buses.filter((b) => hop.get(b) === h && side.get(b) === s).sort(byOrder);
  for (let h = deepest; h >= 1; h--) {
    const list = pick("up", h);
    if (list.length) groups.push({ side: "up", hop: h, buses: list });
  }
  // The first centre on its own row, any others (the far end of a branch,
  // the other windings of a transformer) on the row below it.
  const first = seeds.slice(0, 1).filter((c) => hop.get(c) === 0);
  const rest = seeds.slice(1).filter((c) => hop.get(c) === 0);
  if (first.length) groups.push({ side: "centre", hop: 0, buses: first });
  if (rest.length) groups.push({ side: "centre", hop: 0, buses: rest });
  for (let h = 1; h <= deepest; h++) {
    const list = pick("down", h);
    if (list.length) groups.push({ side: "down", hop: h, buses: list });
  }
  return { groups, hop };
}

/**
 * Fold each group into as many rows as it takes to stay near `limit` wide,
 * sharing a group's buses evenly between its rows in the order given.
 * Above the centre the rows are stacked upwards, so the first ones reached
 * stay nearest the centre. Returns the rows, top to bottom.
 */
export function foldGroups(groups: HopGroup[], width: (bus: number) => number, limit: number): number[][] {
  const out: number[][] = [];
  for (const g of groups) {
    const total = g.buses.reduce((s, b) => s + width(b), 0);
    const count = Math.max(1, Math.ceil(total / Math.max(limit, 1) - 1e-9));
    const target = total / count;
    // Each bus goes to the row its middle falls in, measured along the
    // group laid end to end and cut into equal shares.
    const rows: number[][] = Array.from({ length: count }, () => []);
    let before = 0;
    for (const b of g.buses) {
      const w = width(b);
      rows[Math.min(count - 1, Math.floor((before + w / 2) / target))].push(b);
      before += w;
    }
    if (g.side === "up") rows.reverse();
    out.push(...rows);
  }
  return out;
}

export interface Fit {
  limit: number;
  rows: number[][];
  /** Estimated drawing size. */
  width: number;
  height: number;
}

/**
 * The row width that lets the drawing be shown largest in a view of the
 * given `aspect` (width / height), with each row taking `rowHeight`.
 * `scale` corrects estimated widths, from a layout already measured.
 */
export function bestFold(
  groups: HopGroup[],
  width: (bus: number) => number,
  aspect: number,
  rowHeight: number,
  scale = 1,
): Fit {
  const widest = Math.max(1, ...groups.flatMap((g) => g.buses.map(width)));
  const limits = new Set<number>([widest]);
  for (const g of groups) {
    const total = g.buses.reduce((s, b) => s + width(b), 0);
    for (let k = 1; k <= g.buses.length; k++) limits.add(Math.max(widest, total / k));
  }
  let best: Fit | null = null;
  let bestZoom = -1;
  for (const limit of [...limits].sort((a, b) => b - a)) {
    const rows = foldGroups(groups, width, limit);
    const w = scale * Math.max(...rows.map((r) => r.reduce((s, b) => s + width(b), 0)));
    const h = rows.length * rowHeight;
    const zoom = Math.min(aspect / w, 1 / h);
    // Fewer rows win ties, so a drawing that already fits isn't folded.
    if (zoom > bestZoom * 1.02) {
      best = { limit, rows, width: w, height: h };
      bestZoom = zoom;
    }
  }
  return best!;
}
