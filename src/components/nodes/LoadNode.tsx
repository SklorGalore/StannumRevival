import type { NodeProps } from "@xyflow/react";
import type { LoadNode as LoadNodeType } from "../../store/types";
import { mvar, mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 60;
const CX = W / 2;
const TEXT_Y = 34;
const TERMINALS: Terminal[] = [{ id: "load-top", type: "target", x: CX, y: 0, side: "top" }];

export function LoadNode({ data, selected }: NodeProps<LoadNodeType>) {
  const isMotor = data.loadType === "motor";
  const results = useResults();
  const ink = useInk();
  const row = data.caseRef?.kind === "load" ? results?.buses.get(data.caseRef.id) : undefined;
  const outline = selected ? SELECTED : ink.line;

  return (
    <SymbolFrame
      w={W}
      h={H}
      rotation={data.rotation}
      terminals={TERMINALS}
      graphic={
        isMotor ? (
          <>
            <line x1={CX} y1={0} x2={CX} y2={22} stroke={ink.line} strokeWidth={STROKE} />
            <circle cx={CX} cy={38} r={16} fill="white" stroke={outline} strokeWidth={STROKE} />
            {/* The letter turns back so it reads upright. */}
            <text
              x={CX}
              y={42.5}
              textAnchor="middle"
              fontSize={13}
              fontWeight={600}
              fill={ink.line}
              transform={data.rotation ? `rotate(${-data.rotation} ${CX} 38)` : undefined}
            >
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
        )
      }
      labelTop={TEXT_Y - 9}
      label={
        <>
          <text y={TEXT_Y} fontSize={10} fill={ink.text}>
            {data.label}
          </text>
          {row && (
            <text y={TEXT_Y + 12} fontSize={9} fill={ink.result}>
              <tspan>{mw(row.pd)}</tspan>
              {results?.ac && (
                <tspan x={0} dy={11}>
                  {mvar(row.qd)}
                </tspan>
              )}
            </text>
          )}
        </>
      }
    />
  );
}
