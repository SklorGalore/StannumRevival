import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { LoadNode as LoadNodeType } from "../../store/types";
import { mvar, mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";

const W = 40;
const H = 60;
const CX = W / 2;

export function LoadNode({ data, selected }: NodeProps<LoadNodeType>) {
  const isMotor = data.loadType === "motor";
  const results = useResults();
  const ink = useInk();
  const row = data.caseRef?.kind === "load" ? results?.buses.get(data.caseRef.id) : undefined;
  const outline = selected ? SELECTED : ink.line;
  const textX = W + 2;
  const textY = 34;

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        {isMotor ? (
          <>
            <line x1={CX} y1={0} x2={CX} y2={22} stroke={ink.line} strokeWidth={STROKE} />
            <circle cx={CX} cy={38} r={16} fill="white" stroke={outline} strokeWidth={STROKE} />
            <text x={CX} y={42.5} textAnchor="middle" fontSize={13} fontWeight={600} fill={ink.line}>
              M
            </text>
          </>
        ) : (
          <>
            <line x1={CX} y1={0} x2={CX} y2={24} stroke={ink.line} strokeWidth={STROKE} />
            {/* Static load: an arrow pointing away from the bus. */}
            <polygon
              points={`${CX - 9},24 ${CX + 9},24 ${CX},${H}`}
              fill={ink.line}
              stroke={outline}
              strokeWidth={STROKE}
              strokeLinejoin="round"
            />
          </>
        )}
        <text x={textX} y={textY} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
        {row && (
          <text x={textX} y={textY + 12} fontSize={9} fill={ink.result}>
            <tspan>{mw(row.pd)}</tspan>
            {results?.ac && (
              <tspan x={textX} dy={11}>
                {mvar(row.qd)}
              </tspan>
            )}
          </text>
        )}
      </svg>
      <Handle type="target" position={Position.Top} id="load-top" style={{ top: 0, left: CX }} />
    </div>
  );
}
