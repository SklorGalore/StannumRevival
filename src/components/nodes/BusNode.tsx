import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { BusNode as BusNodeType, BusTap } from "../../store/types";
import { getVoltageColor } from "./shared/voltageColor";
import { useResults, RESULT_TEXT } from "../../lib/diagram/results";

const BAR = 6;

/** Taps for hand-drawn buses: one on top, four below, one at each end. */
function defaultTaps(width: number): BusTap[] {
  return [
    { id: "bus-top", x: width / 2, side: "top" },
    ...[0.2, 0.4, 0.6, 0.8].map((f, i) => ({ id: `bus-tap-${i}`, x: width * f, side: "bottom" as const })),
  ];
}

export function BusNode({ data, selected }: NodeProps<BusNodeType>) {
  const width = data.length ?? 200;
  const color = getVoltageColor(data.voltageKV);
  const results = useResults();
  const row = data.caseRef ? results?.buses.get(data.caseRef.id) : undefined;
  const taps = data.taps ?? defaultTaps(width);

  if (data.star) {
    // Star point of a three-winding transformer.
    return (
      <div className="node-wrapper relative" style={{ width, height: BAR }}>
        <svg width={width} height={BAR} overflow="visible" style={{ display: "block" }}>
          <circle cx={width / 2} cy={BAR / 2} r={4} fill={selected ? "#3b82f6" : "#1e293b"} />
        </svg>
        {taps.map((t) => (
          <Handle
            key={t.id}
            type="source"
            id={t.id}
            position={t.side === "top" ? Position.Top : Position.Bottom}
            className="bus-tap"
            style={{ left: t.x }}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="node-wrapper relative" style={{ width, height: BAR }}>
      <svg width={width} height={BAR} overflow="visible" style={{ display: "block" }}>
        <rect x={0} y={0} width={width} height={BAR} fill={color} rx={1} />
        <text x={-8} y={0} textAnchor="end" fontSize={11} fontWeight={600} fill="#1e293b">
          {data.label}
        </text>
        <text x={-8} y={13} textAnchor="end" fontSize={9.5} fill="#64748b">
          {data.caseRef ? `${data.caseRef.id} · ` : ""}
          {data.voltageKV} kV
        </text>
        {row && (
          <text
            x={width + 8}
            y={BAR / 2 + 3.5}
            fontSize={9.5}
            fill={row.violation > 0 ? "#dc2626" : RESULT_TEXT}
            fontWeight={row.violation > 0 ? 600 : 400}
          >
            {row.vm.toFixed(3)} pu {row.va.toFixed(1)}°
          </text>
        )}
      </svg>

      {taps.map((t) => (
        <Handle
          key={t.id}
          type="source"
          id={t.id}
          position={t.side === "top" ? Position.Top : Position.Bottom}
          className="bus-tap"
          style={{ left: t.x }}
        />
      ))}
      <Handle type="source" position={Position.Left} id="bus-left" className="bus-tap" style={{ top: BAR / 2 }} />
      <Handle type="source" position={Position.Right} id="bus-right" className="bus-tap" style={{ top: BAR / 2 }} />
    </div>
  );
}
