import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { TransformerNode as TransformerNodeType } from "../../store/types";
import { branchFlow, loadingColor, mvar, mw, useResults, RESULT_TEXT } from "../../lib/diagram/results";

const W = 56;
const H = 104;
const CX = W / 2;
const R = 20;

function WindingSymbol({
  cx,
  cy,
  type,
}: {
  cx: number;
  cy: number;
  type: "Wye" | "Delta";
}) {
  if (type === "Wye") {
    return (
      <g>
        <line x1={cx} y1={cy - 8} x2={cx} y2={cy + 2} stroke="#475569" strokeWidth={1.25} />
        <line x1={cx} y1={cy + 2} x2={cx - 6} y2={cy + 10} stroke="#475569" strokeWidth={1.25} />
        <line x1={cx} y1={cy + 2} x2={cx + 6} y2={cy + 10} stroke="#475569" strokeWidth={1.25} />
      </g>
    );
  }
  return (
    <polygon
      points={`${cx},${cy - 8} ${cx - 7},${cy + 5} ${cx + 7},${cy + 5}`}
      fill="none"
      stroke="#475569"
      strokeWidth={1.25}
    />
  );
}

export function TransformerNode({
  data,
  selected,
}: NodeProps<TransformerNodeType>) {
  const primaryWinding = data.connection.startsWith("Wye") ? "Wye" : "Delta";
  const secondaryWinding = data.connection.endsWith("Wye") ? "Wye" : "Delta";
  const results = useResults();
  const row = data.caseRef?.kind === "branch" ? results?.branches.get(data.caseRef.id) : undefined;
  const flow = row ? branchFlow(row, data.reversed) : null;
  const overload = loadingColor(row?.loading_pct);
  const outline = selected ? "#3b82f6" : data.outOfService ? "#94a3b8" : (overload ?? "#1e293b");

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block" }}
      >
        {/* Top stem */}
        <line x1={CX} y1={0} x2={CX} y2={12} stroke="#1e293b" strokeWidth={2} />

        {/* Primary winding circle */}
        <circle
          cx={CX}
          cy={32}
          r={R}
          fill="white"
          stroke={outline}
          strokeWidth={2}
          strokeDasharray={data.outOfService ? "4 3" : undefined}
        />
        {!data.windingsUnknown && <WindingSymbol cx={CX} cy={32} type={primaryWinding} />}

        {/* Secondary winding circle */}
        <circle
          cx={CX}
          cy={72}
          r={R}
          fill="white"
          stroke={outline}
          strokeWidth={2}
          strokeDasharray={data.outOfService ? "4 3" : undefined}
        />
        {!data.windingsUnknown && <WindingSymbol cx={CX} cy={72} type={secondaryWinding} />}

        {/* Bottom stem */}
        <line x1={CX} y1={92} x2={CX} y2={104} stroke="#1e293b" strokeWidth={2} />

        {/* Ground symbol for grounded wye secondary */}
        {!data.windingsUnknown && data.groundingSecondary === "solidly" && secondaryWinding === "Wye" && (
          <>
            <line x1={CX - 8} y1={96} x2={CX + 8} y2={96} stroke="#1e293b" strokeWidth={2} />
            <line x1={CX - 5} y1={100} x2={CX + 5} y2={100} stroke="#1e293b" strokeWidth={1.5} />
            <line x1={CX - 2} y1={104} x2={CX + 2} y2={104} stroke="#1e293b" strokeWidth={1} />
          </>
        )}

        {/* MVA / kV labels */}
        <text x={W + 4} y={28} fontSize={9} fill="#475569">
          {data.label}
        </text>
        <text x={W + 4} y={40} fontSize={9} fill="#64748b">
          {data.ratingMVA > 0 ? `${data.ratingMVA} MVA` : "Unrated"}
        </text>
        <text x={W + 4} y={52} fontSize={9} fill="#64748b">
          {data.primaryKV && data.secondaryKV
            ? `${data.primaryKV}/${data.secondaryKV} kV`
            : `${data.primaryKV || data.secondaryKV} kV winding`}
        </text>
        {flow && (
          <text x={W + 4} y={68} fontSize={9} fill={overload ?? RESULT_TEXT} fontWeight={overload ? 600 : 400}>
            <tspan>{mw(Math.abs(flow.p))}</tspan>
            {results?.ac && (
              <tspan x={W + 4} dy={11}>
                {mvar(flow.q)}
              </tspan>
            )}
            {row?.loading_pct != null && (
              <tspan x={W + 4} dy={11}>
                {row.loading_pct.toFixed(0)}%
              </tspan>
            )}
          </text>
        )}
      </svg>

      <Handle
        type="target"
        position={Position.Top}
        id="xfmr-primary"
        style={{ top: 0, left: CX }}
      />
      <Handle
        type="source"
        position={Position.Bottom}
        id="xfmr-secondary"
        style={{ bottom: 0, left: CX }}
      />
    </div>
  );
}
