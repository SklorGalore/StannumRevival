import { useReactFlow } from "@xyflow/react";
import { useDiagramStore } from "../../store/diagramStore";
import { moveSegment, type Pt } from "../../lib/diagram/router";
import { SELECTED } from "../../lib/diagram/ink";

/** Segments shorter than this get no grip: too small to aim at. */
const MIN_GRIP = 16;

/** Grips on a selected wire's segments: drag one sideways to move it. The
 *  wire then keeps that shape until it is rerouted. */
export function SegmentGrips({ id, points }: { id: string; points: Pt[] }) {
  const { screenToFlowPosition } = useReactFlow();

  const startDrag = (k: number) => (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    const store = useDiagramStore.getState();
    store.checkpoint();
    const from = points.map((p) => ({ ...p }));
    const vertical = Math.abs(from[k].x - from[k + 1].x) < 0.5;
    const origin = screenToFlowPosition({ x: e.clientX, y: e.clientY }, { snapToGrid: false });
    const at = vertical ? from[k].x : from[k].y;
    let last = 0;

    const move = (ev: PointerEvent) => {
      const p = screenToFlowPosition({ x: ev.clientX, y: ev.clientY }, { snapToGrid: false });
      const { gridSize, snapToGrid, edges, setEdges } = useDiagramStore.getState();
      let to = at + (vertical ? p.x - origin.x : p.y - origin.y);
      if (snapToGrid) to = Math.round(to / gridSize) * gridSize;
      const delta = to - at;
      if (delta === last) return;
      last = delta;
      const route = moveSegment(from, k, delta);
      // Written straight to the store: the checkpoint above covers the drag.
      setEdges(
        edges.map((w) => {
          if (w.id !== id || !w.data) return w;
          const { labelAt: _, ...data } = w.data;
          return { ...w, data: { ...data, route, pinnedRoute: true } };
        })
      );
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
  };

  return (
    <g className="segment-grips nopan nodrag">
      {points.slice(0, -1).map((a, k) => {
        const b = points[k + 1];
        const len = Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
        if (len < MIN_GRIP) return null;
        const vertical = Math.abs(a.x - b.x) < 0.5;
        const cx = (a.x + b.x) / 2;
        const cy = (a.y + b.y) / 2;
        const cursor = vertical ? "ew-resize" : "ns-resize";
        return (
          <g key={k} onPointerDown={startDrag(k)} style={{ cursor }} data-segment={k}>
            <path d={`M${a.x},${a.y} L${b.x},${b.y}`} stroke="transparent" strokeWidth={10} fill="none" />
            <rect
              x={cx - (vertical ? 3 : 6)}
              y={cy - (vertical ? 6 : 3)}
              width={vertical ? 6 : 12}
              height={vertical ? 12 : 6}
              rx={1.5}
              fill="white"
              stroke={SELECTED}
              strokeWidth={1.25}
            />
          </g>
        );
      })}
    </g>
  );
}
