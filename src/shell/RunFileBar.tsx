import { AlertTriangle, CheckCircle2, Circle, Loader2, Save, Square, Trash2, X } from "lucide-react";
import { fileName } from "../lib/study/recent";
import { describeRun, useRunFile } from "../store/runFileStore";

/** Under the case bar: Record in progress, a run file running, or how the last run went. */
export function RunFileBar() {
  const recording = useRunFile((s) => s.recording);
  const steps = useRunFile((s) => s.steps.length);
  const running = useRunFile((s) => s.running);
  const report = useRunFile((s) => s.report);
  const error = useRunFile((s) => s.error);
  const r = useRunFile.getState();

  if (recording)
    return (
      <div role="status" data-testid="recording-bar" className="shrink-0 flex items-center gap-2.5 border-b border-red-200 bg-red-50 px-4 py-1.5 text-xs text-red-900">
        <Circle size={10} className="shrink-0 fill-red-600 text-red-600 animate-pulse" aria-hidden="true" />
        <span className="font-semibold">Recording a run file</span>
        <span className="text-red-800/80">
          {steps} step{steps === 1 ? "" : "s"} so far. Work as usual: opening, solving, edits, contingencies and exports are kept.
        </span>
        {error && <span className="text-red-700">{error}</span>}
        <span className="ml-auto flex items-center gap-1.5">
          <button
            type="button"
            onClick={() => void r.saveRecording()}
            className="inline-flex items-center gap-1 rounded-md border border-red-300 bg-white px-2 py-0.5 font-medium hover:bg-red-100"
            data-testid="save-recording"
          >
            <Save size={12} /> Stop and save…
          </button>
          <button type="button" onClick={r.discardRecording} className="inline-flex items-center gap-1 rounded-md px-2 py-0.5 hover:bg-red-100" title="Stop recording without saving">
            <Trash2 size={12} /> Discard
          </button>
        </span>
      </div>
    );

  if (running)
    return (
      <div role="status" className="shrink-0 flex items-center gap-2.5 border-b border-teal-100 bg-teal-50 px-4 py-1.5 text-xs text-teal-900">
        <Loader2 size={13} className="shrink-0 animate-spin" aria-hidden="true" />
        <span>
          Running <span className="font-semibold">{fileName(running)}</span>… The log shows each line as it finishes.
        </span>
        <button type="button" onClick={r.cancel} className="ml-auto inline-flex items-center gap-1 rounded-md border border-teal-300 bg-white px-2 py-0.5 hover:bg-teal-100">
          <Square size={11} /> Cancel
        </button>
      </div>
    );

  if (!report && !error) return null;
  const notice = report ? describeRun(report) : { tone: "warning" as const, title: "The run file didn't run.", lines: [error!] };
  const warn = notice.tone === "warning";
  return (
    <div
      role={warn ? "alert" : "status"}
      data-testid="run-file-notice"
      className={`shrink-0 flex items-start gap-2.5 border-b px-4 py-2 text-xs ${
        warn ? "border-amber-200 bg-amber-50 text-amber-900" : "border-teal-100 bg-teal-50 text-teal-900"
      }`}
    >
      {warn ? <AlertTriangle size={14} className="mt-0.5 shrink-0" /> : <CheckCircle2 size={14} className="mt-0.5 shrink-0" />}
      <div className="min-w-0 flex-1">
        <p className="font-semibold">{notice.title}</p>
        {notice.lines.map((line, i) => (
          <p key={i} className="mt-0.5 break-words">
            {line}
          </p>
        ))}
      </div>
      <button type="button" onClick={r.dismiss} className="shrink-0 rounded p-0.5 hover:bg-black/5" aria-label="Dismiss" title="Dismiss">
        <X size={13} />
      </button>
    </div>
  );
}
