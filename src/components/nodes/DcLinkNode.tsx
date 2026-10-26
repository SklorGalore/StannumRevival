import type { NodeProps } from "@xyflow/react";
import type { DcLinkNode as DcLinkNodeType } from "../../store/types";
import { mw, useResults } from "../../lib/diagram/results";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 140;
const CX = W / 2;
/** Converter boxes: 32 px squares with the AC side at the outer end. */
const BOX = 32;
const TOP = 14;
const BOTTOM = H - 14 - BOX;
/** The dashed DC section between the converters. */
const DC_DASH = "6 4";
const TERMINALS: Terminal[] = [
  { id: "dc-top", type: "target", x: CX, y: 0, side: "top" },
  { id: "dc-bottom", type: "source", x: CX, y: H, side: "bottom" },
];

const KIND_TEXT = {
  two_terminal: "Two-terminal DC",
  vsc: "VSC DC",
  multi_terminal: "Multi-terminal DC",
} as const;

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

/** A DC line drawn as one symbol: rectifier, DC line, inverter. VSC lines and
 *  each converter of a multi-terminal system (paired with the converter that
 *  holds the DC voltage) use the same symbol with their own caption.
 *  The arrow on the DC section points from the rectifier to the inverter. */
export function DcLinkNode({ data, selected }: NodeProps<DcLinkNodeType>) {
  const ink = useInk();
  const results = useResults();
  const color = selected ? ink.selected : data.outOfService ? ink.off : ink.line;
  const stem = (kv: number) => (data.outOfService ? ink.off : ink.voltage(kv));
  const dcTop = TOP + BOX;
  const mid = (dcTop + BOTTOM) / 2;
  // Arrow from the rectifier toward the inverter.
  const down = !data.reversed;
  const arrow = down ? `M${CX - 4},${mid - 3} L${CX},${mid + 3} L${CX + 4},${mid - 3}` : `M${CX - 4},${mid + 3} L${CX},${mid - 3} L${CX + 4},${mid + 3}`;
  return (
    <SymbolFrame
      w={W}
      h={H}
      focus={data.focus}
      rotation={data.rotation}
      terminals={TERMINALS}
      graphic={
        <>
          <line x1={CX} y1={0} x2={CX} y2={TOP} stroke={stem(data.upperKV)} strokeWidth={STROKE} />
          <line x1={CX} y1={BOTTOM + BOX} x2={CX} y2={H} stroke={stem(data.lowerKV)} strokeWidth={STROKE} />
          <line x1={CX} y1={dcTop} x2={CX} y2={BOTTOM} stroke={color} strokeWidth={2} strokeDasharray={DC_DASH} />
          {!data.outOfService && <path d={arrow} fill="none" stroke={color} strokeWidth={1.4} />}
          <Converter y={TOP} acOnTop color={color} />
          <Converter y={BOTTOM} acOnTop={false} color={color} />
        </>
      }
      labelTop={TOP + 3}
      label={
        <>
          <text y={TOP + 12} fontSize={10} fill={ink.text}>
            {data.label}
          </text>
          <text y={TOP + 24} fontSize={9} fill={ink.muted}>
            {data.outOfService ? "Blocked" : KIND_TEXT[data.dcKind ?? "two_terminal"]}
          </text>
          {!data.outOfService && (
            <text y={TOP + 36} fontSize={9} fill={results ? ink.result : ink.muted}>
              {mw(data.scheduledMW)} {results ? "" : "sched."}
            </text>
          )}
        </>
      }
    />
  );
}
