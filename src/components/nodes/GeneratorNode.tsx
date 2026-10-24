import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { GeneratorNode as GeneratorNodeType } from "../../store/types";
import { mvar, mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";
import { Ground } from "./shared/Ground";

const W = 40;
const H = 80;
const CX = W / 2;
const CY = 36;
const R = 16;

export function GeneratorNode({ data, selected }: NodeProps<GeneratorNodeType>) {
  const isGrounded = data.groundingType === "solidly" || data.groundingType === "resistance";
  const results = useResults();
  const ink = useInk();
  const row = data.caseRef?.kind === "gen" ? results?.generators.get(data.caseRef.id) : undefined;
  const textX = W + 2;

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        <line x1={CX} y1={0} x2={CX} y2={CY - R} stroke={ink.line} strokeWidth={STROKE} />
        <circle cx={CX} cy={CY} r={R} fill="white" stroke={selected ? SELECTED : ink.line} strokeWidth={STROKE} />
        {/* Sine wave: the IEC mark for an AC machine. */}
        <path
          d={`M${CX - 9},${CY} q4.5,-9 9,0 t9,0`}
          fill="none"
          stroke={ink.line}
          strokeWidth={STROKE}
        />
        {isGrounded && (
          <>
            <line x1={CX} y1={CY + R} x2={CX} y2={62} stroke={ink.line} strokeWidth={STROKE} />
            <Ground x={CX} y={62} color={ink.line} />
          </>
        )}
        <text x={textX} y={CY - 2} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
        {row && (
          <text x={textX} y={CY + 10} fontSize={9} fill={row.online ? ink.result : ink.off}>
            {row.online ? (
              <>
                <tspan>{mw(row.pg)}</tspan>
                {results?.ac && (
                  <tspan x={textX} dy={11} fill={row.q_violation > 0 ? ink.alert("#dc2626") : undefined}>
                    {mvar(row.qg)}
                  </tspan>
                )}
              </>
            ) : (
              "Offline"
            )}
          </text>
        )}
      </svg>
      <Handle type="source" position={Position.Top} id="gen-top" style={{ top: 0, left: CX }} />
    </div>
  );
}
