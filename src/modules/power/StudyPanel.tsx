// The study tab: the edits every solve uses, each removable.
import { FlaskConical, Trash2, X } from "lucide-react";
import { useStudy } from "../../store/studyStore";
import { describeEdit } from "../../ui/StudyActions";
import { busElement } from "../../lib/elements";
import { Button, ErrorBanner, Panel } from "../../ui/primitives";
import { DrawAroundButton } from "../../ui/DrawAround";

export function StudyPanel({ onShow }: { onShow: (element: string) => void }) {
  const entries = useStudy((s) => s.entries);
  const error = useStudy((s) => s.error);
  const busy = useStudy((s) => s.busy);
  const remove = useStudy((s) => s.remove);
  const clear = useStudy((s) => s.clear);
  return (
    <div className="p-4 flex flex-col gap-3 overflow-y-auto h-full">
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Panel
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
      <p className="text-[11px] text-slate-500">
        Edits apply to power flow, sensitivities and MATPOWER export. Topology and Case diff show the case as filed.
      </p>
    </div>
  );
}
