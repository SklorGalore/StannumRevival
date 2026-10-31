// The log drawer's History page: every change and export, as run file lines,
// to copy or save. Pick lines to keep only some; with none picked, all go.
import { useLayoutEffect, useRef, useState, type MouseEvent } from "react";
import { Copy, FileCode2, Trash2 } from "lucide-react";
import { clock } from "../lib/log/log";
import { stepLine, useHistory } from "../store/historyStore";

export function HistoryPanel() {
  const entries = useHistory((s) => s.entries);
  const picked = useHistory((s) => s.picked);
  const error = useHistory((s) => s.error);
  const h = useHistory.getState();
  const [copied, setCopied] = useState(false);

  // Follow new steps, as the log does.
  const list = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = list.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [entries.length]);

  const copy = async () => {
    if (await h.copy()) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  };
  const some = picked.size > 0;
  const all = some && picked.size === entries.length;
  const which = some ? `${picked.size} picked` : "all";
  const tool = "inline-flex items-center gap-1 rounded px-2 py-1 text-[11px] text-slate-600 hover:bg-slate-100 disabled:opacity-50";

  return (
    <div className="flex flex-col flex-1 min-h-0" data-testid="history">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-slate-200 text-[11px] text-slate-600">
        <label className="flex items-center gap-1.5" title="Pick every line">
          <input
            type="checkbox"
            checked={all}
            ref={(el) => {
              if (el) el.indeterminate = some && !all;
            }}
            onChange={(e) => h.pickAll(e.target.checked)}
            disabled={!entries.length}
            aria-label="Pick every line"
          />
          {entries.length} step{entries.length === 1 ? "" : "s"}
          {some && <span className="text-teal-800">· {picked.size} picked</span>}
        </label>
        {error && <span className="truncate text-red-700">{error}</span>}
        <div className="ml-auto flex items-center gap-0.5">
          <button className={tool} onClick={() => void copy()} disabled={!entries.length} title={`Copy ${which} as run file text`}>
            <Copy size={12} /> {copied ? "Copied" : "Copy"}
          </button>
          <button className={tool} onClick={() => void h.save()} disabled={!entries.length} title={`Save ${which} as a run file`} data-testid="history-save">
            <FileCode2 size={12} /> Save as run file…
          </button>
          <button className={tool} onClick={h.clear} disabled={!entries.length} title="Empty the history. Nothing in the case changes.">
            <Trash2 size={12} /> Clear
          </button>
        </div>
      </div>
      <div ref={list} role="list" aria-label="History" className="flex-1 min-h-0 overflow-auto font-mono text-[11.5px] leading-5 py-1">
        {entries.map((e) => {
          const on = picked.has(e.id);
          const pick = (ev: MouseEvent) => {
            ev.preventDefault();
            h.toggle(e.id, ev.shiftKey);
          };
          return (
            <div
              key={e.id}
              role="listitem"
              onClick={pick}
              className={`flex items-start gap-3 px-3 cursor-default select-none ${on ? "bg-teal-50 text-teal-950" : "text-slate-700 hover:bg-slate-50"}`}
            >
              <input
                type="checkbox"
                checked={on}
                onClick={pick}
                onChange={() => {}}
                className="mt-1 shrink-0"
                aria-label={`Pick ${e.command} at ${clock(e.time).slice(0, 8)}`}
              />
              <span className="shrink-0 tabular-nums text-slate-500">{clock(e.time).slice(0, 8)}</span>
              <span className="min-w-0 break-all" title={stepLine(e)}>
                <span className="font-semibold">{e.command}</span>
                {stepLine(e).slice(e.command.length)}
              </span>
            </div>
          );
        })}
        {!entries.length && (
          <p className="px-3 py-2 font-sans text-xs text-slate-500">
            Nothing yet. Opening a case, solving, study edits, contingency and transfer runs and exports are kept here as you go, ready to copy
            or save as a run file.
          </p>
        )}
      </div>
    </div>
  );
}
