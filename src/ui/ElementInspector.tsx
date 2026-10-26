// One inspector for one element, shared by every module: what it is, its
// solved results, its limits and file data, the study's edits to it, and
// what you can do with it. Module-specific parts (Topology's structure, the
// diagram's drawing fields) slot in below.
import { useEffect, useState, type ReactNode } from "react";
import { ChevronRight, Play, X } from "lucide-react";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import { useStudy, editTarget } from "../store/studyStore";
import { api } from "../lib/analysis/api";
import type { DetailField, ElementDetail } from "../lib/analysis/types";
import { fixed } from "../lib/analysis/format";
import { busElement, elementLabel, parseElement } from "../lib/elements";
import { elementResults, type ResultRow, type Tone } from "../lib/inspect";
import { DrawAroundButton } from "./DrawAround";
import { StudyActions, describeEdit } from "./StudyActions";
import { TripButton, TripOutcome } from "./TripCompare";

const KIND_LABEL: Record<string, string> = {
  bus: "Bus",
  line: "Line",
  transformer: "Transformer",
  switch: "Switch",
  generator: "Generator",
  load: "Load",
  dcline: "DC line",
};

const TONE_TEXT: Record<Tone, string> = { ok: "text-slate-800", warn: "text-amber-700", bad: "text-red-700" };
const TONE_CHIP: Record<Tone, string> = {
  ok: "bg-slate-100 text-slate-700",
  warn: "bg-amber-100 text-amber-900",
  bad: "bg-red-100 text-red-800",
};

// File data per element, kept for the open case so picking back and forth
// doesn't ask the core again.
let cacheVersion = -1;
const cache = new Map<string, ElementDetail | string>();

function useElementDetail(element: string): { detail: ElementDetail | null; error: string | null } {
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const canRead = useWorkspace((s) => !!s.caseSummary?.power.available);
  if (cacheVersion !== caseVersion) {
    cacheVersion = caseVersion;
    cache.clear();
  }
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!canRead || cache.has(element)) return;
    let live = true;
    api
      .elementDetail(element)
      .then((d) => cache.set(element, d))
      .catch((e: Error) => cache.set(element, e.message))
      .finally(() => live && setTick((n) => n + 1));
    return () => {
      live = false;
    };
  }, [element, caseVersion, canRead]);
  const hit = cache.get(element);
  return { detail: typeof hit === "object" ? hit : null, error: typeof hit === "string" ? hit : null };
}

// Which sections are folded is remembered on this computer.
const FOLD_KEY = "stannum-inspector-folded";
function loadFolded(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(FOLD_KEY) ?? "{}") as Record<string, boolean>;
  } catch {
    return {};
  }
}

function Section({
  id,
  title,
  count,
  defaultOpen = true,
  children,
}: {
  id: string;
  title: string;
  count?: number;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(() => !(loadFolded()[id] ?? !defaultOpen));
  const toggle = () => {
    const next = !open;
    setOpen(next);
    try {
      localStorage.setItem(FOLD_KEY, JSON.stringify({ ...loadFolded(), [id]: !next }));
    } catch {
      // Storage may be unavailable; the section still folds for now.
    }
  };
  return (
    <section className="border-t border-slate-200" data-testid={`inspector-${id}`}>
      <button
        type="button"
        className="w-full flex items-center gap-1 px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-slate-500 hover:bg-slate-50"
        onClick={toggle}
        aria-expanded={open}
      >
        <ChevronRight size={12} className={`transition-transform ${open ? "rotate-90" : ""}`} />
        {title}
        {count !== undefined && <span className="ml-1 font-medium text-slate-400 normal-case tracking-normal">{count}</span>}
      </button>
      {open && <div className="px-3 pb-2.5">{children}</div>}
    </section>
  );
}

function fieldValue(f: DetailField): string {
  if (typeof f.value === "string") return f.value;
  if (Number.isInteger(f.value) && (f.unit === "" || Math.abs(f.value) >= 1000)) return String(f.value);
  // Per-unit values keep their precision without trailing zeros (0.017, 1.0735).
  return f.unit === "p.u." ? String(Number(f.value.toPrecision(6))) : fixed(f.value, Math.abs(f.value) >= 100 ? 1 : 2);
}

/** Label / value / unit rows, aligned on the value. */
function Rows({ rows }: { rows: { label: string; value: string; unit: string; tone?: Tone }[] }) {
  return (
    <dl className="grid grid-cols-[minmax(0,1fr)_auto_auto] gap-x-2 gap-y-0.5 text-xs">
      {rows.map((r, i) => (
        <div key={`${r.label}-${r.unit}-${i}`} className="contents">
          <dt className="text-slate-500 truncate" title={r.label}>
            {r.label}
          </dt>
          <dd className={`text-right font-mono tabular-nums ${TONE_TEXT[r.tone ?? "ok"]} ${r.tone && r.tone !== "ok" ? "font-semibold" : ""}`}>
            {r.value}
          </dd>
          <dd className="text-slate-400 text-[11px] whitespace-nowrap">{r.unit}</dd>
        </div>
      ))}
    </dl>
  );
}

function ElementButton({ element, onPick, children }: { element: string; onPick: (e: string) => void; children?: ReactNode }) {
  return (
    <button
      type="button"
      className="text-left text-teal-800 underline decoration-teal-300 underline-offset-2 hover:decoration-teal-700"
      onClick={() => onPick(element)}
      title={`Inspect ${elementLabel(element)}`}
    >
      {children ?? elementLabel(element)}
    </button>
  );
}

/** Whether the study changes the element's status, and the edits that touch it. */
function useStudyFor(element: string) {
  const entries = useStudy((s) => s.entries);
  const parsed = parseElement(element);
  const bus = parsed?.kind === "bus" || parsed?.kind === "load" ? parsed.buses[0] : null;
  const mine = entries
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => (entry.edit.kind === "load" ? entry.edit.bus === bus : entry.edit.element === element));
  const status = mine.find(({ entry }) => entry.edit.kind === "status")?.entry.edit;
  return { mine, outInStudy: status?.kind === "status" ? !status.online : null };
}

export interface ElementInspectorProps {
  element: string;
  /** The module showing the inspector, for "Draw around" and selection. */
  from: ModuleId;
  /** Inspect another element (a terminal bus, a connected branch). Defaults to the shared selection. */
  onPick?: (element: string) => void;
  onClose?: () => void;
  /** Module-specific sections, shown after the study. */
  children?: ReactNode;
  /** The diagram's drawing fields, folded away by default. */
  drawing?: ReactNode;
}

export function ElementInspector({ element, from, onPick, onClose, children, drawing }: ElementInspectorProps) {
  const report = useWorkspace((s) => s.solveReport);
  const focus = useWorkspace((s) => s.focus);
  const requestSolve = useWorkspace((s) => s.requestSolve);
  const canSolve = useWorkspace((s) => !!s.caseSummary?.power.available);
  const removeEdit = useStudy((s) => s.remove);
  const pick = onPick ?? focus;
  const { detail, error } = useElementDetail(element);
  const results = report ? elementResults(report, element) : null;
  const { mine, outInStudy } = useStudyFor(element);
  const kind = parseElement(element)?.kind ?? element.split(":")[0];

  const offline = outInStudy ?? (results ? results.offline : detail ? !detail.online : false);
  const status = outInStudy === true ? "Out in this study" : offline ? "Out of service" : "In service";
  const tone: Tone = results && !offline ? results.tone : "ok";

  return (
    <div className="flex flex-col text-xs" data-testid="element-inspector" data-element={element}>
      <header className="px-3 pt-2.5 pb-2">
        <div className="flex items-center gap-2">
          <p className="text-[10px] font-semibold uppercase tracking-wider text-slate-400">{KIND_LABEL[kind] ?? kind}</p>
          <span
            className={`rounded px-1.5 py-px text-[10px] font-semibold ${
              offline ? "bg-red-50 text-red-700" : "bg-emerald-50 text-emerald-800"
            }`}
            data-testid="inspector-status"
          >
            {status}
          </span>
          {onClose && (
            <button
              type="button"
              className="ml-auto -mr-1 p-0.5 rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700"
              onClick={onClose}
              aria-label="Close inspector"
              title="Close"
            >
              <X size={14} />
            </button>
          )}
        </div>
        <h3 className="mt-0.5 text-sm font-semibold text-slate-800 leading-snug" data-testid="inspector-title">
          {elementLabel(element)}
        </h3>
        {detail && detail.buses.length > 0 && (
          <p className="mt-0.5 flex flex-wrap gap-x-2 gap-y-0.5 text-[11px] text-slate-500">
            {detail.buses.map((b) => {
              const kv = b.base_kv > 0 ? `${fixed(b.base_kv, b.base_kv < 10 ? 2 : 0)} kV` : "";
              const text = [b.name, kv].filter(Boolean).join(" · ");
              return kind === "bus" ? (
                <span key={b.number}>{text}</span>
              ) : (
                <ElementButton key={b.number} element={busElement(b.number)} onPick={pick}>
                  {b.name ? `${b.number} ${b.name}` : `Bus ${b.number}`}
                  {b.base_kv > 0 ? ` · ${fixed(b.base_kv, b.base_kv < 10 ? 2 : 0)} kV` : ""}
                </ElementButton>
              );
            })}
          </p>
        )}
        {results?.headline && !offline && (
          <div className="mt-2 flex items-center gap-2" data-testid="inspector-headline">
            <span className={`rounded px-1.5 py-0.5 text-[11px] font-semibold ${TONE_CHIP[tone]}`}>{results.headline}</span>
            {results.loading !== null && (kind === "line" || kind === "transformer") && (
              <span className="flex-1 min-w-0 h-1.5 rounded-full bg-slate-100 overflow-hidden" aria-hidden="true" data-testid="inspector-loading">
                <span
                  className={`block h-full ${tone === "bad" ? "bg-red-500" : tone === "warn" ? "bg-amber-500" : "bg-teal-600"}`}
                  style={{ width: `${Math.min(100, Math.max(0, results.loading))}%` }}
                />
              </span>
            )}
          </div>
        )}
        <div className="mt-2 -ml-1.5 flex flex-wrap items-center gap-0.5">
          <DrawAroundButton element={element} from={from} />
          <StudyActions element={element} />
          <TripButton element={element} from={from} />
        </div>
      </header>
      <TripOutcome element={element} onPick={pick} />

      <Section id="results" title="Results">
        {!report ? (
          <p className="text-slate-500">
            Not solved yet.{" "}
            {canSolve && (
              <button type="button" className="inline-flex items-center gap-0.5 text-teal-700 underline" onClick={requestSolve}>
                <Play size={11} /> Solve
              </button>
            )}
          </p>
        ) : !results ? (
          <p className="text-slate-500">The latest solve doesn't list this element.</p>
        ) : results.offline || outInStudy ? (
          <p className="text-slate-500">Out of service, so it carries no flow.</p>
        ) : (
          <div className="flex flex-col gap-2">
            {results.groups.map((g, i) => (
              <div key={g.title || i}>
                {g.title && <p className="mb-0.5 text-[11px] font-medium text-slate-600">{g.title}</p>}
                <Rows rows={g.rows as ResultRow[]} />
              </div>
            ))}
          </div>
        )}
      </Section>

      {detail && detail.limits.length > 0 && (
        <Section id="limits" title="Limits">
          <Rows rows={detail.limits.map((f) => ({ label: f.label, value: fieldValue(f), unit: f.unit }))} />
        </Section>
      )}

      <Section id="data" title="Case data">
        {error ? (
          <p className="text-red-700">{error}</p>
        ) : !detail ? (
          <p className="text-slate-400">{canSolve ? "Reading…" : "Power flow can't read this case, so its data isn't available."}</p>
        ) : (
          <div className="flex flex-col gap-2">
            {detail.data.map((g, i) => (
              <div key={g.title || i}>
                {g.title && <p className="mb-0.5 text-[11px] font-medium text-slate-600">{g.title}</p>}
                <Rows rows={g.fields.map((f) => ({ label: f.label, value: fieldValue(f), unit: f.unit }))} />
              </div>
            ))}
            {detail.note && <p className="text-[11px] text-slate-500">{detail.note}</p>}
            <p className="font-mono text-[10px] text-slate-400 break-all" title="Shared id used by every module">
              {element}
            </p>
          </div>
        )}
      </Section>

      {detail && detail.connected.length > 0 && (
        <Section id="connected" title="Connected" count={detail.connected.length}>
          <ul className="flex flex-col gap-0.5" data-testid="inspector-connected">
            {detail.connected.map((c) => (
              <li key={c}>
                <ElementButton element={c} onPick={pick} />
              </li>
            ))}
          </ul>
        </Section>
      )}

      <Section id="study" title="Study" count={mine.length}>
        {mine.length === 0 ? (
          <p className="text-slate-500">No study edits to this element.</p>
        ) : (
          <ul className="flex flex-col gap-1" data-testid="inspector-edits">
            {mine.map(({ entry, index }) => (
              <li key={editTarget(entry.edit)} className="flex items-start gap-1">
                <span className="flex-1 min-w-0 text-slate-700">{describeEdit(entry)}</span>
                <button
                  type="button"
                  className="shrink-0 p-0.5 rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700"
                  onClick={() => void removeEdit(index)}
                  aria-label={`Undo: ${describeEdit(entry)}`}
                  title="Remove this edit from the study"
                >
                  <X size={12} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </Section>

      {children}

      {drawing && (
        <Section id="drawing" title="Drawing" defaultOpen={false}>
          {drawing}
        </Section>
      )}
    </div>
  );
}

/** A wrapper for module-specific sections, so they look like the shared ones. */
export { Section as InspectorSection };

/**
 * The inspector's column beside a canvas or table, or, in a narrow pane, a
 * card floating over its right edge so the drawing keeps the width.
 */
export function inspectorClass(floating: boolean, top = "top-2"): string {
  return floating
    ? `absolute right-2 ${top} bottom-2 z-30 w-72 max-w-[calc(100%-1rem)] rounded-lg border border-slate-200 bg-white shadow-lg overflow-y-auto`
    : "w-72 shrink-0 border-l border-slate-200 bg-white overflow-y-auto";
}
