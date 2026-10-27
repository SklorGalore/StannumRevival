import { AlertTriangle, FlaskConical, Info, Save, X } from "lucide-react";
import { fileName } from "../lib/study/recent";
import { useStudyFile } from "../store/studyFileStore";
import { useWorkspace } from "../store/workspaceStore";

/** In the case bar: the study file the case belongs to, and Save study. */
export function StudyFileButton({ narrow }: { narrow: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const path = useStudyFile((s) => s.path);
  const dirty = useStudyFile((s) => s.dirty);
  const busy = useStudyFile((s) => s.busy);
  const save = useStudyFile((s) => s.save);
  if (!summary) return null;
  const name = path ? fileName(path) : null;
  const label = busy === "saving" ? "Saving…" : name ? name : "Save study";
  return (
    <button
      type="button"
      onClick={() => void save()}
      disabled={!!busy}
      className="inline-flex max-w-[16rem] items-center gap-1.5 rounded-md border border-slate-300 px-2.5 py-1.5 text-xs text-slate-600 hover:bg-slate-50 hover:text-slate-800 disabled:opacity-50"
      title={
        name
          ? `${path}\n${dirty ? "Changed since it was saved. Click to save." : "Saved."}`
          : "Save the study edits, saved solves, settings and drawing to a .stannum file, with a link to this case"
      }
      aria-label={name ? `Save study ${name}` : "Save study"}
      data-testid="save-study"
    >
      {name ? <FlaskConical size={13} className="shrink-0 text-teal-700" /> : <Save size={13} className="shrink-0" />}
      {!narrow && <span className="truncate">{label}</span>}
      {name && dirty && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500" aria-label="unsaved changes" />}
    </button>
  );
}

/** Under the case bar: what opening a study found (a changed or moved case), or why saving failed. */
export function StudyNoticeBar() {
  const notice = useStudyFile((s) => s.notice);
  const error = useStudyFile((s) => s.error);
  const dismiss = () => useStudyFile.setState({ notice: null, error: null });
  if (!notice && !error) return null;
  const warn = !!error || notice?.tone === "warning";
  return (
    <div
      role={warn ? "alert" : "status"}
      data-testid="study-notice"
      className={`shrink-0 flex items-start gap-2.5 border-b px-4 py-2 text-xs ${
        warn ? "border-amber-200 bg-amber-50 text-amber-900" : "border-teal-100 bg-teal-50 text-teal-900"
      }`}
    >
      {warn ? <AlertTriangle size={14} className="mt-0.5 shrink-0" /> : <Info size={14} className="mt-0.5 shrink-0" />}
      <div className="min-w-0 flex-1">
        {error ? (
          <p>{error}</p>
        ) : (
          <>
            <p className="font-semibold">{notice!.title}</p>
            {notice!.lines.map((line, i) => (
              <p key={i} className="mt-0.5">
                {line}
              </p>
            ))}
          </>
        )}
      </div>
      <button type="button" onClick={dismiss} className="shrink-0 rounded p-0.5 hover:bg-black/5" aria-label="Dismiss" title="Dismiss">
        <X size={13} />
      </button>
    </div>
  );
}
