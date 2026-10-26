import type { NodeProps } from "@xyflow/react";
import type { FuseNode as FuseNodeType } from "../../store/types";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 60;
const CX = W / 2;
const BODY_W = 12;
const BODY_H = 24;
const BODY_Y = (H - BODY_H) / 2;
const TERMINALS: Terminal[] = [
  { id: "fuse-top", type: "target", x: CX, y: 0, side: "top" },
  { id: "fuse-bottom", type: "source", x: CX, y: H, side: "bottom" },
];

/** Fuse: a rectangle with the conductor running through it. */
export function FuseNode({ data, selected }: NodeProps<FuseNodeType>) {
  const ink = useInk();
  const blown = data.blown ? ink.alert("#dc2626") : null;
  const stroke = selected ? ink.selected : (blown ?? ink.line);

  return (
    <SymbolFrame
      w={W}
      h={H}
      rotation={data.rotation}
      labelOffset={data.labelOffset}
      terminals={TERMINALS}
      graphic={
        <>
          <line
            x1={CX}
            y1={0}
            x2={CX}
            y2={H}
            stroke={ink.line}
            strokeWidth={STROKE}
            strokeDasharray={blown ? `${BODY_Y} ${BODY_H}` : undefined}
          />
          <rect
            x={CX - BODY_W / 2}
            y={BODY_Y}
            width={BODY_W}
            height={BODY_H}
            fill={blown ? "white" : "none"}
            stroke={stroke}
            strokeWidth={STROKE}
          />
          {blown && (
            <path
              d={`M${CX - 4},${BODY_Y + 8} L${CX + 4},${BODY_Y + 16} M${CX + 4},${BODY_Y + 8} L${CX - 4},${BODY_Y + 16}`}
              stroke={blown}
              strokeWidth={STROKE}
            />
          )}
        </>
      }
      labelTop={H / 2 - 6}
      label={
        <text y={H / 2 + 3} fontSize={10} fill={ink.text}>
          {data.label}
        </text>
      }
    />
  );
}
