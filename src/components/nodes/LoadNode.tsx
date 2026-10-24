import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { LoadNode as LoadNodeType } from "../../store/types";

const W = 40;
const H = 56;

export function LoadNode({ data, selected }: NodeProps<LoadNodeType>) {
  const isMotor = data.loadType === "motor";

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block" }}
      >
        {/* Stem from top */}
        <line x1={W / 2} y1={0} x2={W / 2} y2={12} stroke="#1e293b" strokeWidth={2} />

        {isMotor ? (
          <>
            <circle
              cx={W / 2}
              cy={36}
              r={20}
              fill="white"
              stroke={selected ? "#3b82f6" : "#1e293b"}
              strokeWidth={2}
            />
            <text
              x={W / 2}
              y={41}
              textAnchor="middle"
              fontSize={16}
              fontWeight="bold"
              fill="#1e293b"
              fontFamily="sans-serif"
            >
              M
            </text>
          </>
        ) : (
          /* Static load: downward triangle */
          <polygon
            points={`${W / 2},12 4,${H - 4} ${W - 4},${H - 4}`}
            fill={selected ? "#dbeafe" : "#f1f5f9"}
            stroke={selected ? "#3b82f6" : "#1e293b"}
            strokeWidth={2}
          />
        )}

        {/* Label */}
        <text
          x={W + 4}
          y={isMotor ? 40 : 36}
          fontSize={10}
          fill="#475569"
          fontFamily="monospace"
        >
          {data.label}
        </text>
      </svg>

      <Handle
        type="target"
        position={Position.Top}
        id="load-top"
        style={{ top: 0, left: W / 2 }}
      />
    </div>
  );
}
