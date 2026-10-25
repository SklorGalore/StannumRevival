import type { BusData, BusTap, Rotation, SLDEdge, SLDNode } from "../../store/types";
import { busTaps } from "../../components/nodes/shared/busTaps";
import { BAR, along, isVertical, placeTerminal, sideToward, tapPoint, terminalOf } from "./bus";
import { symbolSize } from "./sizes";

/** Size a node takes on the canvas now. */
function boxOf(n: SLDNode): { w: number; h: number } {
  if (n.type === "bus") {
    const d = n.data as BusData;
    const len = d.length ?? 200;
    if (d.star) return { w: len, h: BAR };
    return d.orientation === "vertical" ? { w: BAR, h: len } : { w: len, h: BAR };
  }
  return symbolSize(n, true);
}

/** `v` on the grid. A tie, which a turn often makes, goes to the side
 *  nearer `was`, so turning back and forth never walks the symbol away. */
function snap(v: number, grid: number, was: number) {
  if (grid <= 0) return v;
  const lo = Math.floor(v / grid) * grid;
  const hi = lo + grid;
  if (Math.abs(v - lo - grid / 2) < 1e-6) return Math.abs(was - lo) <= Math.abs(was - hi) ? lo : hi;
  return v - lo < hi - v ? lo : hi;
}

/**
 * The node turned by a quarter, clockwise when `turn` is 90, about its
 * centre. A bus swaps between horizontal and vertical; a star point stays as
 * it is. Text on symbols stays upright, so only the symbol turns.
 */
export function rotateNode(n: SLDNode, turn: 90 | -90, grid = 0): SLDNode {
  const { w, h } = boxOf(n);
  const centre = { x: n.position.x + w / 2, y: n.position.y + h / 2 };
  // The turned box has its sides swapped.
  const position = {
    x: snap(centre.x - h / 2, grid, n.position.x),
    y: snap(centre.y - w / 2, grid, n.position.y),
  };
  if (n.type === "bus") {
    const d = n.data as BusData;
    if (d.star) return n;
    const orientation = d.orientation === "vertical" ? "horizontal" : "vertical";
    // Labels go back to their default spot for the new direction.
    return { ...n, position, data: { ...d, orientation, labelOffset: undefined } } as SLDNode;
  }
  const from = ((n.data as { rotation?: Rotation }).rotation ?? 0) as number;
  const rotation = ((from + turn + 360) % 360) as Rotation;
  return { ...n, position, data: { ...n.data, rotation, labelOffset: undefined } } as SLDNode;
}

/** Terminal of each symbol that can sit straight on a bus bar. */
const ON_BUS: Partial<Record<string, string>> = { generator: "gen-top", load: "load-top", shunt: "shunt-top" };

/**
 * `nodes` with the selected ones turned a quarter. Generators, loads and
 * shunts sitting on a turned bus go round with it, so they stay on the bar.
 */
export function rotateSelected(nodes: SLDNode[], edges: SLDEdge[], turn: 90 | -90, grid = 0): SLDNode[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const out = new Map<string, SLDNode>();
  for (const n of nodes) if (n.selected) out.set(n.id, rotateNode(n, turn, grid));
  for (const n of nodes) {
    if (!n.selected || n.type !== "bus" || (n.data as BusData).star) continue;
    const bus = out.get(n.id)!;
    // Horizontal to vertical is a clockwise turn, so the symbols turn the same way.
    const spin = isVertical(bus.data as BusData) ? 90 : 270;
    const taps = busTaps(n.data as BusData);
    const carried = new Set<string>();
    for (const e of edges) {
      const [tapId, symId, handle] =
        e.source === n.id ? [e.sourceHandle, e.target, e.targetHandle] : e.target === n.id ? [e.targetHandle, e.source, e.sourceHandle] : [];
      const sym = symId ? byId.get(symId) : undefined;
      const tap = taps.find((t) => t.id === tapId);
      if (!sym || !tap || ON_BUS[sym.type ?? ""] !== handle) continue;
      const at = tapPoint(n, tap.x, tap.side);
      const { p } = terminalOf(sym);
      if (Math.abs(p.x - at.x) > 2 || Math.abs(p.y - at.y) > 2) continue;
      const from = ((sym.data as { rotation?: Rotation }).rotation ?? 0) as number;
      const turned = { ...sym, data: { ...sym.data, rotation: ((from + spin) % 360) as Rotation, labelOffset: undefined } } as SLDNode;
      const { w, h } = symbolSize(turned, true);
      const position = placeTerminal(tapPoint(bus, tap.x, tap.side), terminalOf(turned).side, w, h);
      out.set(sym.id, { ...turned, position } as SLDNode);
      carried.add(tap.id);
    }
    // Other wires leave from the side of the bar facing where they go, as
    // near level with it as a free spot allows, so they don't have to wrap
    // round the bar. Taps shared by several wires stay where they are.
    const uses = new Map<string, string[]>();
    for (const e of edges) {
      const [tapId, other] = e.source === n.id ? [e.sourceHandle, e.target] : e.target === n.id ? [e.targetHandle, e.source] : [];
      if (tapId && other) uses.set(tapId, [...(uses.get(tapId) ?? []), other]);
    }
    const len = (bus.data as BusData).length ?? 200;
    const step = grid > 0 ? grid : 20;
    const moving = taps.filter((t) => !carried.has(t.id) && uses.get(t.id)?.length === 1);
    const placed = taps.filter((t) => !moving.includes(t));
    let changed = false;
    const faced = new Map<string, BusTap>();
    for (const t of moving) {
      const far = out.get(uses.get(t.id)![0]) ?? byId.get(uses.get(t.id)![0]);
      if (!far) continue;
      const { w, h } = boxOf(far);
      const centre = { x: far.position.x + w / 2, y: far.position.y + h / 2 };
      const side = sideToward(bus, centre);
      const want = Math.min(len - step, Math.max(step, Math.round(along(bus, centre) / step) * step));
      // The nearest spot at least a step from the other taps, on either side,
      // so a wire never looks like it carries straight on across the bar.
      let x = want;
      for (let k = 1; placed.some((p) => Math.abs(p.x - x) < step) && k < 40; k++)
        x = Math.min(len, Math.max(0, want + (k % 2 ? 1 : -1) * Math.ceil(k / 2) * step));
      const next = { ...t, side, x };
      placed.push(next);
      faced.set(t.id, next);
      changed ||= next.side !== t.side || next.x !== t.x;
    }
    if (changed)
      out.set(n.id, { ...bus, data: { ...bus.data, taps: taps.map((t) => faced.get(t.id) ?? t) } } as SLDNode);
  }
  return nodes.map((n) => out.get(n.id) ?? n);
}
