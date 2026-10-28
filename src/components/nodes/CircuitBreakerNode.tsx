import type { NodeProps } from "@xyflow/react";
import type { CircuitBreakerNode as CBNodeType } from "../../store/types";
import { useDiagramStore } from "../../store/diagramStore";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";
import { SwitchLabel, useCaseSwitch } from "./shared/caseSwitch";

const W = 40;
const H = 60;
const CX = W / 2;
const BOX = 20;
const BOX_Y = (H - BOX) / 2;
const TERMINALS: Terminal[] = [
  { id: "cb-top", type: "target", x: CX, y: 0, side: "top" },
  { id: "cb-bottom", type: "source", x: CX, y: H, side: "bottom" },
];

/** Breaker: a square, filled when closed. Double-click to open or close;
 *  one of the case's is operated in the study. */
export function CircuitBreakerNode({ id, data, selected }: NodeProps<CBNodeType>) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const ink = useInk();
  const sw = useCaseSwitch(data);
  const stroke = selected ? ink.selected : ink.line;
  const fill = data.state === "fault" && !sw.linked ? ink.alert("#dc2626") : sw.closed ? ink.line : "white";

  return (
    <SymbolFrame
      element={data.caseRef?.element}
      focus={data.focus}
      w={W}
      h={H}
      rotation={data.rotation}
      labelOffset={data.labelOffset}
      terminals={TERMINALS}
      title={sw.title}
      onDoubleClick={() => sw.operate(() => updateNodeData(id, { state: data.state === "closed" ? "open" : "closed" }))}
      graphic={
        <>
          <line x1={CX} y1={0} x2={CX} y2={BOX_Y} stroke={ink.line} strokeWidth={STROKE} />
          <rect x={CX - BOX / 2} y={BOX_Y} width={BOX} height={BOX} fill={fill} stroke={stroke} strokeWidth={STROKE} />
          <line x1={CX} y1={BOX_Y + BOX} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} />
        </>
      }
      labelTop={H / 2 - 6}
      label={<SwitchLabel data={data} sw={sw} y={H / 2 + 3} />}
    />
  );
}
