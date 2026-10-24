import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { DisconnectorNode as DSNodeType } from "../../store/types";
import { useDiagramStore } from "../../store/diagramStore";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";

const W = 40;
const H = 60;
const CX = W / 2;
const TOP = 18;
const PIVOT = 42;

/** Disconnect switch: a blade hinged at the bottom. Double-click to operate. */
export function DisconnectorNode({ id, data, selected }: NodeProps<DSNodeType>) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const ink = useInk();
  const blade = selected ? SELECTED : ink.line;
  const closed = data.state === "closed";
  // Open: the blade swings 30° away from the upper contact.
  const end = closed ? { x: CX, y: TOP } : { x: CX + 12, y: TOP + 3 };

  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg
        width={W}
        height={H}
        overflow="visible"
        style={{ display: "block" }}
        onDoubleClick={() => updateNodeData(id, { state: closed ? "open" : "closed" })}
      >
        <title>Double-click to {closed ? "open" : "close"}</title>
        <line x1={CX} y1={0} x2={CX} y2={TOP} stroke={ink.line} strokeWidth={STROKE} />
        <line x1={CX - 5} y1={TOP} x2={CX + 5} y2={TOP} stroke={ink.line} strokeWidth={STROKE} />
        <line x1={CX} y1={PIVOT} x2={end.x} y2={end.y} stroke={blade} strokeWidth={STROKE} />
        <circle cx={CX} cy={PIVOT} r={2} fill={ink.line} />
        <line x1={CX} y1={PIVOT} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} />
        <text x={W + 2} y={H / 2 + 3} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
      </svg>
      <Handle type="target" position={Position.Top} id="ds-top" style={{ top: 0, left: CX }} />
      <Handle type="source" position={Position.Bottom} id="ds-bottom" style={{ bottom: 0, left: CX }} />
    </div>
  );
}
