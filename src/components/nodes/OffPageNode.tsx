import type { NodeProps } from "@xyflow/react";
import type { OffPageNode as OffPageNodeType } from "../../store/types";
import { useInk, SELECTED, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 40;
const CX = W / 2;
const TERMINALS: Terminal[] = [{ id: "op-top", type: "source", x: CX, y: 0, side: "top" }];

/** A branch that continues to a bus outside the drawing. Its flow is shown
 *  on the wire leading to it. */
export function OffPageNode({ data, selected }: NodeProps<OffPageNodeType>) {
  const ink = useInk();
  const stroke = selected ? SELECTED : data.outOfService ? ink.off : ink.line;
  return (
    <SymbolFrame
      w={W}
      h={H}
      rotation={data.rotation}
      terminals={TERMINALS}
      title={`Double-click to bring ${data.busName || `Bus ${data.bus}`} into the drawing`}
      graphic={
        <>
          <line x1={CX} y1={0} x2={CX} y2={14} stroke={stroke} strokeWidth={STROKE} />
          {/* Off-page connector: a flag pointing away from the bus. */}
          <polygon
            points={`${CX - 8},14 ${CX + 8},14 ${CX + 8},30 ${CX},${H - 2} ${CX - 8},30`}
            fill="white"
            stroke={stroke}
            strokeWidth={STROKE}
            strokeLinejoin="round"
            strokeDasharray={data.outOfService ? "3 2" : undefined}
          />
        </>
      }
      labelTop={17}
      label={
        <text y={26} fontSize={9.5} fill={ink.text}>
          {data.label}
        </text>
      }
    />
  );
}
