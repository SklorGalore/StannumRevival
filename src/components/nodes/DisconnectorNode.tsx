import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { DisconnectorNode as DSNodeType } from "../../store/types";
import { useDiagramStore } from "../../store/diagramStore";

const W = 24;
const H = 48;
const PIVOT_X = W / 2;
const PIVOT_Y = 32;
const TERM_Y = 12;

export function DisconnectorNode({
  id,
  data,
  selected,
}: NodeProps<DSNodeType>) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);

  function handleClick() {
    updateNodeData(id, { state: data.state === "closed" ? "open" : "closed" });
  }

  // Blade endpoints
  const bladeEndX = data.state === "closed" ? PIVOT_X : PIVOT_X + 10;
  const bladeEndY = data.state === "closed" ? TERM_Y : TERM_Y + 4;

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block", cursor: "pointer" }}
        onClick={handleClick}
      >
        {/* Top terminal dot */}
        <circle
          cx={PIVOT_X}
          cy={TERM_Y}
          r={3}
          fill={selected ? "#3b82f6" : "#1e293b"}
        />

        {/* Blade */}
        <line
          x1={PIVOT_X}
          y1={PIVOT_Y}
          x2={bladeEndX}
          y2={bladeEndY}
          stroke={selected ? "#3b82f6" : "#1e293b"}
          strokeWidth={2}
        />

        {/* Pivot dot */}
        <circle cx={PIVOT_X} cy={PIVOT_Y} r={2.5} fill="#1e293b" />

        {/* Bottom stem */}
        <line x1={W / 2} y1={PIVOT_Y} x2={W / 2} y2={H} stroke="#1e293b" strokeWidth={2} />

        {/* Top stem (to terminal) */}
        {data.state === "closed" && (
          <line x1={W / 2} y1={TERM_Y} x2={W / 2} y2={0} stroke="#1e293b" strokeWidth={2} />
        )}
        {data.state === "open" && (
          <line x1={W / 2} y1={TERM_Y} x2={W / 2} y2={0} stroke="#1e293b" strokeWidth={2} strokeDasharray="3,2" />
        )}

        {/* Label */}
        <text x={W + 4} y={28} fontSize={9} fill="#475569">
          {data.label}
        </text>
      </svg>

      <Handle
        type="target"
        position={Position.Top}
        id="ds-top"
        style={{ top: 0, left: W / 2 }}
      />
      <Handle
        type="source"
        position={Position.Bottom}
        id="ds-bottom"
        style={{ bottom: 0, left: W / 2 }}
      />
    </div>
  );
}
