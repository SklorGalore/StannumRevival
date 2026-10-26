import { Handle, Position, useNodeId } from "@xyflow/react";
import type { ReactNode } from "react";
import type { LabelOffset, Rotation } from "../../../store/types";
import { useDiagramStore } from "../../../store/diagramStore";
import { useInk } from "../../../lib/diagram/ink";
import { Leader, useBBox, useLabelDrag } from "./labels";
import { useRemeasure } from "./remeasure";

export type Side = "top" | "right" | "bottom" | "left";

/** A connection point, in the symbol's own (unrotated) coordinates. */
export interface Terminal {
  id: string;
  type: "source" | "target";
  x: number;
  y: number;
  side: Side;
}

const SIDES: Side[] = ["top", "right", "bottom", "left"];
const POSITION: Record<Side, Position> = {
  top: Position.Top,
  right: Position.Right,
  bottom: Position.Bottom,
  left: Position.Left,
};

/** Size of a w×h symbol once turned. */
export function rotatedSize(w: number, h: number, rotation: Rotation = 0) {
  return rotation === 90 || rotation === 270 ? { w: h, h: w } : { w, h };
}

/** Where a point of a w×h symbol lands once turned clockwise by `rotation`. */
export function rotatePoint(x: number, y: number, w: number, h: number, rotation: Rotation = 0) {
  switch (rotation) {
    case 90:
      return { x: h - y, y: x };
    case 180:
      return { x: w - x, y: h - y };
    case 270:
      return { x: y, y: w - x };
    default:
      return { x, y };
  }
}

export const rotateSide = (side: Side, rotation: Rotation = 0): Side =>
  SIDES[(SIDES.indexOf(side) + rotation / 90) % 4];

/** Places a handle with its outer edge on the terminal, inside the symbol:
 *  React Flow ends a wire at the handle's outer edge (see index.css). */
export function handleStyle(x: number, y: number, side: Side) {
  const shift = { top: "-50%, 0", bottom: "-50%, -100%", left: "0, -50%", right: "-100%, -50%" }[side];
  return { left: x, top: y, right: "auto", bottom: "auto", transform: `translate(${shift})` };
}

const TURN: Record<Rotation, (w: number, h: number) => string | undefined> = {
  0: () => undefined,
  90: (_w, h) => `translate(${h},0) rotate(90)`,
  180: (w, h) => `translate(${w},${h}) rotate(180)`,
  270: (w) => `translate(0,${w}) rotate(270)`,
};

/**
 * Draws a symbol turned by `rotation` (clockwise, in 90° steps). The graphic
 * and the terminals turn; the text stays upright. The label is laid out as if
 * the symbol were upright, to its right (x from 0). Upright or upside down it
 * stays there; turned on its side, the block moves under the symbol, centred,
 * its top line (`labelTop`) just below it. Dragging the label moves it by
 * `labelOffset`; once clear of the symbol a leader line ties it back.
 */
export function SymbolFrame({
  w,
  h,
  rotation = 0,
  graphic,
  label,
  labelTop = 0,
  labelOffset,
  terminals,
  title,
  onDoubleClick,
  focus,
}: {
  w: number;
  h: number;
  rotation?: Rotation;
  graphic: ReactNode;
  /** Text lines at x = 0, using the symbol's own y coordinates. */
  label?: ReactNode;
  /** Top of the label block, in the symbol's own y coordinates. */
  labelTop?: number;
  labelOffset?: LabelOffset;
  terminals: Terminal[];
  title?: string;
  onDoubleClick?: () => void;
  /** The element a local drawing was made around: drawn with a halo. */
  focus?: boolean;
}) {
  const size = rotatedSize(w, h, rotation);
  // Handles move when the symbol turns: re-measure so wires follow.
  const id = useNodeId();
  useRemeasure(String(rotation));
  const sideways = rotation === 90 || rotation === 270;
  // Sideways, the block is centred under the symbol (text-anchor inherits).
  const base = sideways ? { x: size.w / 2, y: size.h + 6 - labelTop } : { x: size.w + 2, y: 0 };
  const off = labelOffset ?? { x: 0, y: 0 };
  const labelAt = { x: base.x + off.x, y: base.y + off.y };
  const ink = useInk();
  const drag = useLabelDrag(labelOffset, (next) => id && useDiagramStore.getState().updateNodeData(id, { labelOffset: next }));
  const [labelRef, labelBox] = useBBox<SVGGElement>([labelAt.x, labelAt.y, label, rotation]);
  return (
    <div className="node-wrapper relative" style={{ width: size.w, height: size.h }}>
      <svg width={size.w} height={size.h} overflow="visible" style={{ display: "block" }} onDoubleClick={onDoubleClick}>
        {title && <title>{title}</title>}
        {focus && <FocusHalo w={size.w} h={size.h} />}
        <g transform={TURN[rotation](w, h)}>{graphic}</g>
        {label && labelOffset && labelBox && (
          <Leader
            from={{ x: 0, y: 0, w: size.w, h: size.h }}
            to={{ x: labelBox.x + labelAt.x, y: labelBox.y + labelAt.y, w: labelBox.w, h: labelBox.h }}
            color={ink.muted}
          />
        )}
        {label && (
          <g
            ref={labelRef}
            transform={`translate(${labelAt.x},${labelAt.y})`}
            textAnchor={sideways ? "middle" : undefined}
            {...drag}
          >
            <title>Drag to move the label</title>
            {label}
          </g>
        )}
      </svg>
      {terminals.map((t) => {
        const p = rotatePoint(t.x, t.y, w, h, rotation);
        const side = rotateSide(t.side, rotation);
        return (
          <Handle
            key={t.id}
            type={t.type}
            id={t.id}
            position={POSITION[side]}
            style={handleStyle(p.x, p.y, side)}
          />
        );
      })}
    </div>
  );
}

/** Halo behind the element a local drawing was made around. */
export function FocusHalo({ w, h, x = 0, y = 0 }: { w: number; h: number; x?: number; y?: number }) {
  const ink = useInk();
  return (
    <rect
      x={x - 7}
      y={y - 7}
      width={w + 14}
      height={h + 14}
      rx={6}
      fill={ink.mono ? "none" : ink.halo}
      fillOpacity={ink.mono ? undefined : 0.18}
      stroke={ink.halo}
      strokeWidth={ink.mono ? 1 : 1.5}
      strokeDasharray={ink.mono ? "4 3" : undefined}
      data-testid="focus-halo"
    >
      <title>Studied element</title>
    </rect>
  );
}
