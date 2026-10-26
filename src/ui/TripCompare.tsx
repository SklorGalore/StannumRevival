// "Trip and compare": take one branch or generator out of the latest AC
// solve, solve again, and draw what changes on the diagram (the Contingency
// layer). Shown in the element inspector.
import { useState } from "react";
import { Zap } from "lucide-react";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import { useContingencies } from "../store/contingencyStore";
import { useLayers } from "../store/layerStore";
import { useStudy } from "../store/studyStore";
import { elementLabel, parseElement } from "../lib/elements";
import { fixed } from "../lib/analysis/format";

/** Whether an element can be tripped: a branch or generator, with a converged AC solve to start from. */
function useCanTrip(element: string): boolean {
  const report = useWorkspace((s) => s.solveReport);
  const kind = parseElement(element)?.kind;
  return !!report && report.converged && report.model === "ac" && (kind === "line" || kind === "transformer" || kind === "generator");
}

export function TripButton({ element, from }: { element: string; from: ModuleId }) {
  const can = useCanTrip(element);
  const tripping = useContingencies((s) => s.tripping);
  const trip = useContingencies((s) => s.trip);
  const drawAround = useWorkspace((s) => s.drawAround);
  const [error, setError] = useState<{ element: string; message: string } | null>(null);
  if (!can) return null;
  async function run() {
    setError(null);
    try {
      await trip(element);
      useLayers.getState().toggle("contingency", true);
      // On the diagram the drawing stays; elsewhere, draw around the element.
      if (from !== "diagram") drawAround(element, from);
    } catch (e) {
      setError({ element, message: (e as Error).message });
    }
  }
  return (
    <>
      <button
        type="button"
        disabled={tripping !== null}
        onClick={(e) => {
          e.stopPropagation();
          void run();
        }}
        className="shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap text-slate-600 hover:bg-slate-100 hover:text-red-700 disabled:opacity-50"
        title={`Take ${elementLabel(element)} out of the latest solve, solve again in AC, and draw what changes`}
        data-testid="trip-and-compare"
      >
        <Zap size={12} />
        {tripping === element ? "Tripping…" : "Trip and compare"}
      </button>
      {error?.element === element && <p className="basis-full px-1.5 text-[11px] text-red-700">{error.message}</p>}
    </>
  );
}

/** What tripping this element did, while it is drawn on the diagram. */
export function TripOutcome({ element, onPick }: { element: string; onPick: (element: string) => void }) {
  const detail = useContingencies((s) => s.detail);
  const onDiagram = useContingencies((s) => s.onDiagram);
  const open = useContingencies((s) => s.open);
  const setOnDiagram = useContingencies((s) => s.setOnDiagram);
  const limit = useContingencies((s) => s.choice.limitPct);
  const add = useStudy((s) => s.add);
  if (!detail || !onDiagram || open !== null || detail.row.element !== element) return null;
  const row = detail.row;
  const solved = row.converged ? `Solved in AC in ${row.iterations ?? 0} iteration${row.iterations === 1 ? "" : "s"}.` : "The AC solve did not converge.";
  return (
    <div className="mx-3 mb-2 rounded border border-red-200 bg-red-50/60 px-2.5 py-2 text-[11px] text-slate-700" data-testid="trip-outcome">
      <p className="font-semibold text-slate-800">If it trips</p>
      <p>{solved}</p>
      {row.note && <p className="text-amber-800">{row.note}</p>}
      <p className="mt-1">
        {detail.overloads.length === 0
          ? `No branch goes over ${fixed(limit, 0)}% of rate A, or gets worse.`
          : `${detail.overloads.length} branch${detail.overloads.length === 1 ? "" : "es"} over ${fixed(limit, 0)}%, new or worse:`}
      </p>
      {detail.overloads.length > 0 && (
        <ul className="mt-0.5">
          {detail.overloads.slice(0, 5).map((o) => (
            <li key={o.element} className="flex items-center gap-2">
              <button type="button" className="min-w-0 flex-1 truncate text-left text-teal-800 underline decoration-teal-300" onClick={() => onPick(o.element)}>
                {elementLabel(o.element)}
              </button>
              <span className="tabular-nums font-semibold text-red-700">{fixed(o.pct, 0)}%</span>
              <span className="tabular-nums text-slate-400">was {fixed(o.base_pct, 0)}</span>
            </li>
          ))}
        </ul>
      )}
      {detail.voltage_problems.length > 0 && <p className="mt-1">{detail.voltage_problems.length} buses outside their voltage limits.</p>}
      {detail.cut_off.length > 0 && (
        <p className="mt-1">
          Splits off {detail.cut_off.length} bus{detail.cut_off.length === 1 ? "" : "es"}
          {row.lost_load_mw > 0 ? `, losing ${fixed(row.lost_load_mw, 0)} MW of load` : ""}.
        </p>
      )}
      <p className="mt-1.5 flex gap-3">
        <button type="button" className="text-teal-800 underline" onClick={() => void add({ edit: { kind: "status", element, online: false } })}>
          Keep it out in the study
        </button>
        <button type="button" className="text-slate-500 underline" onClick={() => setOnDiagram(false)}>
          Clear
        </button>
      </p>
    </div>
  );
}
