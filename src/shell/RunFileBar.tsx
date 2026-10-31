import { AlertTriangle, CheckCircle2, Loader2, Square, X } from "lucide-react";
import { fileName } from "../lib/study/recent";
import { describeRun, useRunFile } from "../store/runFileStore";

/** Under the case bar: a run file running, or how the last run went. */
export function RunFileBar() {
  const running = useRunFile((s) => s.running);
  const report = useRunFile((s) => s.report);
  const error = useRunFile((s) => s.error);
  const r = useRunFile.getState();

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
