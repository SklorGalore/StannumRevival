import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { FuseNode as FuseNodeType } from "../../store/types";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";

const W = 40;
const H = 60;
const CX = W / 2;
const BODY_W = 12;
const BODY_H = 24;
const BODY_Y = (H - BODY_H) / 2;

/** Fuse: a rectangle with the conductor running through it. */
export function FuseNode({ data, selected }: NodeProps<FuseNodeType>) {
  const ink = useInk();
  const blown = data.blown ? ink.alert("#dc2626") : null;
  const stroke = selected ? SELECTED : (blown ?? ink.line);

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        <line x1={CX} y1={0} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} strokeDasharray={blown ? `${BODY_Y} ${BODY_H}` : undefined} />
        <rect
          x={CX - BODY_W / 2}
          y={BODY_Y}
          width={BODY_W}
          height={BODY_H}
          fill={blown ? "white" : "none"}
          stroke={stroke}
          strokeWidth={STROKE}
        />
        {blown && (
          <path
            d={`M${CX - 4},${BODY_Y + 8} L${CX + 4},${BODY_Y + 16} M${CX + 4},${BODY_Y + 8} L${CX - 4},${BODY_Y + 16}`}
            stroke={blown}
            strokeWidth={STROKE}
          />
        )}
        <text x={W + 2} y={H / 2 + 3} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
      </svg>
      <Handle type="target" position={Position.Top} id="fuse-top" style={{ top: 0, left: CX }} />
      <Handle type="source" position={Position.Bottom} id="fuse-bottom" style={{ bottom: 0, left: CX }} />
    </div>
  );
}
