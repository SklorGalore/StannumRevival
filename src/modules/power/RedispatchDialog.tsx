// Redispatch to relieve overloads: choose the rating and limit, what to
// relieve and which generators may move, preview the moves (a DC optimal
// power flow checked in AC), then write the new outputs into the case's
// records, so History logs one redispatch command a run file can repeat.
import { useEffect, useMemo, useState } from "react";
import { Shuffle } from "lucide-react";
import { api } from "../../lib/analysis/api";
import type { RedispatchReport } from "../../lib/analysis/commands.gen";
import { count, fixed } from "../../lib/analysis/format";
import { isBranch, redispatchLine, redispatchSettings, useRedispatch } from "../../store/redispatchStore";
import { useWorkspace } from "../../store/workspaceStore";
import { useLog } from "../../store/logStore";
import { Button, ErrorBanner, Metric, Notice, inputClass } from "../../ui/primitives";
import { Modal } from "../../ui/Modal";
import { useCaseGroups } from "../../ui/useCaseGroups";
import { Picks } from "./ContingencySetup";

/** Opens the dialog, from the Analysis menu or Ctrl+K. A selected branch becomes the one to relieve. */
export function openRedispatch() {
  const element = useWorkspace.getState().selection?.element;
  const r = useRedispatch.getState();
  if (element && isBranch(element)) r.setChoice({ only: true, branches: element });
  r.setOpen(true);
}

/** Whether the open case can be redispatched, or why not. */
export function redispatchBlocked(): string | null {
  const w = useWorkspace.getState();
  if (!w.caseSummary) return "Open a case first";
  if (!w.caseSummary.power.available) return "This case has no power flow data";
  if (!w.solveReport) return "Solve the case in AC first";
  return null;
}

const STATUS: Record<string, { label: string; tone: "good" | "warn" | "bad" | "neutral" }> = {
  relieved: { label: "Relieved", tone: "good" },
  partly: { label: "Partly relieved", tone: "warn" },
  nothing_to_do: { label: "Nothing to do", tone: "neutral" },
  failed: { label: "AC solve failed", tone: "bad" },
};

export function RedispatchDialog() {
  const open = useRedispatch((s) => s.open);
  if (!open) return null;
  return <RedispatchPanel />;
}

function RedispatchPanel() {
  const { choice, setChoice, setOpen, reset } = useRedispatch();
  const groups = useCaseGroups();
  const solved = useWorkspace((s) => s.solveReport);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const areaNames = useMemo(() => new Map((solved?.areas ?? []).map((a) => [a.number, a.name.trim()])), [solved]);
  const built = redispatchSettings(choice);
  const key = JSON.stringify(built);
  const [preview, setPreview] = useState<RedispatchReport | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // A preview runs AC solves, so it waits for the button; a change clears it.
  useEffect(() => setPreview(null), [key, caseVersion]);

  const close = () => setOpen(false);
  async function runPreview() {
    if (!("settings" in built)) return;
    setPreviewing(true);
    setError(null);
    try {
      setPreview(await api.redispatchPreview(built.settings));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setPreviewing(false);
    }
  }
  async function apply() {
    if (!("settings" in built)) return;
    setBusy(true);
    setError(null);
    try {
      const w = useWorkspace.getState();
      const report = await w.redispatch(built.settings);
      useLog.getState().add("info", redispatchLine(report));
      close();
      if (choice.solveAfter && report.summary.units_moved > 0) w.requestSolve();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const problem = "problem" in built ? built.problem : null;
  const nothing = preview !== null && preview.summary.units_moved === 0;
  return (
    <Modal label="Redispatch to relieve overloads" onClose={close} className="w-[min(1040px,calc(100vw-2rem))] h-[min(680px,calc(100vh-2rem))] flex flex-col">
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-200">
        <h2 className="text-sm font-semibold text-slate-800 flex items-center gap-2">
          <Shuffle size={15} className="text-teal-700" /> Redispatch to relieve overloads
        </h2>
        <Button type="button" variant="ghost" onClick={reset} title="Put every choice back to its default">
          Reset
        </Button>
      </div>
      <div className="flex flex-1 min-h-0">
        <div className="w-72 shrink-0 border-r border-slate-200 bg-slate-50 p-3 overflow-auto text-xs flex flex-col gap-4" data-testid="redispatch-settings">
          <section className="flex flex-col gap-2">
            <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase">Limits</h3>
            <div className="flex gap-2">
              <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
                Rating
                <select className={inputClass} value={choice.rating} onChange={(e) => setChoice({ rating: e.target.value as typeof choice.rating })} data-testid="redispatch-rating">
                  <option value="a">Rate A</option>
                  <option value="b">Rate B</option>
                  <option value="c">Rate C</option>
                </select>
              </label>
              <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
                At or below, %
                <input
                  className={`${inputClass} w-20 tabular-nums`}
                  type="number"
                  min={1}
                  step="any"
                  value={choice.limitPct ?? ""}
                  onChange={(e) => setChoice({ limitPct: e.target.value === "" ? null : Number(e.target.value) })}
                  data-testid="redispatch-limit"
                />
              </label>
            </div>
            <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
              Monitor branches at or above, kV
              <input
                className={`${inputClass} w-24`}
                type="number"
                min={0}
                step="any"
                placeholder="any"
                value={choice.monitorKvMin ?? ""}
                onChange={(e) => setChoice({ monitorKvMin: e.target.value === "" ? null : Math.max(0, Number(e.target.value)) })}
                data-testid="redispatch-kv"
              />
            </label>
          </section>
          <section className="flex flex-col gap-2">
            <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase">Relieve</h3>
            <label className="flex items-center gap-1.5">
              <input type="radio" name="relieve" checked={!choice.only} onChange={() => setChoice({ only: false })} data-testid="redispatch-every" />
              Every branch over the limit
            </label>
            <label className="flex items-center gap-1.5">
              <input type="radio" name="relieve" checked={choice.only} onChange={() => setChoice({ only: true })} data-testid="redispatch-only" />
              Only these branches
            </label>
            {choice.only && (
              <textarea
                className={`${inputClass} font-mono text-[11px] h-16`}
                value={choice.branches}
                placeholder="line:1:2:1"
                onChange={(e) => setChoice({ branches: e.target.value })}
                aria-label="Branches to relieve"
                data-testid="redispatch-branches"
              />
            )}
            <p className="text-[11px] text-slate-500 leading-snug">Other monitored branches can't go over the limit, or further over it when they already are.</p>
          </section>
          <section className="flex flex-col gap-2">
            <Picks title="Generators may move in areas" options={groups?.area ?? []} names={areaNames} chosen={choice.areas} onChange={(areas) => setChoice({ areas })} testId="redispatch-areas" />
            <p className="text-[11px] text-slate-500 leading-snug">Every online unit with room between Pmin and Pmax when none is picked. Each island keeps its total.</p>
          </section>
          <label className="flex items-center gap-1.5" title="Solve in AC after each pass and move again while a branch is still over; off gives the DC estimate">
            <input type="checkbox" checked={choice.acCheck} onChange={(e) => setChoice({ acCheck: e.target.checked })} data-testid="redispatch-ac" />
            Check in AC
          </label>
        </div>
        <div className="flex-1 min-w-0 overflow-auto p-4 text-xs" data-testid="redispatch-results">
          {!preview && (
            <div className="flex flex-col gap-3 max-w-xl">
              <p className="text-slate-600">
                Moves online generators up and down, as little as it can, so monitored branches end within the limit: a DC optimal power flow from the last AC solve, checked with AC solves.
              </p>
              <div>
                <Button type="button" variant="primary" disabled={!!problem || previewing} onClick={() => void runPreview()} data-testid="redispatch-preview">
                  {previewing ? "Working it out…" : "Preview"}
                </Button>
              </div>
              {problem && <Notice>{problem}</Notice>}
            </div>
          )}
          {preview && <Preview report={preview} />}
        </div>
      </div>
      <div className="flex items-center gap-2 px-4 py-3 border-t border-slate-200">
        <span className="text-[11px] text-slate-500 min-w-0 flex-1 truncate" data-testid="redispatch-summary">
          {preview ? redispatchLine(preview) : problem ? <span className="text-amber-700">{problem}</span> : "Preview to see the moves."}
        </span>
        <label className="flex items-center gap-1.5 text-[11px] text-slate-600 whitespace-nowrap" title="Run the power flow with the new outputs right away">
          <input type="checkbox" checked={choice.solveAfter} onChange={(e) => setChoice({ solveAfter: e.target.checked })} data-testid="redispatch-solve-after" />
          Solve after
        </label>
        <Button type="button" onClick={close}>
          Cancel
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={!!problem || busy || !preview || nothing}
          title={nothing ? "Nothing would move" : !preview ? "Preview first" : undefined}
          onClick={() => void apply()}
          data-testid="redispatch-apply"
          className="whitespace-nowrap"
        >
          <Shuffle size={13} /> {busy ? "Redispatching…" : "Redispatch"}
        </Button>
      </div>
      {error && (
        <div className="px-4 pb-3">
          <ErrorBanner>{error}</ErrorBanner>
        </div>
      )}
    </Modal>
  );
}

function pctTone(pct: number, limit: number): string {
  return pct > limit + 0.05 ? "text-red-700 font-medium" : pct > limit - 5 ? "text-amber-700" : "";
}

function Preview({ report }: { report: RedispatchReport }) {
  const s = report.summary;
  const status = STATUS[s.status] ?? { label: s.status, tone: "neutral" as const };
  return (
    <div className="flex flex-col gap-4" data-testid="redispatch-report">
      <div className="grid grid-cols-4 gap-2">
        <Metric label="Result" value={status.label} tone={status.tone} detail={s.ac_checked ? `${s.passes} AC pass${s.passes === 1 ? "" : "es"}` : "DC estimate"} />
        <Metric label="Moved" value={`${fixed(s.moved_mw, 1)} MW`} detail={`${count(s.units_moved)} of ${count(s.movable)} units`} />
        <Metric label="Over the limit" value={`${s.over_before} → ${s.over_after}`} tone={s.over_after ? "warn" : "good"} detail={`of ${count(s.monitored)} monitored`} />
        <Metric
          label="Worst after"
          value={s.worst_after ? `${fixed(s.worst_after.pct_after, 1)}%` : "–"}
          tone={s.worst_after && s.worst_after.pct_after > s.worst_after.limit_pct + 0.05 ? "bad" : "neutral"}
          detail={s.worst_after?.name}
        />
      </div>
      {report.notes.map((n) => (
        <p key={n} className="text-[11px] text-slate-600">
          {n}
        </p>
      ))}
      {report.branches.length > 0 && (
        <section>
          <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase mb-1">Branches</h3>
          <div className="border border-slate-200 rounded overflow-auto max-h-56">
            <table className="text-xs w-full" data-testid="redispatch-branches-table">
              <thead className="sticky top-0 bg-slate-50">
                <tr className="text-[10px] uppercase tracking-wider text-slate-500 text-left">
                  <th className="px-2 py-1 font-semibold">Branch</th>
                  <th className="px-2 py-1 font-semibold text-right">Rating MVA</th>
                  <th className="px-2 py-1 font-semibold text-right">Limit %</th>
                  <th className="px-2 py-1 font-semibold text-right">Before %</th>
                  <th className="px-2 py-1 font-semibold text-right">After %</th>
                  <th className="px-2 py-1 font-semibold text-right">MW before</th>
                  <th className="px-2 py-1 font-semibold text-right">MW after</th>
                </tr>
              </thead>
              <tbody>
                {report.branches.map((b) => (
                  <tr key={b.element} className="border-t border-slate-100">
                    <td className="px-2 py-0.5" title={b.element}>
                      {b.name}
                      {b.target && <span className="ml-1.5 text-[10px] text-teal-700">to relieve</span>}
                    </td>
                    <td className="px-2 py-0.5 text-right tabular-nums">{fixed(b.rating_mva, 0)}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums text-slate-500">{fixed(b.limit_pct, 1)}</td>
                    <td className={`px-2 py-0.5 text-right tabular-nums ${pctTone(b.pct_before, b.limit_pct)}`}>{fixed(b.pct_before, 1)}</td>
                    <td className={`px-2 py-0.5 text-right tabular-nums ${pctTone(b.pct_after, b.limit_pct)}`}>{fixed(b.pct_after, 1)}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums text-slate-500">{fixed(b.mw_before, 1)}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums">{fixed(b.mw_after, 1)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
      {report.units.length > 0 && (
        <section>
          <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase mb-1">Generators, biggest move first</h3>
          <div className="border border-slate-200 rounded overflow-auto max-h-56">
            <table className="text-xs w-full" data-testid="redispatch-units-table">
              <thead className="sticky top-0 bg-slate-50">
                <tr className="text-[10px] uppercase tracking-wider text-slate-500 text-left">
                  <th className="px-2 py-1 font-semibold">Generator</th>
                  <th className="px-2 py-1 font-semibold text-right">Area</th>
                  <th className="px-2 py-1 font-semibold text-right">MW before</th>
                  <th className="px-2 py-1 font-semibold text-right">MW after</th>
                  <th className="px-2 py-1 font-semibold text-right">Change</th>
                  <th className="px-2 py-1 font-semibold text-right">Pmin – Pmax</th>
                </tr>
              </thead>
              <tbody>
                {report.units.map((u) => {
                  const d = u.mw_after - u.mw_before;
                  return (
                    <tr key={u.element} className="border-t border-slate-100">
                      <td className="px-2 py-0.5" title={u.element}>
                        {u.name}
                      </td>
                      <td className="px-2 py-0.5 text-right tabular-nums">{u.area}</td>
                      <td className="px-2 py-0.5 text-right tabular-nums text-slate-500">{fixed(u.mw_before, 1)}</td>
                      <td className="px-2 py-0.5 text-right tabular-nums">{fixed(u.mw_after, 1)}</td>
                      <td className={`px-2 py-0.5 text-right tabular-nums ${d > 0 ? "text-teal-700" : "text-amber-700"}`}>
                        {d > 0 ? "+" : ""}
                        {fixed(d, 1)}
                      </td>
                      <td className="px-2 py-0.5 text-right tabular-nums text-slate-500">
                        {fixed(u.pmin, 0)} – {fixed(u.pmax, 0)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}
