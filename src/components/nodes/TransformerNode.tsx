import type { NodeProps } from "@xyflow/react";
import type { TransformerNode as TransformerNodeType } from "../../store/types";
import { branchFlow, loadingColor, mvar, mw, useResults } from "../../lib/diagram/results";
import { useResultMarks } from "../../lib/diagram/detail";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { Ground } from "./shared/Ground";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 100;
const CX = W / 2;
const R = 16;
/** Winding centres: two overlapping circles, the IEC two-winding symbol. */
const HV = 38;
const LV = 62;
const TERMINALS: Terminal[] = [
  { id: "xfmr-primary", type: "target", x: CX, y: 0, side: "top" },
  { id: "xfmr-secondary", type: "source", x: CX, y: H, side: "bottom" },
];

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
  const marks = useResultMarks();
  const row = data.caseRef?.kind === "branch" ? results?.branches.get(data.caseRef.id) : undefined;
  const flow = row ? branchFlow(row, data.reversed) : null;
  const overload = loadingColor(row?.loading_pct);
  const offline = data.outOfService || (row && !row.online);
  const outline = selected ? ink.selected : offline ? ink.off : overload ? ink.alert(overload) : ink.line;
  const dash = offline ? "4 3" : undefined;
  const grounded = !data.windingsUnknown && data.groundingSecondary === "solidly" && secondaryWinding === "Wye";

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
          <line x1={CX} y1={0} x2={CX} y2={HV - R} stroke={ink.line} strokeWidth={STROKE} />
          <line x1={CX} y1={LV + R} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} />
          <circle cx={CX} cy={HV} r={R} fill="white">
            {flow && marks && (
              <title>
                {[mw(Math.abs(flow.p)), ...(results?.ac ? [mvar(flow.q)] : []), ...(row?.loading_pct != null ? [`${row.loading_pct.toFixed(0)}%`] : [])].join(" · ")}
              </title>
            )}
          </circle>
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
        </>
      }
      labelTop={19}
      label={
        <>
          <text y={28} fontSize={10} fill={ink.text}>
            {data.label}
          </text>
          <text y={40} fontSize={9} fill={ink.muted}>
            {data.ratingMVA > 0 ? `${data.ratingMVA} MVA` : "Unrated"}
          </text>
          <text y={52} fontSize={9} fill={ink.muted}>
            {data.primaryKV && data.secondaryKV
              ? `${data.primaryKV}/${data.secondaryKV} kV`
              : `${data.primaryKV || data.secondaryKV} kV winding`}
          </text>
          {flow && !marks && (
            <text y={68} fontSize={9} fill={overload ? ink.alert(overload) : ink.result} fontWeight={overload ? 600 : 400}>
              <tspan>{mw(Math.abs(flow.p))}</tspan>
              {results?.ac && (
                <tspan x={0} dy={11}>
                  {mvar(flow.q)}
                </tspan>
              )}
              {row?.loading_pct != null && (
                <tspan x={0} dy={11}>
                  {row.loading_pct.toFixed(0)}%
                </tspan>
              )}
            </text>
          )}
        </>
      }
    />
  );
}
