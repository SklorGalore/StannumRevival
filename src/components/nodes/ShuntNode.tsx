import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { ShuntNode as ShuntNodeType } from "../../store/types";
import { mvar, mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";
import { Ground } from "./shared/Ground";

const W = 40;
const H = 60;
const CX = W / 2;

/** Fixed shunt: a capacitor (supplies Mvar) or a reactor (absorbs Mvar), to ground. */
export function ShuntNode({ data, selected }: NodeProps<ShuntNodeType>) {
  const results = useResults();
  const ink = useInk();
  const row = data.caseRef?.kind === "shunt" ? results?.buses.get(data.caseRef.id) : undefined;
  const stroke = selected ? SELECTED : ink.line;
  const reactor = data.nominalMVAR < 0;
  const textX = W + 2;

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        {reactor ? (
          <>
            <line x1={CX} y1={0} x2={CX} y2={16} stroke={ink.line} strokeWidth={STROKE} />
            {/* Reactor: a coil of three turns. */}
            <path
              d={`M${CX},16 a5,4 0 1,1 0,8 a5,4 0 1,1 0,8 a5,4 0 1,1 0,8`}
              fill="none"
              stroke={stroke}
              strokeWidth={STROKE}
            />
            <line x1={CX} y1={40} x2={CX} y2={48} stroke={ink.line} strokeWidth={STROKE} />
          </>
        ) : (
          <>
            <line x1={CX} y1={0} x2={CX} y2={22} stroke={ink.line} strokeWidth={STROKE} />
            {/* Capacitor: two plates. */}
            <path d={`M${CX - 11},22 h22 M${CX - 11},28 h22`} stroke={stroke} strokeWidth={STROKE + 0.5} />
            <line x1={CX} y1={28} x2={CX} y2={48} stroke={ink.line} strokeWidth={STROKE} />
          </>
        )}
        <Ground x={CX} y={48} color={ink.line} />
        <text x={textX} y={22} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
        <text x={textX} y={34} fontSize={9} fill={row ? ink.result : ink.muted}>
          {/* Shunt output scales with the square of the solved voltage. */}
          <tspan>{mvar(data.nominalMVAR * (row ? row.vm * row.vm : 1))}</tspan>
          {data.nominalMW !== 0 && (
            <tspan x={textX} dy={11}>
              {mw(data.nominalMW * (row ? row.vm * row.vm : 1))}
            </tspan>
          )}
        </text>
      </svg>
      <Handle type="target" position={Position.Top} id="shunt-top" style={{ top: 0, left: CX }} />
    </div>
  );
}
