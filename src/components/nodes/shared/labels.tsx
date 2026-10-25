import { useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { useReactFlow } from "@xyflow/react";

export type Offset = { x: number; y: number };
type Rect = { x: number; y: number; w: number; h: number };

/** Moved labels snap to this many px. */
export const LABEL_SNAP = 5;

/**
 * Pointer handlers that drag a label. `onMove` gets the new offset, snapped,
 * each time it changes; the store coalesces those writes into one undo step.
 */
export function useLabelDrag(offset: Offset | undefined, onMove: (next: Offset) => void) {
  const { getZoom } = useReactFlow();
  const start = useRef<{ x: number; y: number; from: Offset; last: Offset } | null>(null);
  return {
    className: "label-drag nodrag nopan",
    onPointerDown: (e: ReactPointerEvent<SVGElement>) => {
      if (e.button !== 0) return;
      e.stopPropagation();
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      const from = offset ?? { x: 0, y: 0 };
      start.current = { x: e.clientX, y: e.clientY, from, last: from };
    },
    onPointerMove: (e: ReactPointerEvent<SVGElement>) => {
      const s = start.current;
      if (!s) return;
      const zoom = getZoom();
      const snap = (v: number) => Math.round(v / LABEL_SNAP) * LABEL_SNAP;
      const next = { x: snap(s.from.x + (e.clientX - s.x) / zoom), y: snap(s.from.y + (e.clientY - s.y) / zoom) };
      if (next.x === s.last.x && next.y === s.last.y) return;
      s.last = next;
      onMove(next);
    },
    onPointerUp: () => {
      start.current = null;
    },
  };
}

/** Bounding box of an SVG element, kept up to date across renders. */
export function useBBox<T extends SVGGraphicsElement>(deps: unknown[]) {
  const ref = useRef<T>(null);
  const [box, setBox] = useState<Rect | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return setBox(null);
    const b = el.getBBox();
    setBox((old) =>
      old && old.x === b.x && old.y === b.y && old.w === b.width && old.h === b.height
        ? old
        : { x: b.x, y: b.y, w: b.width, h: b.height }
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return [ref, box] as const;
}

/** Closest points of two boxes, and the gap between them. */
function closest(a: Rect, b: Rect) {
  const pick = (a0: number, a1: number, b0: number, b1: number) =>
    b0 > a1 ? [a1, b0] : a0 > b1 ? [a0, b1] : [Math.max(a0, b0) / 2 + Math.min(a1, b1) / 2, Math.max(a0, b0) / 2 + Math.min(a1, b1) / 2];
  const [ax, bx] = pick(a.x, a.x + a.w, b.x, b.x + b.w);
  const [ay, by] = pick(a.y, a.y + a.h, b.y, b.y + b.h);
  return { a: { x: ax, y: ay }, b: { x: bx, y: by }, gap: Math.hypot(bx - ax, by - ay) };
}

/** Thin line tying a moved label back to what it belongs to, once it has
 *  been pulled clear of it. */
export function Leader({ from, to, color }: { from: Rect; to: Rect | null; color: string }) {
  if (!to) return null;
  const { a, b, gap } = closest(from, to);
  if (gap < 12) return null;
  // Stop just short of the text.
  const end = { x: b.x - ((b.x - a.x) / gap) * 3, y: b.y - ((b.y - a.y) / gap) * 3 };
  return (
    <line
      className="label-leader"
      x1={a.x}
      y1={a.y}
      x2={end.x}
      y2={end.y}
      stroke={color}
      strokeWidth={0.75}
      strokeDasharray="2 2"
      pointerEvents="none"
    />
  );
}

export const isMoved = (o: Offset | undefined) => !!o && (o.x !== 0 || o.y !== 0);
