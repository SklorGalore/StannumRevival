import { useEffect, type RefObject } from "react";
import { ConnectionMode, useStoreApi, type InternalNode } from "@xyflow/react";
import { useDiagramStore } from "../../store/diagramStore";
import type { SLDEdge } from "../../store/types";
import { BoxIndex, ROUTE_TOLERANCE, isOrthogonal, pinEnds, routeWire, type Box, type Pt, type Terminal } from "../../lib/diagram/router";

type Handle = NonNullable<NonNullable<InternalNode["internals"]["handleBounds"]>["source"]>[number];

/** Where a handle meets its wire, as React Flow computes it. */
function handlePoint(node: InternalNode, h: Handle): Terminal {
  const x = node.internals.positionAbsolute.x + h.x;
  const y = node.internals.positionAbsolute.y + h.y;
  switch (h.position) {
    case "top":
      return { x: x + h.width / 2, y, side: "top" };
    case "right":
      return { x: x + h.width, y: y + h.height / 2, side: "right" };
    case "bottom":
      return { x: x + h.width / 2, y: y + h.height, side: "bottom" };
    default:
      return { x, y: y + h.height / 2, side: "left" };
  }
}

/** Both ends of a wire, or null until its symbols have been measured. */
function wireEnds(e: SLDEdge, lookup: Map<string, InternalNode>, loose: boolean): [Terminal, Terminal] | null {
  const s = lookup.get(e.source);
  const t = lookup.get(e.target);
  const sb = s?.internals.handleBounds;
  const tb = t?.internals.handleBounds;
  if (!s || !t || !sb || !tb) return null;
  const pick = (list: Handle[] | null | undefined, id: string | null | undefined) =>
    (list ?? []).find((h) => (id ? h.id === id : true)) ?? null;
  const sh = pick(sb.source, e.sourceHandle);
  const th = pick(loose ? [...(tb.target ?? []), ...(tb.source ?? [])] : tb.target, e.targetHandle);
  if (!sh || !th) return null;
  return [handlePoint(s, sh), handlePoint(t, th)];
}

const near = (p: Pt, q: Pt) => Math.abs(p.x - q.x) <= ROUTE_TOLERANCE && Math.abs(p.y - q.y) <= ROUTE_TOLERANCE;

type Area = Box;
/** Wires this short (end to end) are left unrouted. */
const STUB = 8;
/** How far the router may search past a wire's ends (its widest retry). */
const SEARCH_REACH = 600;
/** How far text may hang outside a symbol or wire. */
const LABEL_REACH = 200;

function bounds(pts: Pt[]): Box {
  const xs = pts.map((p) => p.x);
  const ys = pts.map((p) => p.y);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
}
const grow = (b: Box, by: number): Box => ({ x: b.x - by, y: b.y - by, w: b.w + 2 * by, h: b.h + 2 * by });
const overlaps = (a: Box, b: Box) => a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h;

/** A symbol's outline with its text, relative to its position. It only
 *  changes with the symbol's data, size or text, so moving a symbol needs
 *  no new measurement. */
const outlines = new WeakMap<object, { sig: string; box: Box }>();

/** Box each symbol takes up on screen, its text included, in flow coordinates. */
function measureBoxes(root: HTMLElement, lookup: Map<string, InternalNode>, zoom: number, area: Area): Box[] {
  const boxes: Box[] = [];
  for (const el of root.querySelectorAll<HTMLElement>(".react-flow__node")) {
    const node = lookup.get(el.dataset.id ?? "");
    if (!node) continue;
    const at = node.internals.positionAbsolute;
    const { width = 0, height = 0 } = node.measured ?? {};
    // Labels reach a little way past the node's own box.
    if (!overlaps(area, grow({ x: at.x, y: at.y, w: width, h: height }, LABEL_REACH))) continue;
    const sig = `${width}x${height}|${el.textContent}`;
    let outline = outlines.get(node.internals.userNode.data);
    if (outline?.sig !== sig) {
      outline = { sig, box: measureOutline(el, zoom) ?? { x: 0, y: 0, w: width, h: height } };
      outlines.set(node.internals.userNode.data, outline);
    }
    const b = outline.box;
    boxes.push({ x: at.x + b.x, y: at.y + b.y, w: b.w, h: b.h });
  }
  return boxes;
}

/** Outline of a node element and everything drawn in it, relative to the
 *  node, in flow units. Null when the element isn't laid out (module hidden). */
function measureOutline(el: HTMLElement, zoom: number): Box | null {
  const frame = el.getBoundingClientRect();
  if (frame.width === 0 && frame.height === 0) return null;
  let x0 = frame.left;
  let y0 = frame.top;
  let x1 = frame.right;
  let y1 = frame.bottom;
  // Labels hang outside the node's own box (SVG overflow is visible).
  for (const part of el.querySelectorAll("svg text, svg rect, svg circle, svg path, svg line, svg polygon")) {
    const r = part.getBoundingClientRect();
    if (!r.width && !r.height) continue;
    x0 = Math.min(x0, r.left);
    y0 = Math.min(y0, r.top);
    x1 = Math.max(x1, r.right);
    y1 = Math.max(y1, r.bottom);
  }
  return { x: (x0 - frame.left) / zoom, y: (y0 - frame.top) / zoom, w: (x1 - x0) / zoom, h: (y1 - y0) / zoom };
}

/** Flow results and names written beside other wires. Edge SVG is drawn in
 *  flow coordinates, so getBBox needs no conversion. */
function wireLabels(root: HTMLElement, routes: Map<string, Pt[]>, area: Area): Box[] {
  const boxes: Box[] = [];
  for (const edge of root.querySelectorAll<SVGGElement>(".react-flow__edge")) {
    const route = routes.get(edge.dataset.id ?? "");
    if (!route || !overlaps(area, grow(bounds(route), LABEL_REACH))) continue;
    for (const text of edge.querySelectorAll<SVGTextElement>("text.edge-label")) {
      const b = text.getBBox();
      if (b.width && b.height) boxes.push({ x: b.x, y: b.y, w: b.width, h: b.height });
    }
  }
  return boxes;
}

/** Keeps every wire's stored route in step with its symbols. Wires with no
 *  route, or whose ends have moved, get a new one once nothing is being
 *  dragged. A wire whose route was adjusted by hand keeps its shape and
 *  only has its ends moved. Routes are stored in `data.route`. */
export function useWireRouting(root: RefObject<HTMLElement | null>) {
  const flow = useStoreApi();
  const nodes = useDiagramStore((s) => s.nodes);
  const edges = useDiagramStore((s) => s.edges);

  useEffect(() => {
    if (nodes.some((n) => n.dragging)) return;
    // Wait a frame so moved symbols and new taps are measured.
    const frame = requestAnimationFrame(() => {
      const { nodeLookup, connectionMode, transform } = flow.getState();
      const loose = connectionMode === ConnectionMode.Loose;
      const { edges: current, nodes: all, setEdges } = useDiagramStore.getState();
      if (all.some((n) => n.dragging)) return;

      const todo: { i: number; ends: [Terminal, Terminal] }[] = [];
      const next = [...current];
      let changed = false;
      current.forEach((e, i) => {
        const ends = wireEnds(e, nodeLookup, loose);
        if (!ends) return;
        const route = e.data?.route;
        // A symbol sitting on its bus bar is joined by a wire of next to no
        // length: drawn straight, never routed.
        if (Math.abs(ends[0].x - ends[1].x) + Math.abs(ends[0].y - ends[1].y) <= STUB) {
          if (route) {
            const { route: _, labelAt: __, pinnedRoute: ___, ...rest } = e.data!;
            next[i] = { ...e, data: rest as SLDEdge["data"] };
            changed = true;
          }
          return;
        }
        if (route && route.length >= 2 && near(route[0], ends[0]) && near(route[route.length - 1], ends[1])) return;
        if (route && route.length >= 2 && e.data?.pinnedRoute) {
          const pinned = pinEnds(route, ends[0], ends[1]);
          if (isOrthogonal(pinned)) {
            next[i] = { ...e, data: { ...e.data!, route: pinned } };
            changed = true;
            return;
          }
        }
        todo.push({ i, ends });
      });
      if (!todo.length) {
        if (changed) setEdges(next, false);
        return;
      }

      const el = root.current;
      const redo = new Set(todo.map((t) => t.i));
      // Only what the routes can reach is measured: the router looks at most
      // SEARCH_REACH past a wire's ends.
      const area = grow(bounds(todo.flatMap((t) => t.ends)), SEARCH_REACH);
      const others = new Map(
        next.flatMap((e, i) => (!redo.has(i) && e.data?.route ? [[e.id, e.data.route] as const] : []))
      );
      const boxes = el
        ? [...measureBoxes(el, nodeLookup, transform[2] || 1, area), ...wireLabels(el, others, area)]
        : [];
      const index = new BoxIndex(boxes);
      const wires = next.filter((_, i) => !redo.has(i)).flatMap((e) => (e.data?.route ? [e.data.route] : []));
      // Short wires first: they have the fewest ways round.
      const span = ([a, b]: [Terminal, Terminal]) => Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
      todo.sort((p, q) => span(p.ends) - span(q.ends));
      for (const { i, ends } of todo) {
        const { points } = routeWire(ends[0], ends[1], index, wires);
        wires.push(points);
        const e = next[i];
        // The layout's reserved label spot belonged to the old route.
        const { labelAt: _, pinnedRoute: __, ...rest } = e.data ?? ({} as NonNullable<SLDEdge["data"]>);
        next[i] = { ...e, data: { ...rest, route: points } as SLDEdge["data"] };
      }
      // Routes follow from the drawing, so they don't mark it changed by themselves.
      setEdges(next, false);
    });
    return () => cancelAnimationFrame(frame);
  }, [nodes, edges, flow, root]);
}
