import { AlertTriangle, FileCode, Info, Save, X } from "lucide-react";
import { fileName } from "../lib/study/recent";
import { useStudyFile } from "../store/studyFileStore";
import { useWorkspace } from "../store/workspaceStore";

/** In the toolbar: Save as run file, showing the run file once the work is saved to one. */
export function StudyFileButton({ narrow }: { narrow: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const path = useStudyFile((s) => s.path);
  const dirty = useStudyFile((s) => s.dirty);
  const busy = useStudyFile((s) => s.busy);
  const save = useStudyFile((s) => s.save);
  if (!summary) return null;
  const name = path ? fileName(path) : null;
  const label = busy === "saving" ? "Saving…" : name ? name : "Save as run file";
  return (
    <button
      type="button"
      onClick={() => void save()}
      disabled={!!busy}
      className="shrink-0 inline-flex max-w-[16rem] items-center gap-1.5 rounded-md px-2 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-100 hover:text-slate-800 disabled:opacity-50"
      title={
        name
          ? `${path}\n${dirty ? "Changed since it was saved. Click to save." : "Saved."}`
          : "Save a run file that opens this case, attaches its side files, sets the study edits and solves"
      }
      aria-label={name ? `Save run file ${name}` : "Save as run file"}
      data-testid="save-run-file"
    >
      {name ? <FileCode size={14} className="shrink-0 text-teal-700" /> : <Save size={14} className="shrink-0" />}
      {!narrow && <span className="truncate">{label}</span>}
      {name && dirty && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500" aria-label="unsaved changes" />}
    </button>
  );
}

/** Under the toolbar: what opening an old study found (a changed or moved case), or why saving failed. */
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
