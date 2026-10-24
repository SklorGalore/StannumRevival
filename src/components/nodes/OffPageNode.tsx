import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { OffPageNode as OffPageNodeType } from "../../store/types";

const W = 20;
const H = 28;

/** A branch that continues to a bus outside the drawing. Its flow is shown
 *  on the wire leading to it. */
export function OffPageNode({ data, selected }: NodeProps<OffPageNodeType>) {
  const stroke = selected ? "#3b82f6" : data.outOfService ? "#94a3b8" : "#334155";
  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        <line x1={W / 2} y1={0} x2={W / 2} y2={8} stroke={stroke} strokeWidth={1.5} />
        {/* Off-page connector: a flag pointing away from the bus. */}
        <polygon
          points={`2,8 ${W - 2},8 ${W - 2},20 ${W / 2},${H - 1} 2,20`}
          fill="white"
          stroke={stroke}
          strokeWidth={1.5}
          strokeDasharray={data.outOfService ? "3 2" : undefined}
        />
        <text x={W + 5} y={14} fontSize={9.5} fill="#475569">
          {data.label}
        </text>
      </svg>
      <Handle type="source" position={Position.Top} id="op-top" style={{ left: W / 2 }} />
    </div>
  );
}
