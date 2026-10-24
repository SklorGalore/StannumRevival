import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { CircuitBreakerNode as CBNodeType } from "../../store/types";
import { useDiagramStore } from "../../store/diagramStore";

const W = 24;
const H = 48;
const BOX_SIZE = 20;
const BOX_X = (W - BOX_SIZE) / 2;
const BOX_Y = 14;

function boxFill(state: "open" | "closed" | "fault"): string {
  if (state === "closed") return "#1e293b";
  if (state === "fault") return "#dc2626";
  return "white";
}

export function CircuitBreakerNode({
  id,
  data,
  selected,
}: NodeProps<CBNodeType>) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);

  function handleClick() {
    const next = data.state === "closed" ? "open" : "closed";
    updateNodeData(id, { state: next });
  }

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block", cursor: "pointer" }}
        onClick={handleClick}
      >
        {/* Top stem */}
        <line x1={W / 2} y1={0} x2={W / 2} y2={BOX_Y} stroke="#1e293b" strokeWidth={2} />

        {/* Box */}
        <rect
          x={BOX_X}
          y={BOX_Y}
          width={BOX_SIZE}
          height={BOX_SIZE}
          fill={boxFill(data.state)}
          stroke={selected ? "#3b82f6" : "#1e293b"}
          strokeWidth={selected ? 2 : 1.5}
        />

        {/* Open state: X mark */}
        {data.state === "open" && (
          <>
            <line
              x1={BOX_X + 4}
              y1={BOX_Y + 4}
              x2={BOX_X + BOX_SIZE - 4}
              y2={BOX_Y + BOX_SIZE - 4}
              stroke="#1e293b"
              strokeWidth={1.5}
            />
            <line
              x1={BOX_X + BOX_SIZE - 4}
              y1={BOX_Y + 4}
              x2={BOX_X + 4}
              y2={BOX_Y + BOX_SIZE - 4}
              stroke="#1e293b"
              strokeWidth={1.5}
            />
          </>
        )}

        {/* Bottom stem */}
        <line
          x1={W / 2}
          y1={BOX_Y + BOX_SIZE}
          x2={W / 2}
          y2={H}
          stroke="#1e293b"
          strokeWidth={2}
        />

        {/* Label */}
        <text x={W + 4} y={28} fontSize={9} fill="#475569" fontFamily="monospace">
          {data.label}
        </text>
      </svg>

      <Handle
        type="target"
        position={Position.Top}
        id="cb-top"
        style={{ top: 0, left: W / 2 }}
      />
      <Handle
        type="source"
        position={Position.Bottom}
        id="cb-bottom"
        style={{ bottom: 0, left: W / 2 }}
      />
    </div>
  );
}
