// The log: a one-line strip under every module that says what the app is
// doing, opening into a drawer with the full record (Ctrl+`).
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { AlertTriangle, ChevronDown, ChevronUp, Copy, Download, Loader2, OctagonX, Terminal, Trash2 } from "lucide-react";
import { startLogStream, useLog } from "../store/logStore";
import { useWorkspace } from "../store/workspaceStore";
import { asText, clock, duration, linkify, shown, type LevelFilter, type LogEntry } from "../lib/log/log";
import { saveTextExport } from "../lib/analysis/api";
import { SearchBox } from "../ui/primitives";

const HEIGHT_KEY = "stannum-log-height";
/** Lines drawn at once; the rest stay in the log and in Copy and Save. */
const DRAWN = 2000;
/** Quick calls don't flash in the strip. */
const SHOW_AFTER_MS = 250;

function loadHeight(): number {
  try {
    const h = Number(localStorage.getItem(HEIGHT_KEY));
    return h >= 100 ? h : 240;
  } catch {
    return 240;
  }
}

function saveHeight(h: number) {
  try {
    localStorage.setItem(HEIGHT_KEY, String(Math.round(h)));
  } catch {
    // Storage may be unavailable; the height still holds for this session.
  }
}

/** Re-render every `ms` while `on`, for elapsed-time counters. */
function useTick(on: boolean, ms = 200) {
  const [, setNow] = useState(0);
  useEffect(() => {
    if (!on) return;
    const id = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(id);
  }, [on, ms]);
}

export function LogDock() {
  const open = useLog((s) => s.open);
  const setOpen = useLog((s) => s.setOpen);
  const [height, setHeight] = useState(loadHeight);

  useEffect(() => startLogStream(), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.code === "Backquote") {
        e.preventDefault();
        const s = useLog.getState();
        s.setOpen(!s.open);
      }
    };
    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, []);

  const startResize = (e: ReactPointerEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startH = height;
    const max = window.innerHeight * 0.7;
    let h = startH;
    const move = (ev: PointerEvent) => {
      h = Math.min(max, Math.max(100, startH + startY - ev.clientY));
      setHeight(h);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      saveHeight(h);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return (
    <div className="shrink-0 flex flex-col border-t border-slate-200 bg-white" data-testid="log-dock">
      {open && (
        <>
          <div
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize log"
            onPointerDown={startResize}
            className="h-1 -mt-0.5 cursor-row-resize hover:bg-teal-500/40"
          />
          <LogDrawer height={height} onClose={() => setOpen(false)} />
        </>
      )}
      <LogStrip open={open} onToggle={() => setOpen(!open)} />
    </div>
  );
}

const SHORTCUT = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘`" : "Ctrl `";

/** Always visible: what is running now, or the latest line. */
function LogStrip({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  const running = useLog((s) => s.running);
  const latest = useLog((s) => {
    for (let i = s.entries.length - 1; i >= 0; i--) if (s.entries[i].level !== "debug") return s.entries[i];
    return null;
  });
  const unseen = useLog((s) => s.unseen);
  useTick(running.length > 0);
  const now = Date.now();
  const busy = running.find((a) => now - a.started >= SHOW_AFTER_MS);

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={open}
      aria-label={open ? "Hide log" : "Show log"}
      title={`${open ? "Hide" : "Show"} the log (${SHORTCUT})`}
      className="h-7 w-full flex items-center gap-2 px-3 text-[11px] text-slate-500 hover:bg-slate-50 text-left"
    >
      <Terminal size={12} className="shrink-0 text-slate-400" />
      <span className="font-medium text-slate-600 shrink-0">Log</span>
      <span className="text-slate-300 shrink-0">|</span>
      <span className="min-w-0 flex-1 truncate" data-testid="log-status" aria-live="polite">
        {busy ? (
          <span className="inline-flex items-center gap-1.5 text-teal-800">
            <Loader2 size={12} className="animate-spin shrink-0" />
            {busy.label}… <span className="tabular-nums text-slate-500">{duration(now - busy.started)}</span>
            {running.length > 1 && <span className="text-slate-400">· {running.length - 1} more</span>}
          </span>
        ) : latest ? (
          <span className={latest.level === "error" ? "text-red-700" : latest.level === "warn" ? "text-amber-700" : ""}>
            <span className="tabular-nums text-slate-400">{clock(latest.time).slice(0, 8)}</span> {latest.message}
          </span>
        ) : (
          "Ready"
        )}
      </span>
      {unseen.error > 0 && (
        <span className="inline-flex items-center gap-1 text-red-700 shrink-0" title="Errors since you last looked">
          <OctagonX size={12} /> {unseen.error}
        </span>
      )}
      {unseen.warn > 0 && (
        <span className="inline-flex items-center gap-1 text-amber-700 shrink-0" title="Warnings since you last looked">
          <AlertTriangle size={12} /> {unseen.warn}
        </span>
      )}
      <kbd className="rounded border border-slate-200 px-1 text-[10px] text-slate-400 shrink-0">{SHORTCUT}</kbd>
      {open ? <ChevronDown size={13} className="shrink-0" /> : <ChevronUp size={13} className="shrink-0" />}
    </button>
  );
}

const FILTERS: { id: LevelFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "warn", label: "Warnings" },
  { id: "error", label: "Errors" },
];

function LogDrawer({ height, onClose }: { height: number; onClose: () => void }) {
  const entries = useLog((s) => s.entries);
  const clear = useLog((s) => s.clear);
  const [filter, setFilter] = useState<LevelFilter>("all");
  const [details, setDetails] = useState(false);
  const [query, setQuery] = useState("");
  const [copied, setCopied] = useState(false);

  const visible = useMemo(() => entries.filter((e) => shown(e, filter, details, query)), [entries, filter, details, query]);
  const counts = useMemo(() => {
    let warn = 0;
    let error = 0;
    for (const e of entries) {
      if (e.level === "warn") warn++;
      else if (e.level === "error") error++;
    }
    return { all: entries.filter((e) => details || e.level !== "debug").length, warn, error };
  }, [entries, details]);
  const drawn = visible.length > DRAWN ? visible.slice(visible.length - DRAWN) : visible;

  // Follow new lines unless the reader has scrolled up to look at older ones.
  const list = useRef<HTMLDivElement>(null);
  const [follow, setFollow] = useState(true);
  useLayoutEffect(() => {
    const el = list.current;
    if (el && follow) el.scrollTop = el.scrollHeight;
  }, [drawn, follow, height]);
  const onScroll = () => {
    const el = list.current;
    if (el) setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(asText(visible));
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      useLog.getState().add("warn", "Couldn't copy the log to the clipboard.");
    }
  };
  const save = async () => {
    const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
    try {
      await saveTextExport(`stannum-${stamp}.log`, asText(visible), { name: "Log", extensions: ["log", "txt"] });
    } catch {
      // The failure is already in the log.
    }
  };

  const tool = "inline-flex items-center gap-1 rounded px-2 py-1 text-[11px] text-slate-600 hover:bg-slate-100";
  return (
    <section aria-label="Log" className="flex flex-col min-h-0" style={{ height }}>
      <header className="flex items-center gap-2 px-3 py-1.5 border-b border-slate-200">
        <div role="radiogroup" aria-label="Show" className="flex rounded-md border border-slate-300 overflow-hidden">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              role="radio"
              aria-checked={filter === f.id}
              onClick={() => setFilter(f.id)}
              className={`px-2.5 py-1 text-[11px] font-medium border-l first:border-l-0 border-slate-300 ${
                filter === f.id ? "bg-teal-700 text-white" : "bg-white text-slate-600 hover:bg-slate-50"
              }`}
            >
              {f.label} <span className="tabular-nums opacity-70">{counts[f.id]}</span>
            </button>
          ))}
        </div>
        <label className="flex items-center gap-1.5 text-[11px] text-slate-600" title="Include solver iterations and other detail lines">
          <input type="checkbox" checked={details} onChange={(e) => setDetails(e.target.checked)} />
          Details
        </label>
        <SearchBox value={query} onChange={setQuery} placeholder="Search the log" />
        <div className="ml-auto flex items-center gap-0.5">
          <button className={tool} onClick={copy} disabled={!visible.length}>
            <Copy size={12} /> {copied ? "Copied" : "Copy"}
          </button>
          <button className={tool} onClick={save} disabled={!visible.length}>
            <Download size={12} /> Save…
          </button>
          <button className={tool} onClick={clear} disabled={!entries.length}>
            <Trash2 size={12} /> Clear
          </button>
          <button className={tool} onClick={onClose} aria-label="Close log">
            <ChevronDown size={13} />
          </button>
        </div>
      </header>
      <div className="relative flex-1 min-h-0">
        <div
          ref={list}
          onScroll={onScroll}
          role="log"
          aria-label="Log lines"
          className="absolute inset-0 overflow-auto font-mono text-[11.5px] leading-5 py-1"
        >
          {visible.length > DRAWN && (
            <p className="px-3 text-slate-400">
              Showing the last {DRAWN.toLocaleString()} of {visible.length.toLocaleString()} lines. Save keeps them all.
            </p>
          )}
          {drawn.map((e) => (
            <LogLine key={e.key} entry={e} />
          ))}
          {!drawn.length && (
            <p className="px-3 py-2 font-sans text-xs text-slate-500">
              {entries.length
                ? "No lines match."
                : "Nothing logged yet. Opening a case, solving, comparing and drawing are recorded here as they happen."}
            </p>
          )}
        </div>
        {!follow && (
          <button
            onClick={() => setFollow(true)}
            className="absolute bottom-2 right-4 rounded-full bg-teal-700 text-white text-[11px] px-3 py-1 shadow"
          >
            Jump to latest
          </button>
        )}
      </div>
    </section>
  );
}

const LEVEL_STYLE: Record<LogEntry["level"], string> = {
  error: "bg-red-50 text-red-800",
  warn: "bg-amber-50/60 text-amber-800",
  info: "text-slate-700",
  debug: "text-slate-500",
};

function LogLine({ entry }: { entry: LogEntry }) {
  const hasCase = useWorkspace((s) => !!s.caseSummary);
  const focus = useWorkspace((s) => s.focus);
  return (
    <div className={`flex gap-3 px-3 ${LEVEL_STYLE[entry.level]}`} data-level={entry.level}>
      <span className="shrink-0 tabular-nums text-slate-400">{clock(entry.time)}</span>
      <span className="shrink-0 w-14 truncate text-slate-400" title={entry.source}>
        {entry.source}
      </span>
      <span className="min-w-0 whitespace-pre-wrap break-words">
        {entry.level === "error" && <OctagonX size={11} className="inline mr-1 -mt-0.5" />}
        {entry.level === "warn" && <AlertTriangle size={11} className="inline mr-1 -mt-0.5" />}
        {linkify(entry.message).map((s, i) =>
          "element" in s && hasCase ? (
            <button
              key={i}
              onClick={() => focus(s.element)}
              className="underline decoration-dotted underline-offset-2 hover:text-teal-700"
              title={`Show bus ${s.text} in every module`}
            >
              {s.text}
            </button>
          ) : (
            <span key={i}>{s.text}</span>
          )
        )}
      </span>
    </div>
  );
}
