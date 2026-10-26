import { useState } from "react";
import { Crosshair, TriangleAlert, Wand2 } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { useDiagramStore } from "../../store/diagramStore";
import type { SolveReport } from "../../lib/analysis/types";
import { busElement, elementLabel } from "../../lib/elements";
import { centreBus } from "../../ui/DrawAround";
import { GenerateDialog } from "../../components/dialogs/GenerateDialog";
import { Button } from "../../ui/primitives";

/** The worst finding of a solve to draw around: the most overloaded branch,
 *  else the bus furthest outside its voltage limits. */
export function worstFinding(report: SolveReport | null): { element: string; what: string } | null {
  const l = report?.limits;
  if (!report || !l) return null;
  if (l.worst_branch) {
    const [a, b, pct] = l.worst_branch;
    const row = report.branches
      .filter((r) => (r.from === a && r.to === b) || (r.from === b && r.to === a))
      .reduce<
        SolveReport["branches"][number] | null
      >((x, r) => (!x || (r.loading_pct ?? 0) > (x.loading_pct ?? 0) ? r : x), null);
    if (row) return { element: row.element, what: `${pct.toFixed(0)}% loaded` };
  }
  if (l.worst_voltage) {
    const [bus, by] = l.worst_voltage;
    return { element: busElement(bus), what: `${by.toFixed(3)} p.u. outside its limits` };
  }
  return null;
}

/**
 * With a case open and nothing drawn, offer to draw around what you are
 * looking at: the shared selection, or the worst violation of the last solve.
 */
export function DrawPrompt() {
  const summary = useWorkspace((s) => s.caseSummary);
  const selection = useWorkspace((s) => s.selection);
  const report = useWorkspace((s) => s.solveReport);
  const drawAround = useWorkspace((s) => s.drawAround);
  const empty = useDiagramStore((s) => s.nodes.length === 0);
  const [other, setOther] = useState(false);
  if (!summary?.power.available || !empty) return null;
  const picked = selection && centreBus(selection.element) !== null ? selection.element : null;
  const worst = worstFinding(report);
  return (
    <>
      <div className="absolute inset-0 z-10 flex items-center justify-center pointer-events-none [&_button]:pointer-events-auto">
        <div
          // Only the buttons take the pointer: symbols dragged from the palette
          // drop straight through onto the canvas.
          className="w-[22rem] max-w-[calc(100%-2rem)] rounded-lg border border-slate-200 bg-white p-4 shadow-sm text-xs text-slate-600 flex flex-col gap-3 whitespace-normal"
          data-testid="draw-prompt"
        >
          <div>
            <h2 className="text-sm font-semibold text-slate-800">Draw a one-line from {summary.name}</h2>
            <p className="mt-1">Drawings are made a few hops around the element you're studying.</p>
          </div>
          <div className="flex flex-col gap-1.5">
            {picked && (
              <Button variant="primary" className="justify-start" onClick={() => drawAround(picked, "diagram")}>
                <Crosshair size={13} /> Draw around {elementLabel(picked)}
              </Button>
            )}
            {worst && (
              <Button
                variant={picked ? "secondary" : "primary"}
                className="justify-start text-left"
                onClick={() => drawAround(worst.element, "diagram")}
                title="The worst violation of the last solve"
              >
                <TriangleAlert size={13} className="shrink-0" />
                <span>
                  Draw around the worst violation: {elementLabel(worst.element)}, {worst.what}
                </span>
              </Button>
            )}
            <Button
              variant={picked || worst ? "secondary" : "primary"}
              className="justify-start"
              onClick={() => setOther(true)}
            >
              <Wand2 size={13} /> Choose a bus, an area or the whole case…
            </Button>
          </div>
          {!worst && !report && (
            <p className="text-slate-500">Solve the case and this offers its worst violation too.</p>
          )}
          <p className="text-slate-500">
            To draw by hand instead, drag symbols from Components, on the left, onto the canvas.
          </p>
        </div>
      </div>
      {other && <GenerateDialog onClose={() => setOther(false)} />}
    </>
  );
}
