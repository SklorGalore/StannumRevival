import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { BusNode as BusNodeType } from "../../store/types";
import { getVoltageColor } from "./shared/voltageColor";

export function BusNode({ data, selected }: NodeProps<BusNodeType>) {
  const width = data.length ?? 200;
  const color = getVoltageColor(data.voltageKV);
  const BUS_H = 8;
  const SVG_H = 44;

  return (
    <div className="node-wrapper relative" style={{ width }}>
      <svg
        width={width}
        height={SVG_H}
        overflow="visible"
        style={{ display: "block" }}
      >
        <rect
          x={0}
          y={18}
          width={width}
          height={BUS_H}
          fill={color}
          stroke={selected ? "#3b82f6" : "#1e293b"}
          strokeWidth={selected ? 2 : 1}
          rx={1}
        />
        <text
          x={width / 2}
          y={12}
          textAnchor="middle"
          fontSize={11}
          fill="#475569"
          fontFamily="monospace"
        >
          {data.label}
        </text>
        <text
          x={width / 2}
          y={38}
          textAnchor="middle"
          fontSize={10}
          fill={color}
          fontFamily="monospace"
        >
          {data.voltageKV} kV
        </text>
      </svg>

      {/* Left end handle */}
      <Handle
        type="source"
        position={Position.Left}
        id="bus-left"
        style={{ top: 22, left: 0 }}
      />
      {/* Right end handle */}
      <Handle
        type="source"
        position={Position.Right}
        id="bus-right"
        style={{ top: 22, right: 0 }}
      />
      {/* Top handle */}
      <Handle
        type="target"
        position={Position.Top}
        id="bus-top"
        style={{ top: 0, left: width / 2 }}
      />
      {/* Bottom tap handles */}
      {[0.2, 0.4, 0.6, 0.8].map((frac, i) => (
        <Handle
          key={i}
          type="source"
          position={Position.Bottom}
          id={`bus-tap-${i}`}
          style={{ bottom: 0, left: width * frac }}
        />
      ))}
    </div>
  );
}
