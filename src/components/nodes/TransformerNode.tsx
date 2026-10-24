import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { TransformerNode as TransformerNodeType } from "../../store/types";
import { branchFlow, loadingColor, mvar, mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";
import { Ground } from "./shared/Ground";

const W = 40;
const H = 100;
const CX = W / 2;
const R = 16;
/** Winding centres: two overlapping circles, the IEC two-winding symbol. */
const HV = 38;
const LV = 62;

function WindingSymbol({ cx, cy, type, color }: { cx: number; cy: number; type: "Wye" | "Delta"; color: string }) {
  if (type === "Wye") {
    return (
      <path
        d={`M${cx},${cy - 6} V${cy} L${cx - 5},${cy + 4} M${cx},${cy} L${cx + 5},${cy + 4}`}
        fill="none"
        stroke={color}
        strokeWidth={1.25}
      />
    );
  }
  return (
    <polygon
      points={`${cx},${cy - 6} ${cx - 6},${cy + 4} ${cx + 6},${cy + 4}`}
      fill="none"
      stroke={color}
      strokeWidth={1.25}
      strokeLinejoin="round"
    />
  );
}

export function TransformerNode({ data, selected }: NodeProps<TransformerNodeType>) {
  const primaryWinding = data.connection.startsWith("Wye") ? "Wye" : "Delta";
  const secondaryWinding = data.connection.endsWith("Wye") ? "Wye" : "Delta";
  const results = useResults();
  const ink = useInk();
  const row = data.caseRef?.kind === "branch" ? results?.branches.get(data.caseRef.id) : undefined;
  const flow = row ? branchFlow(row, data.reversed) : null;
  const overload = loadingColor(row?.loading_pct);
  const outline = selected ? SELECTED : data.outOfService ? ink.off : overload ? ink.alert(overload) : ink.line;
  const dash = data.outOfService ? "4 3" : undefined;
  const textX = W + 2;
  const grounded = !data.windingsUnknown && data.groundingSecondary === "solidly" && secondaryWinding === "Wye";

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        <line x1={CX} y1={0} x2={CX} y2={HV - R} stroke={ink.line} strokeWidth={STROKE} />
        <line x1={CX} y1={LV + R} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} />
        <circle cx={CX} cy={HV} r={R} fill="white" />
        <circle cx={CX} cy={LV} r={R} fill="white" />
        <circle cx={CX} cy={HV} r={R} fill="none" stroke={outline} strokeWidth={STROKE} strokeDasharray={dash} />
        <circle cx={CX} cy={LV} r={R} fill="none" stroke={outline} strokeWidth={STROKE} strokeDasharray={dash} />
        {!data.windingsUnknown && (
          <>
            <WindingSymbol cx={CX} cy={HV - 4} type={primaryWinding} color={ink.text} />
            <WindingSymbol cx={CX} cy={LV + 5} type={secondaryWinding} color={ink.text} />
          </>
        )}
        {grounded && (
          <>
            <path d={`M${CX - R},${LV} h-8 v8`} fill="none" stroke={ink.line} strokeWidth={STROKE} />
            <Ground x={CX - R - 8} y={LV + 8} color={ink.line} />
          </>
        )}

        <text x={textX} y={28} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
        <text x={textX} y={40} fontSize={9} fill={ink.muted}>
          {data.ratingMVA > 0 ? `${data.ratingMVA} MVA` : "Unrated"}
        </text>
        <text x={textX} y={52} fontSize={9} fill={ink.muted}>
          {data.primaryKV && data.secondaryKV
            ? `${data.primaryKV}/${data.secondaryKV} kV`
            : `${data.primaryKV || data.secondaryKV} kV winding`}
        </text>
        {flow && (
          <text
            x={textX}
            y={68}
            fontSize={9}
            fill={overload ? ink.alert(overload) : ink.result}
            fontWeight={overload ? 600 : 400}
          >
            <tspan>{mw(Math.abs(flow.p))}</tspan>
            {results?.ac && (
              <tspan x={textX} dy={11}>
                {mvar(flow.q)}
              </tspan>
            )}
            {row?.loading_pct != null && (
              <tspan x={textX} dy={11}>
                {row.loading_pct.toFixed(0)}%
              </tspan>
            )}
          </text>
        )}
      </svg>
      <Handle type="target" position={Position.Top} id="xfmr-primary" style={{ top: 0, left: CX }} />
      <Handle type="source" position={Position.Bottom} id="xfmr-secondary" style={{ bottom: 0, left: CX }} />
    </div>
  );
}
