import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { TransformerNode as TransformerNodeType } from "../../store/types";

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
        <line x1={cx} y1={cy - 8} x2={cx} y2={cy + 2} stroke="#94a3b8" strokeWidth={1} />
        <line x1={cx} y1={cy + 2} x2={cx - 6} y2={cy + 10} stroke="#94a3b8" strokeWidth={1} />
        <line x1={cx} y1={cy + 2} x2={cx + 6} y2={cy + 10} stroke="#94a3b8" strokeWidth={1} />
      </g>
    );
  }
  return (
    <polygon
      points={`${cx},${cy - 8} ${cx - 7},${cy + 5} ${cx + 7},${cy + 5}`}
      fill="none"
      stroke="#94a3b8"
      strokeWidth={1}
    />
  );
}

export function TransformerNode({
  data,
  selected,
}: NodeProps<TransformerNodeType>) {
  const primaryWinding = data.connection.startsWith("Wye") ? "Wye" : "Delta";
  const secondaryWinding = data.connection.endsWith("Wye") ? "Wye" : "Delta";

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
          stroke={selected ? "#3b82f6" : "#1e293b"}
          strokeWidth={2}
        />
        <WindingSymbol cx={CX} cy={32} type={primaryWinding} />

        {/* Secondary winding circle */}
        <circle
          cx={CX}
          cy={72}
          r={R}
          fill="white"
          stroke={selected ? "#3b82f6" : "#1e293b"}
          strokeWidth={2}
        />
        <WindingSymbol cx={CX} cy={72} type={secondaryWinding} />

        {/* Bottom stem */}
        <line x1={CX} y1={92} x2={CX} y2={104} stroke="#1e293b" strokeWidth={2} />

        {/* Ground symbol for grounded wye secondary */}
        {data.groundingSecondary === "solidly" && secondaryWinding === "Wye" && (
          <>
            <line x1={CX - 8} y1={96} x2={CX + 8} y2={96} stroke="#1e293b" strokeWidth={2} />
            <line x1={CX - 5} y1={100} x2={CX + 5} y2={100} stroke="#1e293b" strokeWidth={1.5} />
            <line x1={CX - 2} y1={104} x2={CX + 2} y2={104} stroke="#1e293b" strokeWidth={1} />
          </>
        )}

        {/* MVA / kV labels */}
        <text x={W + 4} y={28} fontSize={9} fill="#475569" fontFamily="monospace">
          {data.label}
        </text>
        <text x={W + 4} y={40} fontSize={9} fill="#64748b" fontFamily="monospace">
          {data.ratingMVA}MVA
        </text>
        <text x={W + 4} y={52} fontSize={9} fill="#64748b" fontFamily="monospace">
          {data.primaryKV}/{data.secondaryKV}kV
        </text>
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
