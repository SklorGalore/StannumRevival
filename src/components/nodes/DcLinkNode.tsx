import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { DcLinkNode as DcLinkNodeType } from "../../store/types";
import { mw, useResults } from "../../lib/diagram/results";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";

const W = 40;
const H = 140;
const CX = W / 2;
/** Converter boxes: 32 px squares with the AC side at the outer end. */
const BOX = 32;
const TOP = 14;
const BOTTOM = H - 14 - BOX;
/** The dashed DC section between the converters. */
const DC_DASH = "6 4";

/** A converter: a box split by a diagonal, ~ on the AC side and = on the DC side. */
function Converter({ y, acOnTop, color }: { y: number; acOnTop: boolean; color: string }) {
  const x = CX - BOX / 2;
  const [acY, dcY] = acOnTop ? [y + 11, y + BOX - 8] : [y + BOX - 8, y + 11];
  const [acX, dcX] = acOnTop ? [x + 9, x + BOX - 9] : [x + BOX - 9, x + 9];
  return (
    <g stroke={color} strokeWidth={STROKE} fill="none">
      <rect x={x} y={y} width={BOX} height={BOX} fill="white" />
      {/* Bottom-left to top-right, so the ~ and = corners stay clear of it. */}
      <path d={`M${x},${y + BOX} L${x + BOX},${y}`} />
      <path d={`M${acX - 5},${acY} q2.5,-4 5,0 t5,0`} strokeWidth={1.2} />
      <path d={`M${dcX - 5},${dcY - 2} h10 M${dcX - 5},${dcY + 2} h10`} strokeWidth={1.2} />
    </g>
  );
}

/** A two-terminal DC line drawn as one symbol: rectifier, DC line, inverter.
 *  The arrow on the DC section points from the rectifier to the inverter. */
export function DcLinkNode({ data, selected }: NodeProps<DcLinkNodeType>) {
  const ink = useInk();
  const results = useResults();
  const color = selected ? SELECTED : data.outOfService ? ink.off : ink.line;
  const stem = (kv: number) => (data.outOfService ? ink.off : ink.voltage(kv));
  const dcTop = TOP + BOX;
  const mid = (dcTop + BOTTOM) / 2;
  // Arrow from the rectifier toward the inverter.
  const down = !data.reversed;
  const arrow = down ? `M${CX - 4},${mid - 3} L${CX},${mid + 3} L${CX + 4},${mid - 3}` : `M${CX - 4},${mid + 3} L${CX},${mid - 3} L${CX + 4},${mid + 3}`;
  const textX = W + 2;
  return (
    <div className="node-wrapper relative" style={{ width: W, height: H }}>
      <svg width={W} height={H} overflow="visible" style={{ display: "block" }}>
        <line x1={CX} y1={0} x2={CX} y2={TOP} stroke={stem(data.upperKV)} strokeWidth={STROKE} />
        <line x1={CX} y1={BOTTOM + BOX} x2={CX} y2={H} stroke={stem(data.lowerKV)} strokeWidth={STROKE} />
        <line x1={CX} y1={dcTop} x2={CX} y2={BOTTOM} stroke={color} strokeWidth={2} strokeDasharray={DC_DASH} />
        {!data.outOfService && <path d={arrow} fill="none" stroke={color} strokeWidth={1.4} />}
        <Converter y={TOP} acOnTop color={color} />
        <Converter y={BOTTOM} acOnTop={false} color={color} />
        <text x={textX} y={TOP + 12} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
        <text x={textX} y={TOP + 24} fontSize={9} fill={ink.muted}>
          {data.outOfService ? "Blocked" : "Two-terminal DC"}
        </text>
        {!data.outOfService && (
          <text x={textX} y={mid + 3} fontSize={9} fill={results ? ink.result : ink.muted}>
            {mw(data.scheduledMW)} {results ? "" : "sched."}
          </text>
        )}
      </svg>
      <Handle type="target" position={Position.Top} id="dc-top" style={{ top: 0, left: CX }} />
      <Handle type="source" position={Position.Bottom} id="dc-bottom" style={{ bottom: 0, left: CX }} />
    </div>
  );
}
