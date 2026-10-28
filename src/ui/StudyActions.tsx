// Study edits from wherever an element is shown: take a branch or generator
// out of service (or put it back), set a generator's output, or a bus's load.
import { useEffect, useRef, useState } from "react";
import { FlaskConical, Power } from "lucide-react";
import { useWorkspace } from "../store/workspaceStore";
import { useStudy, type StudyEntry } from "../store/studyStore";
import { elementBus, elementLabel, parseElement } from "../lib/elements";
import { fixed } from "../lib/analysis/format";
import { inputClass } from "./primitives";

/** A study edit in words, such as "Line 4–5 circuit 1 out of service". */
export function describeEdit(entry: StudyEntry): string {
  const e = entry.edit;
  const was = entry.was ? ` (was ${entry.was})` : "";
  if (e.kind === "status") {
    const state = parseElement(e.element)?.kind === "switch" ? (e.online ? "closed" : "open") : e.online ? "in service" : "out of service";
    return `${elementLabel(e.element)} ${state}`;
  }
  if (e.kind === "generation") return `${elementLabel(e.element)} set to ${fixed(e.mw, 1)} MW${was}`;
  return `Bus ${e.bus} load ${fixed(e.mw, 1)} MW, ${fixed(e.mvar, 1)} Mvar${was}`;
}

/** Whether an element is in service now: the study's word, else the latest solve's. */
function useInService(element: string): boolean {
  const entries = useStudy((s) => s.entries);
  const report = useWorkspace((s) => s.solveReport);
  const edit = entries.find((e) => e.edit.kind === "status" && e.edit.element === element);
  if (edit && edit.edit.kind === "status") return edit.edit.online;
  const kind = parseElement(element)?.kind;
  if (kind === "generator") return report?.generators.find((g) => g.element === element)?.online ?? true;
  if (kind === "line" || kind === "transformer" || kind === "switch") return report?.branches.find((b) => b.element === element)?.online ?? true;
  return true;
}

function OutageButton({ element }: { element: string }) {
  const online = useInService(element);
  // A switching device is opened and closed rather than taken out.
  const sw = parseElement(element)?.kind === "switch";
  const add = useStudy((s) => s.add);
  const busy = useStudy((s) => s.busy);
  return (
    <button
      type="button"
      disabled={busy}
      className={`shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap disabled:opacity-50 ${
        online ? "text-slate-600 hover:bg-slate-100 hover:text-red-700" : "text-red-700 bg-red-50 hover:bg-red-100"
      }`}
      onClick={(e) => {
        e.stopPropagation();
        void add({ edit: { kind: "status", element, online: !online } });
      }}
      title={
        sw
          ? `${online ? "Open" : "Close"} ${elementLabel(element)} in this study`
          : online
            ? `Take ${elementLabel(element)} out of service in this study`
            : `Put ${elementLabel(element)} back in service`
      }
    >
      <Power size={12} />
      {sw ? (online ? "Open" : "Close") : online ? "Take out" : "Put back"}
    </button>
  );
}

/** A small form for a generator's output or a bus's load. */
function ValueEditor({ element }: { element: string }) {
  const report = useWorkspace((s) => s.solveReport);
  const add = useStudy((s) => s.add);
  const kind = parseElement(element)?.kind;
  const bus = kind === "generator" ? null : elementBus(element);
  const gen = kind === "generator" ? report?.generators.find((g) => g.element === element) : undefined;
  const busRow = bus !== null ? report?.buses.find((b) => b.id === bus) : undefined;
  const [open, setOpen] = useState(false);
  const [mw, setMw] = useState("");
  const [mvar, setMvar] = useState("");
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && setOpen(false);
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [open]);
  if (!gen && !busRow) return null;
  const label = gen ? "Set output" : "Set load";
  async function apply() {
    const p = Number(mw);
    const q = Number(mvar);
    if (!Number.isFinite(p) || (busRow && !Number.isFinite(q))) return;
    const ok = gen
      ? await add({ edit: { kind: "generation", element, mw: p }, was: `${fixed(gen.pg, 1)} MW` })
      : await add({ edit: { kind: "load", bus: busRow!.id, mw: p, mvar: q }, was: `${fixed(busRow!.pd, 1)} MW, ${fixed(busRow!.qd, 1)} Mvar` });
    if (ok) setOpen(false);
  }
  return (
    <span ref={ref} className="relative">
      <button
        type="button"
        className="shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium text-slate-600 hover:bg-slate-100 whitespace-nowrap"
        onClick={(e) => {
          e.stopPropagation();
          setMw(gen ? String(+gen.pg.toFixed(2)) : String(+busRow!.pd.toFixed(2)));
          setMvar(busRow ? String(+busRow.qd.toFixed(2)) : "");
          setOpen(!open);
        }}
        title={gen ? "Change this generator's active power output in the study" : "Change this bus's load in the study"}
      >
        <FlaskConical size={12} /> {label}
      </button>
      {open && (
        <form
          role="dialog"
          aria-label={label}
          className="absolute right-0 top-full mt-1 z-30 w-56 rounded-md border border-slate-200 bg-white shadow-lg p-2 flex flex-col gap-1.5 text-xs"
          onClick={(e) => e.stopPropagation()}
          onSubmit={(e) => {
            e.preventDefault();
            void apply();
          }}
        >
          <label className="flex items-center gap-2">
            <span className="w-12 text-slate-500">MW</span>
            <input autoFocus aria-label="MW" type="number" step="any" className={`${inputClass} flex-1 min-w-0`} value={mw} onChange={(e) => setMw(e.target.value)} />
          </label>
          {busRow && (
            <label className="flex items-center gap-2">
              <span className="w-12 text-slate-500">Mvar</span>
              <input aria-label="Mvar" type="number" step="any" className={`${inputClass} flex-1 min-w-0`} value={mvar} onChange={(e) => setMvar(e.target.value)} />
            </label>
          )}
          <button type="submit" className="self-end rounded px-2 py-0.5 text-[11px] font-medium text-white bg-teal-700 hover:bg-teal-800">
            Apply to study
          </button>
        </form>
      )}
    </span>
  );
}

/** The study actions that fit an element; nothing for kinds a study can't edit. */
export function StudyActions({ element }: { element: string }) {
  const canSolve = useWorkspace((s) => !!s.caseSummary?.power.available);
  const kind = parseElement(element)?.kind;
  if (!canSolve || !kind) return null;
  const outage = kind === "line" || kind === "transformer" || kind === "switch" || kind === "generator";
  const value = kind === "generator" || kind === "bus" || kind === "load";
  if (!outage && !value) return null;
  return (
    <span className="inline-flex items-center gap-0.5" data-testid="study-actions">
      {outage && <OutageButton element={element} />}
      {value && <ValueEditor element={element} />}
    </span>
  );
}
