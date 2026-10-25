import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { OffPageNode as OffPageNodeType } from "../../store/types";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";

const W = 40;
const H = 40;
const CX = W / 2;

/** A branch that continues to a bus outside the drawing. Its flow is shown
 *  on the wire leading to it. */
export function OffPageNode({ data, selected }: NodeProps<OffPageNodeType>) {
  const ink = useInk();
  const stroke = selected ? SELECTED : data.outOfService ? ink.off : ink.line;
  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        <title>Double-click to bring {data.busName || `Bus ${data.bus}`} into the drawing</title>
        <line x1={CX} y1={0} x2={CX} y2={14} stroke={stroke} strokeWidth={STROKE} />
        {/* Off-page connector: a flag pointing away from the bus. */}
        <polygon
          points={`${CX - 8},14 ${CX + 8},14 ${CX + 8},30 ${CX},${H - 2} ${CX - 8},30`}
          fill="white"
          stroke={stroke}
          strokeWidth={STROKE}
          strokeLinejoin="round"
          strokeDasharray={data.outOfService ? "3 2" : undefined}
        />
        <text x={CX + 12} y={26} fontSize={9.5} fill={ink.text}>
          {data.label}
        </text>
      </svg>
      <Handle type="source" position={Position.Top} id="op-top" style={{ left: CX }} />
    </div>
  );
}
