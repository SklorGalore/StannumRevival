import { BaseEdge, getSmoothStepPath, type EdgeProps } from "@xyflow/react";
import type { ConductorData } from "../../store/types";
import { branchFlow, loadingColor, mvar, mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED } from "../../lib/diagram/ink";
import { useDiagramStore } from "../../store/diagramStore";
import type { SLDEdge } from "../../store/types";
import { ROUTE_TOLERANCE, pinEnds, type Pt } from "../../lib/diagram/router";
import { SegmentGrips } from "./SegmentGrips";

/** Wires meeting at each terminal, keyed "node/handle". */
const degrees = new WeakMap<SLDEdge[], Map<string, number>>();
function degreeMap(edges: SLDEdge[]) {
  let m = degrees.get(edges);
  if (!m) {
    m = new Map();
    for (const e of edges)
      for (const k of [`${e.source}/${e.sourceHandle}`, `${e.target}/${e.targetHandle}`]) m.set(k, (m.get(k) ?? 0) + 1);
    degrees.set(edges, m);
  }
  return m;
}

/** True where three or more conductors meet at a symbol's terminal, which
 *  the drawing shows with a junction dot. Bus bars are junctions already. */
function useJunction(node: string, handle: string | null | undefined) {
  return useDiagramStore((s) =>
    !!handle && !handle.startsWith("bus-") && (degreeMap(s.edges).get(`${node}/${handle}`) ?? 0) >= 2
  );
}

/** The stored route, with its ends pinned to where the handles are now.
 *  Returns null once a symbol has moved, so the wire falls back to a simple
 *  right-angle path until it is rerouted (on release, by useWireRouting). */
function liveRoute(route: Pt[] | undefined, s: Pt, t: Pt): Pt[] | null {
  if (!route || route.length < 2) return null;
  const a = route[0];
  const b = route[route.length - 1];
  const near = (p: Pt, q: Pt) => Math.abs(p.x - q.x) <= ROUTE_TOLERANCE && Math.abs(p.y - q.y) <= ROUTE_TOLERANCE;
  if (!near(a, s) || !near(b, t)) return null;
  return pinEnds(route, s, t);
}

/** The segment passing closest to a point, and the point's projection on it. */
function nearestOnRoute(pts: Pt[], c: Pt) {
  let best = { a: pts[0], b: pts[1], at: pts[0], d: Infinity };
  for (let i = 0; i < pts.length - 1; i++) {
    const [a, b] = [pts[i], pts[i + 1]];
    const at = {
      x: Math.min(Math.max(c.x, Math.min(a.x, b.x)), Math.max(a.x, b.x)),
      y: Math.min(Math.max(c.y, Math.min(a.y, b.y)), Math.max(a.y, b.y)),
    };
    const d = Math.hypot(at.x - c.x, at.y - c.y);
    if (d < best.d) best = { a, b, at, d };
  }
  return best;
}

/** Middle of the longest segment: where flow labels and the arrow go. */
function longestSegment(pts: Pt[]) {
  let best = { a: pts[0], b: pts[1], len: -1 };
  for (let i = 0; i < pts.length - 1; i++) {
    const len = Math.abs(pts[i + 1].x - pts[i].x) + Math.abs(pts[i + 1].y - pts[i].y);
    if (len > best.len) best = { a: pts[i], b: pts[i + 1], len };
  }
  return best;
}

export function ConductorEdge({
  id,
  source,
  target,
  sourceHandleId,
  targetHandleId,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  selected,
}: EdgeProps & { data?: ConductorData }) {
  const results = useResults();
  const ink = useInk();
  const dotAtSource = useJunction(source, sourceHandleId);
  const dotAtTarget = useJunction(target, targetHandleId);
  const s = { x: sourceX, y: sourceY };
  const t = { x: targetX, y: targetY };
  const pts = liveRoute(data?.route, s, t);

  let path: string;
  let seg: { a: Pt; b: Pt };
  let mid: Pt;
  // Where the text goes: the layout's reserved box, else beside the arrow.
  let textAt: { x: number; y: number; anchor: "start" | "middle" } | null = null;
  const box = pts ? data?.labelAt : undefined;
  if (pts) {
    path = pts.map((p, i) => `${i ? "L" : "M"}${p.x},${p.y}`).join(" ");
    if (box) {
      const near = nearestOnRoute(pts, { x: box.x + box.width / 2, y: box.y + box.height / 2 });
      seg = near;
      mid = near.at;
      textAt = { x: box.x + box.width / 2, y: box.y + 8, anchor: "middle" };
    } else {
      seg = longestSegment(pts);
      mid = { x: (seg.a.x + seg.b.x) / 2, y: (seg.a.y + seg.b.y) / 2 };
    }
  } else {
    [path] = getSmoothStepPath({
      sourceX,
      sourceY,
      targetX,
      targetY,
      sourcePosition,
      targetPosition,
      borderRadius: 0,
    });
    seg = { a: s, b: t };
    mid = { x: (s.x + t.x) / 2, y: (s.y + t.y) / 2 };
  }

  const kv = data?.voltageKV ?? 0;
  const row = data?.caseRef?.kind === "branch" ? results?.branches.get(data.caseRef.id) : undefined;
  const flow = row ? branchFlow(row, data?.reversed) : null;
  const loading = loadingColor(row?.loading_pct);
  const overload = loading ? ink.alert(loading) : null;
  const offline = data?.outOfService;

  // With results on, wires go neutral so only loading stands out.
  const base = results ? (ink.mono ? ink.line : "#334155") : ink.voltage(kv);
  const stroke = selected ? SELECTED : offline ? ink.off : (overload ?? base);
  const width = (data?.conductorType === "transmission" ? 2 : 1.5) + (overload ? 1 : 0);

  const vertical = Math.abs(seg.a.x - seg.b.x) < 0.5;
  // Unit vector of the flow, along the labelled segment.
  const dx = Math.sign(seg.b.x - seg.a.x) * (flow && flow.p < 0 ? -1 : 1);
  const dy = Math.sign(seg.b.y - seg.a.y) * (flow && flow.p < 0 ? -1 : 1);
  const arrow =
    flow && Math.abs(flow.p) > 0.05
      ? `M${mid.x + dx * 5},${mid.y + dy * 5} L${mid.x - dx * 4 - dy * 4},${mid.y - dy * 4 + dx * 4} L${mid.x - dx * 4 + dy * 4},${mid.y - dy * 4 - dx * 4} Z`
      : null;
  const lines = flow
    ? [
        mw(Math.abs(flow.p)),
        ...(results?.ac ? [mvar(flow.q)] : []),
        ...(row?.loading_pct != null ? [`${row.loading_pct.toFixed(0)}%`] : []),
      ]
    : data?.label
      ? [data.label]
      : [];
  textAt ??= vertical
    ? { x: mid.x + 7, y: mid.y - (lines.length - 1) * 5.5 + 3, anchor: "start" }
    : { x: mid.x, y: mid.y - 6 - (lines.length - 1) * 11, anchor: "middle" };

  return (
    <>
      <BaseEdge
        id={id}
        path={path}
        style={{ stroke, strokeWidth: width, strokeDasharray: offline ? "5 4" : undefined }}
      />
      {selected && pts && <SegmentGrips id={id} points={pts} />}
      {dotAtSource && <circle cx={sourceX} cy={sourceY} r={3} fill={stroke} />}
      {dotAtTarget && <circle cx={targetX} cy={targetY} r={3} fill={stroke} />}
      {arrow && !offline && <path d={arrow} fill={overload ?? ink.result} />}
      {lines.length > 0 && (
        <text
          x={textAt.x}
          y={textAt.y}
          textAnchor={textAt.anchor}
          fontSize={9}
          fill={overload ?? (flow ? ink.result : ink.text)}
          fontWeight={overload ? 600 : 400}
          className="edge-label"
        >
          {lines.map((l, i) => (
            <tspan key={i} x={textAt.x} dy={i ? 11 : 0}>
              {l}
            </tspan>
          ))}
        </text>
      )}
    </>
  );
}
