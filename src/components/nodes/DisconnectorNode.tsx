import type { NodeProps } from "@xyflow/react";
import type { DisconnectorNode as DSNodeType } from "../../store/types";
import { useDiagramStore } from "../../store/diagramStore";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";
import { SwitchLabel, useCaseSwitch } from "./shared/caseSwitch";

const W = 40;
const H = 60;
const CX = W / 2;
const TOP = 18;
const PIVOT = 42;
const TERMINALS: Terminal[] = [
  { id: "ds-top", type: "target", x: CX, y: 0, side: "top" },
  { id: "ds-bottom", type: "source", x: CX, y: H, side: "bottom" },
];

/** Disconnect switch: a blade hinged at the bottom. Double-click to operate;
 *  one of the case's is operated in the study. */
export function DisconnectorNode({ id, data, selected }: NodeProps<DSNodeType>) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const ink = useInk();
  const sw = useCaseSwitch(data);
  const blade = selected ? ink.selected : ink.line;
  const closed = sw.closed;
  // Open: the blade swings 30° away from the upper contact.
  const end = closed ? { x: CX, y: TOP } : { x: CX + 12, y: TOP + 3 };

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
          <line x1={CX} y1={0} x2={CX} y2={TOP} stroke={ink.line} strokeWidth={STROKE} />
          <line x1={CX - 5} y1={TOP} x2={CX + 5} y2={TOP} stroke={ink.line} strokeWidth={STROKE} />
          <line x1={CX} y1={PIVOT} x2={end.x} y2={end.y} stroke={blade} strokeWidth={STROKE} />
          <circle cx={CX} cy={PIVOT} r={2} fill={ink.line} />
          <line x1={CX} y1={PIVOT} x2={CX} y2={H} stroke={ink.line} strokeWidth={STROKE} />
        </>
      }
      labelTop={H / 2 - 6}
      label={<SwitchLabel data={data} sw={sw} y={H / 2 + 3} />}
    />
  );
}
