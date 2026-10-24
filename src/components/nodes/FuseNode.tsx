import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { FuseNode as FuseNodeType } from "../../store/types";

const W = 24;
const H = 48;
const RECT_W = 16;
const RECT_H = 20;
const RECT_X = (W - RECT_W) / 2;
const RECT_Y = 14;

export function FuseNode({ data, selected }: NodeProps<FuseNodeType>) {
  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block" }}
      >
        {/* Top stem */}
        <line x1={W / 2} y1={0} x2={W / 2} y2={RECT_Y} stroke="#1e293b" strokeWidth={2} />

        {/* Fuse body */}
        <rect
          x={RECT_X}
          y={RECT_Y}
          width={RECT_W}
          height={RECT_H}
          fill={data.blown ? "#fee2e2" : "white"}
          stroke={selected ? "#3b82f6" : data.blown ? "#dc2626" : "#1e293b"}
          strokeWidth={selected ? 2 : 1.5}
          rx={2}
        />

        {data.blown ? (
          /* Blown: red X */
          <>
            <line
              x1={RECT_X + 3}
              y1={RECT_Y + 3}
              x2={RECT_X + RECT_W - 3}
              y2={RECT_Y + RECT_H - 3}
              stroke="#dc2626"
              strokeWidth={1.5}
            />
            <line
              x1={RECT_X + RECT_W - 3}
              y1={RECT_Y + 3}
              x2={RECT_X + 3}
              y2={RECT_Y + RECT_H - 3}
              stroke="#dc2626"
              strokeWidth={1.5}
            />
          </>
        ) : (
          /* Normal: center dot */
          <circle
            cx={W / 2}
            cy={RECT_Y + RECT_H / 2}
            r={2.5}
            fill="#1e293b"
          />
        )}

        {/* Bottom stem */}
        <line
          x1={W / 2}
          y1={RECT_Y + RECT_H}
          x2={W / 2}
          y2={H}
          stroke="#1e293b"
          strokeWidth={2}
        />

        {/* Label */}
        <text x={W + 4} y={28} fontSize={9} fill="#475569" fontFamily="monospace">
          {data.label}
        </text>
      </svg>

      <Handle
        type="target"
        position={Position.Top}
        id="fuse-top"
        style={{ top: 0, left: W / 2 }}
      />
      <Handle
        type="source"
        position={Position.Bottom}
        id="fuse-bottom"
        style={{ bottom: 0, left: W / 2 }}
      />
    </div>
  );
}
