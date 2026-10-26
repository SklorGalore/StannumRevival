import type { NodeProps } from "@xyflow/react";
import type { GeneratorNode as GeneratorNodeType } from "../../store/types";
import { mvar, mw, useResults } from "../../lib/diagram/results";
import { useResultMarks } from "../../lib/diagram/detail";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { Ground } from "./shared/Ground";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 80;
const CX = W / 2;
const CY = 36;
const R = 16;
const TERMINALS: Terminal[] = [{ id: "gen-top", type: "source", x: CX, y: 0, side: "top" }];

export function GeneratorNode({ data, selected }: NodeProps<GeneratorNodeType>) {
  const isGrounded = data.groundingType === "solidly" || data.groundingType === "resistance";
  const results = useResults();
  const ink = useInk();
  const marks = useResultMarks();
  const row = data.caseRef?.kind === "gen" ? results?.generators.get(data.caseRef.id) : undefined;

  return (
    <SymbolFrame
      w={W}
      h={H}
      focus={data.focus}
      rotation={data.rotation}
      labelOffset={data.labelOffset}
      terminals={TERMINALS}
      graphic={
        <>
          <line x1={CX} y1={0} x2={CX} y2={CY - R} stroke={ink.line} strokeWidth={STROKE} />
          <circle
            cx={CX}
            cy={CY}
            r={R}
            fill="white"
            stroke={selected ? ink.selected : marks && row?.online && row.q_violation > 0 ? ink.alert("#dc2626") : ink.line}
            strokeWidth={STROKE}
          >
            {row && marks && <title>{row.online ? [mw(row.pg), ...(results?.ac ? [mvar(row.qg)] : [])].join(" · ") : "Offline"}</title>}
          </circle>
          {/* Sine wave: the IEC mark for an AC machine. */}
          <path d={`M${CX - 9},${CY} q4.5,-9 9,0 t9,0`} fill="none" stroke={ink.line} strokeWidth={STROKE} />
          {isGrounded && (
            <>
              <line x1={CX} y1={CY + R} x2={CX} y2={62} stroke={ink.line} strokeWidth={STROKE} />
              <Ground x={CX} y={62} color={ink.line} />
            </>
          )}
        </>
      }
      labelTop={CY - 11}
      label={
        <>
          <text y={CY - 2} fontSize={10} fill={ink.text}>
            {data.label}
          </text>
          {row && !marks && (
            <text y={CY + 10} fontSize={9} fill={row.online ? ink.result : ink.off}>
              {row.online ? (
                <>
                  <tspan>{mw(row.pg)}</tspan>
                  {results?.ac && (
                    <tspan x={0} dy={11} fill={row.q_violation > 0 ? ink.alert("#dc2626") : undefined}>
                      {mvar(row.qg)}
                    </tspan>
                  )}
                </>
              ) : (
                "Offline"
              )}
            </text>
          )}
        </>
      }
    />
  );
}
