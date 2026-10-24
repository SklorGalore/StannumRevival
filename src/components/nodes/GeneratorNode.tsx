import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { GeneratorNode as GeneratorNodeType } from "../../store/types";

const W = 48;
const H = 80;

export function GeneratorNode({ data, selected }: NodeProps<GeneratorNodeType>) {
  const isGrounded = data.groundingType === "solidly" || data.groundingType === "resistance";

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block" }}
      >
        {/* Stem from top */}
        <line x1={W / 2} y1={0} x2={W / 2} y2={10} stroke="#1e293b" strokeWidth={2} />

        {/* Generator circle */}
        <circle
          cx={W / 2}
          cy={30}
          r={20}
          fill="white"
          stroke={selected ? "#3b82f6" : "#1e293b"}
          strokeWidth={2}
        />
        <text
          x={W / 2}
          y={35}
          textAnchor="middle"
          fontSize={18}
          fontWeight="bold"
          fill="#1e293b"
          fontFamily="sans-serif"
        >
          G
        </text>

        {/* Stem to bottom */}
        <line x1={W / 2} y1={50} x2={W / 2} y2={60} stroke="#1e293b" strokeWidth={2} />

        {/* Ground symbol */}
        {isGrounded && (
          <>
            <line x1={W / 2 - 8} y1={63} x2={W / 2 + 8} y2={63} stroke="#1e293b" strokeWidth={2} />
            <line x1={W / 2 - 5} y1={67} x2={W / 2 + 5} y2={67} stroke="#1e293b" strokeWidth={1.5} />
            <line x1={W / 2 - 2} y1={71} x2={W / 2 + 2} y2={71} stroke="#1e293b" strokeWidth={1} />
          </>
        )}

        {/* Label */}
        <text
          x={W + 4}
          y={34}
          fontSize={10}
          fill="#475569"
          fontFamily="monospace"
        >
          {data.label}
        </text>
      </svg>

      {/* Top connection handle */}
      <Handle
        type="source"
        position={Position.Top}
        id="gen-top"
        style={{ top: 0, left: W / 2 }}
      />
    </div>
  );
}
