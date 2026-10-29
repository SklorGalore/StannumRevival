// A breaker or disconnector drawn for a RAW system switching device of the
// open case: where its state comes from, how double-click operates it, and
// the text beside it.
import type { CaseRef } from "../../../store/types";
import { useDiagramStore } from "../../../store/diagramStore";
import { useStudy } from "../../../store/studyStore";
import { useWorkspace } from "../../../store/workspaceStore";
import { branchFlow, mvar, mw, useResults } from "../../../lib/diagram/results";
import { useResultMarks } from "../../../lib/diagram/detail";
import { useInk } from "../../../lib/diagram/ink";

interface SwitchData {
  label: string;
  state: string;
  normallyOpen?: boolean;
  caseRef?: CaseRef;
  reversed?: boolean;
}

/**
 * Whether the switch is closed, and the flow through it. A case switch
 * shows the study's word on it first (so double-click answers at once),
 * then the shown result's, then the file's. A hand-drawn one keeps its own.
 */
export function useCaseSwitch(data: SwitchData) {
  const element = data.caseRef?.element;
  const results = useResults();
  const marks = useResultMarks();
  const caseName = useDiagramStore((s) => s.metadata.caseName);
  const openName = useWorkspace((s) => s.caseSummary?.name);
  const sameCase = !!caseName && caseName === openName;
  const edit = useStudy((s) =>
    element && sameCase ? s.entries.find((e) => e.edit.kind === "status" && e.edit.element === element) : undefined
  );
  const row = data.caseRef?.kind === "branch" ? results?.branches.get(data.caseRef.id) : undefined;
  const closed = edit?.edit.kind === "status" ? edit.edit.online : row ? row.online : data.state === "closed";
  const flow = row?.online ? branchFlow(row, data.reversed) : null;
  // Out of its normal position, as an operator would want to see.
  const abnormal = !!element && data.normallyOpen != null && closed === data.normallyOpen;
  /** Double-click: open or close it in the study, or flip a hand-drawn one. */
  const operate = (flip: () => void) => {
    if (!element) return flip();
    if (!sameCase) return;
    void useStudy.getState().add({ edit: { kind: "status", element, online: !closed } });
  };
  const title = element
    ? sameCase
      ? `Double-click to ${closed ? "open" : "close"} it in the study`
      : "From another case"
    : `Double-click to ${closed ? "open" : "close"}`;
  return { linked: !!element, closed, flow, ac: !!results?.ac, marks, abnormal, operate, title };
}

/** Name, state and flow, to the right of the symbol. */
export function SwitchLabel({ data, sw, y }: { data: SwitchData; sw: ReturnType<typeof useCaseSwitch>; y: number }) {
  const ink = useInk();
  if (!sw.linked)
    return (
      <text y={y} fontSize={10} fill={ink.text}>
        {data.label}
      </text>
    );
  const state = `${sw.closed ? "Closed" : "Open"}${sw.abnormal ? ` (normally ${sw.closed ? "open" : "closed"})` : ""}`;
  return (
    <>
      <text y={y - 6} fontSize={10} fill={ink.text}>
        {data.label}
      </text>
      <text y={y + 6} fontSize={9} fill={sw.abnormal ? ink.alert("#d97706") : ink.muted} fontWeight={sw.abnormal ? 600 : 400}>
        {state}
      </text>
      {sw.flow && !sw.marks && (
        <text y={y + 20} fontSize={9} fill={ink.result}>
          <tspan>{mw(Math.abs(sw.flow.p))}</tspan>
          {sw.ac && (
            <tspan x={0} dy={11}>
              {mvar(sw.flow.q)}
            </tspan>
          )}
        </text>
      )}
    </>
  );
}
