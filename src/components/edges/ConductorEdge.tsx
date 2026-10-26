import { BaseEdge, getSmoothStepPath, type EdgeProps } from "@xyflow/react";
import type { ConductorData } from "../../store/types";
import { branchFlow, loadingColor, mvar, mw, useDrawnMaxFlow, useResults } from "../../lib/diagram/results";
import { flowWidth, useResultMarks } from "../../lib/diagram/detail";
import { useInk } from "../../lib/diagram/ink";
import { useDiagramStore } from "../../store/diagramStore";
import type { SLDEdge } from "../../store/types";
import { ROUTE_TOLERANCE, pinEnds, type Pt } from "../../lib/diagram/router";
import { arrowPath, flowDirection, pathPoints } from "../../lib/diagram/flowArrow";
import { SegmentGrips } from "./SegmentGrips";
import { Leader, useBBox, useLabelDrag } from "../nodes/shared/labels";
import { useElementMark } from "../../store/layerStore";

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

/** Direction a terminal on each side points, as a unit step. */
const OUT: Record<string, { x: number; y: number }> = {
  top: { x: 0, y: -1 },
  bottom: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
  right: { x: 1, y: 0 },
};

/** Two terminals that point at each other on one line (or touch). */
function facingInLine(s: { x: number; y: number }, sp: string, t: { x: number; y: number }, tp: string) {
  const a = OUT[sp];
  const b = OUT[tp];
  if (!a || !b || a.x !== -b.x || a.y !== -b.y) return false;
  // In line across the run, and the target not more than a little behind.
  const off = a.x !== 0 ? Math.abs(s.y - t.y) : Math.abs(s.x - t.x);
  const ahead = (t.x - s.x) * a.x + (t.y - s.y) * a.y;
  return off < 1 && ahead > -8;
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
  const marks = useResultMarks();
  const maxFlow = useDrawnMaxFlow(marks ? results : null);
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
  } else if (facingInLine(s, sourcePosition, t, targetPosition)) {
    // Straight across, or no length at all for a symbol sitting on its bus:
    // the step path would loop out past the terminals.
    path = `M${s.x},${s.y} L${t.x},${t.y}`;
    seg = { a: s, b: t };
    mid = { x: (s.x + t.x) / 2, y: (s.y + t.y) / 2 };
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
    // Put the arrow on a run of the step path itself, not on the straight
    // line between the ends (which is diagonal whenever the ends are offset).
    const steps = pathPoints(path);
    seg = steps.length > 1 ? longestSegment(steps) : { a: s, b: t };
    mid = { x: (seg.a.x + seg.b.x) / 2, y: (seg.a.y + seg.b.y) / 2 };
  }

  const kv = data?.voltageKV ?? 0;
  const row = data?.caseRef?.kind === "branch" ? results?.branches.get(data.caseRef.id) : undefined;
  const flow = row ? branchFlow(row, data?.reversed) : null;
  const loading = loadingColor(row?.loading_pct);
  const overload = loading ? ink.alert(loading) : null;
  // Out in the file, or taken out in the study (the solve says so).
  const offline = data?.outOfService || (row && !row.online);

  // A wire standing for a DC line (to an off-page marker) is drawn dashed in ink.
  const dc = data?.caseRef?.kind === "dcline";
  // With results on, wires go neutral so only loading stands out.
  const base = dc ? ink.line : results ? ink.wire : ink.voltage(kv);
  const layer = useElementMark(data?.caseRef?.element);
  const stroke = selected ? ink.selected : offline ? ink.off : (overload ?? (layer?.color && !ink.mono ? layer.color : base));
  const baseWidth = (data?.conductorType === "transmission" ? 2 : 1.5) + (overload ? 1 : 0) + (layer?.bold ? 2 : 0);
  // Zoomed out, the wire's weight carries its loading instead of the text.
  const asMarks = marks && !!flow;
  const width = asMarks && flow ? flowWidth(row?.loading_pct, flow.p, maxFlow, baseWidth) : baseWidth;
  const k = asMarks ? 2 : 1;

  // Unit step of the flow along the labelled segment's run.
  const dir = flowDirection(seg.a, seg.b, !!flow && flow.p < 0);
  const vertical = dir.x === 0;
  const arrow = flow && Math.abs(flow.p) > 0.05 ? arrowPath(mid, dir, k) : null;
  const numbers = flow
    ? [
        mw(Math.abs(flow.p)),
        ...(results?.ac ? [mvar(flow.q)] : []),
        ...(row?.loading_pct != null ? [`${row.loading_pct.toFixed(0)}%`] : []),
      ]
    : data?.label
      ? [data.label]
      : [];
  const lines = asMarks ? [] : numbers;
  textAt ??= vertical
    ? { x: mid.x + 7, y: mid.y - (lines.length - 1) * 5.5 + 3, anchor: "start" }
    : { x: mid.x, y: mid.y - 6 - (lines.length - 1) * 11, anchor: "middle" };
  // A dragged label keeps its distance from its spot as the wire moves.
  const moved = data?.labelOffset;
  if (moved) textAt = { ...textAt, x: textAt.x + moved.x, y: textAt.y + moved.y };
  const drag = useLabelDrag(moved, (next) => useDiagramStore.getState().updateEdgeData(id, { labelOffset: next }));
  const [textRef, textBox] = useBBox<SVGTextElement>([textAt.x, textAt.y, lines.join("|")]);

  return (
    <g opacity={layer?.dim ? 0.25 : undefined}>
      {layer?.halo && (
        // A layer's mark (a bridge): a dashed band along the wire.
        <path
          d={path}
          fill="none"
          stroke={ink.mono ? ink.line : layer.halo}
          strokeOpacity={ink.mono ? 1 : 0.55}
          strokeWidth={ink.mono ? 1 : 7}
          strokeDasharray="8 4"
          strokeLinejoin="round"
          data-testid="layer-ring"
          transform={ink.mono ? "translate(0,5)" : undefined}
        >
          {layer.note && <title>{layer.note}</title>}
        </path>
      )}
      {data?.focus && (
        // The branch a local drawing was made around.
        <path
          d={path}
          fill="none"
          stroke={ink.halo}
          strokeOpacity={ink.mono ? 1 : 0.35}
          strokeWidth={ink.mono ? 1 : 14}
          strokeDasharray={ink.mono ? "4 3" : undefined}
          strokeLinejoin="round"
          data-testid="focus-halo"
          transform={ink.mono ? "translate(0,-6)" : undefined}
        >
          <title>Studied element</title>
        </path>
      )}
      <BaseEdge
        id={id}
        path={path}
        style={{ stroke, strokeWidth: width, strokeDasharray: offline ? "5 4" : dc ? "6 4" : undefined }}
      />
      {asMarks && (
        // Hover target for the numbers the marks stand for.
        <path d={path} fill="none" stroke="transparent" strokeWidth={14} data-testid="result-mark">
          <title>{numbers.join(" · ")}</title>
        </path>
      )}
      {selected && pts && <SegmentGrips id={id} points={pts} />}
      {dotAtSource && <circle cx={sourceX} cy={sourceY} r={3} fill={stroke} />}
      {dotAtTarget && <circle cx={targetX} cy={targetY} r={3} fill={stroke} />}
      {arrow && !offline && <path d={arrow} fill={overload ?? ink.result} />}
      {moved && lines.length > 0 && (
        <Leader from={{ x: mid.x, y: mid.y, w: 0, h: 0 }} to={textBox} color={ink.muted} />
      )}
      {lines.length > 0 && (
        <text
          ref={textRef}
          {...drag}
          x={textAt.x}
          y={textAt.y}
          textAnchor={textAt.anchor}
          fontSize={9}
          fill={overload ?? (flow ? ink.result : ink.text)}
          fontWeight={overload ? 600 : 400}
          className={`edge-label ${drag.className}`}
        >
          <title>Drag to move the label</title>
          {lines.map((l, i) => (
            <tspan key={i} x={textAt.x} dy={i ? 11 : 0}>
              {l}
            </tspan>
          ))}
        </text>
      )}
    </g>
  );
}
