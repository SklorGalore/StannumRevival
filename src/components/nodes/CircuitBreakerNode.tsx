import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { CircuitBreakerNode as CBNodeType } from "../../store/types";
import { useDiagramStore } from "../../store/diagramStore";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";

const W = 40;
const H = 60;
const CX = W / 2;
const BOX = 20;
const BOX_Y = (H - BOX) / 2;

/** Breaker: a square, filled when closed. Double-click to open or close. */
export function CircuitBreakerNode({ id, data, selected }: NodeProps<CBNodeType>) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const ink = useInk();
  const stroke = selected ? SELECTED : ink.line;
  const fill = data.state === "closed" ? ink.line : data.state === "fault" ? ink.alert("#dc2626") : "white";

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block" }}
        onDoubleClick={() => updateNodeData(id, { state: data.state === "closed" ? "open" : "closed" })}
      >
        <title>Double-click to {data.state === "closed" ? "open" : "close"}</title>
        <line x1={CX} y1={0} x2={CX} y2={BOX_Y} stroke={ink.line} strokeWidth={STROKE} />
        <rect x={CX - BOX / 2} y={BOX_Y} width={BOX} height={BOX} fill={fill} stroke={stroke} strokeWidth={STROKE} />
        <line x1={CX} y1={BOX_Y + BOX} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} />
        <text x={W + 2} y={H / 2 + 3} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
      </svg>
      <Handle type="target" position={Position.Top} id="cb-top" style={{ top: 0, left: CX }} />
      <Handle type="source" position={Position.Bottom} id="cb-bottom" style={{ bottom: 0, left: CX }} />
    </div>
  );
}
