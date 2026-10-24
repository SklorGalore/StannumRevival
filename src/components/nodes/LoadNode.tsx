import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { LoadNode as LoadNodeType } from "../../store/types";
import { mvar, mw, useResults, RESULT_TEXT } from "../../lib/diagram/results";

const W = 40;
const H = 56;

export function LoadNode({ data, selected }: NodeProps<LoadNodeType>) {
  const isMotor = data.loadType === "motor";
  const results = useResults();
  const row = data.caseRef?.kind === "load" ? results?.buses.get(data.caseRef.id) : undefined;

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
          /* Static load: arrow pointing down, away from the bus */
          <polygon
            points={`4,12 ${W - 4},12 ${W / 2},${H - 4}`}
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
        >
          {data.label}
        </text>
        {row && (
          <text x={W + 4} y={(isMotor ? 40 : 36) + 12} fontSize={9} fill={RESULT_TEXT}>
            <tspan>{mw(row.pd)}</tspan>
            {results?.ac && (
              <tspan x={W + 4} dy={11}>
                {mvar(row.qd)}
              </tspan>
            )}
          </text>
        )}
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
