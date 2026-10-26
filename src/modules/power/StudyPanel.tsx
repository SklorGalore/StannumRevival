// The study tab: the edits every solve uses, each removable.
import { useState } from "react";
import { Bookmark, FlaskConical, RotateCcw, Trash2, X } from "lucide-react";
import { useStudy, type SavedSolve } from "../../store/studyStore";
import type { SolveReport } from "../../lib/analysis/types";
import { count, fixed } from "../../lib/analysis/format";
import { CompareSolves } from "./CompareSolves";
import { describeEdit } from "../../ui/StudyActions";
import { busElement } from "../../lib/elements";
import { Button, ErrorBanner, Panel } from "../../ui/primitives";
import { DrawAroundButton } from "../../ui/DrawAround";

function violationCount(r: SolveReport) {
  return r.limits.voltage_violations + r.limits.reactive_violations + r.limits.branch_overloads;
}

function SavedRow({ solve, onRestore }: { solve: SavedSolve; onRestore: () => void }) {
  const rename = useStudy((s) => s.renameSolve);
  const remove = useStudy((s) => s.deleteSolve);
  const [name, setName] = useState(solve.name);
  const r = solve.report;
  return (
    <li className="flex items-center gap-2 px-3 py-2 text-xs">
      <Bookmark size={13} className="text-teal-600 shrink-0" />
      <input
        aria-label="Solve name"
        className="flex-1 min-w-0 bg-transparent rounded px-1 py-0.5 text-slate-800 hover:bg-slate-50 focus:bg-white focus:outline-none focus:ring-2 focus:ring-teal-500/40"
        value={name}
        onChange={(e) => setName(e.target.value)}
        onBlur={() => name.trim() && rename(solve.id, name.trim())}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
      <span className="text-slate-500 tabular-nums whitespace-nowrap">
        {r.converged ? `${fixed(r.total_losses_mw, 2)} MW losses · ${count(violationCount(r))} violations` : r.status}
        {" · "}
        {solve.entries.length ? `${solve.entries.length} edit${solve.entries.length === 1 ? "" : "s"}` : "as filed"}
      </span>
      <button type="button" className="p-1 rounded text-slate-500 hover:bg-slate-100" onClick={onRestore} title="Go back to this solve's edits" aria-label={`Restore the edits of ${solve.name}`}>
        <RotateCcw size={13} />
      </button>
      <button type="button" className="p-1 rounded text-slate-400 hover:text-red-700 hover:bg-red-50" onClick={() => remove(solve.id)} aria-label={`Delete ${solve.name}`} title="Delete this saved solve">
        <X size={13} />
      </button>
    </li>
  );
}

export function StudyPanel({ onShow, report, stale }: { onShow: (element: string) => void; report: SolveReport | null; stale: boolean }) {
  const saved = useStudy((s) => s.saved);
  const saveSolve = useStudy((s) => s.saveSolve);
  const setEntries = useStudy((s) => s.setEntries);
  const entries = useStudy((s) => s.entries);
  const error = useStudy((s) => s.error);
  const busy = useStudy((s) => s.busy);
  const remove = useStudy((s) => s.remove);
  const clear = useStudy((s) => s.clear);
  return (
    <div className="p-4 flex flex-col gap-3 overflow-y-auto h-full [&>*]:shrink-0">
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Panel
        className="shrink-0"
        title="Edits to this case"
        eyebrow="Study"
        actions={
          entries.length > 1 && (
            <Button variant="ghost" onClick={() => void clear()} disabled={busy}>
              <Trash2 size={13} /> Clear all
            </Button>
          )
        }
      >
        {entries.length === 0 ? (
          <div className="p-4 text-xs text-slate-600 flex gap-3">
            <FlaskConical size={18} className="text-slate-400 shrink-0" />
            <div className="flex flex-col gap-1">
              <p className="font-medium text-slate-800">No edits yet. Every solve uses the case as filed.</p>
              <p>
                Select a branch or generator in a table, the diagram or the search, then use <b>Take out</b> to study an outage, or <b>Set output</b>{" "}
                and <b>Set load</b> to change a generator or a bus. Each edit re-solves the case, and the file on disk never changes.
              </p>
            </div>
          </div>
        ) : (
          <ol className="divide-y divide-slate-100" data-testid="study-edits">
            {entries.map((entry, i) => {
              const e = entry.edit;
              const element = e.kind === "load" ? busElement(e.bus) : e.element;
              return (
                <li key={i} className="flex items-center gap-2 px-3 py-2 text-xs">
                  <span className="w-5 text-right tabular-nums text-slate-400">{i + 1}</span>
                  <button type="button" className="flex-1 min-w-0 truncate text-left text-slate-800 hover:underline" onClick={() => onShow(element)}>
                    {describeEdit(entry)}
                  </button>
                  <DrawAroundButton element={element} from="power" compact />
                  <button
                    type="button"
                    className="p-1 rounded text-slate-400 hover:text-red-700 hover:bg-red-50 disabled:opacity-40"
                    onClick={() => void remove(i)}
                    disabled={busy}
                    aria-label={`Undo: ${describeEdit(entry)}`}
                    title="Undo this edit"
                  >
                    <X size={13} />
                  </button>
                </li>
              );
            })}
          </ol>
        )}
      </Panel>
      <Panel
        className="shrink-0"
        title="Saved solves"
        eyebrow="Compare"
        actions={
          <Button
            variant="secondary"
            disabled={!report || stale}
            onClick={() => {
              if (!report) return;
              const first = entries[0] ? describeEdit(entries[0]) : null;
              saveSolve(report, first ? (entries.length > 1 ? `${first} and ${entries.length - 1} more` : first) : "Case as filed");
            }}
            title={!report ? "Solve the case first" : stale ? "Solve again first: these results predate the latest edits" : "Keep this solve to compare against"}
          >
            <Bookmark size={13} /> Save this solve
          </Button>
        }
      >
        {saved.length === 0 ? (
          <p className="p-4 text-xs text-slate-600">
            Save a solve, change the study, and compare: for example, save the case as filed, take a line out, and see which flows and voltages moved.
          </p>
        ) : (
          <ul className="divide-y divide-slate-100" data-testid="saved-solves">
            {saved.map((solve) => (
              <SavedRow key={solve.id} solve={solve} onRestore={() => void setEntries(solve.entries)} />
            ))}
          </ul>
        )}
      </Panel>
      <CompareSolves current={stale ? null : report} onShow={onShow} />
      <p className="text-[11px] text-slate-500">
        Edits apply to power flow, sensitivities and MATPOWER export. Topology and Case diff show the case as filed.
      </p>
    </div>
  );
}
