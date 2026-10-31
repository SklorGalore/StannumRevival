// The scaling wizard: pick buses by area, zone, owner and kV, say how their
// load and generation scale, check the preview, then scale. The new values go
// into the case's records, as Records edits do, so History logs one
// scale_load_generation command that a run file or script can repeat.
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Scale } from "lucide-react";
import { api } from "../../lib/analysis/api";
import type { ScaleReport, ScaleTotals } from "../../lib/analysis/commands.gen";
import { count, fixed } from "../../lib/analysis/format";
import { describeBuses, describeScaling, scaleSettings, useScaling, type AmountKind, type ScaleChoice } from "../../store/scaleStore";
import { useWorkspace } from "../../store/workspaceStore";
import { useLog } from "../../store/logStore";
import { Button, ErrorBanner, Notice, inputClass } from "../../ui/primitives";
import { Modal } from "../../ui/Modal";
import { useCaseGroups } from "../../ui/useCaseGroups";
import { Picks } from "./ContingencySetup";

type Step = "buses" | "load" | "generation" | "preview";

const STEPS: { id: Step; label: string; hint: string }[] = [
  { id: "buses", label: "Buses", hint: "Area, zone, owner, kV" },
  { id: "load", label: "Load", hint: "MW, Mvar, power factor" },
  { id: "generation", label: "Generation", hint: "MW, limits, sharing" },
  { id: "preview", label: "Preview", hint: "Check before scaling" },
];

/** How many changed rows the preview lists. */
const ROWS_SHOWN = 200;

/** Opens the wizard, from the Analysis menu or Ctrl+K. */
export function openScaling() {
  useScaling.getState().setOpen(true);
}

/** Whether the open case can be scaled, or why not. */
export function scalingBlocked(): string | null {
  const s = useWorkspace.getState().caseSummary;
  if (!s) return "Open a case first";
  if (!s.power.available) return "This case has no power flow data";
  return null;
}

export function ScaleDialog() {
  const open = useScaling((s) => s.open);
  if (!open) return null;
  return <ScaleWizard />;
}

function ScaleWizard() {
  const { choice, setChoice, setOpen, reset } = useScaling();
  const [step, setStep] = useState<Step>("buses");
  const groups = useCaseGroups();
  const solved = useWorkspace((s) => s.solveReport);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const areaNames = useMemo(() => new Map((solved?.areas ?? []).map((a) => [a.number, a.name.trim()])), [solved]);
  const built = scaleSettings(choice);
  const key = JSON.stringify(built);
  const [preview, setPreview] = useState<ScaleReport | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // The preview follows every change, a moment after typing stops.
  useEffect(() => {
    if (!("settings" in built)) {
      setPreview(null);
      setPreviewError(null);
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      api
        .scalePreview(built.settings)
        .then((r) => live && (setPreview(r), setPreviewError(null)))
        .catch((e: Error) => live && (setPreview(null), setPreviewError(e.message)));
    }, 250);
    return () => {
      live = false;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, caseVersion]);

  const close = () => setOpen(false);
  async function scale() {
    if (!("settings" in built)) return;
    setBusy(true);
    setError(null);
    try {
      const w = useWorkspace.getState();
      const report = await w.scaleLoadGeneration(built.settings);
      useLog.getState().add("info", reportLine(report));
      close();
      if (choice.solveAfter && report.changed > 0) w.requestSolve();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const problem = "problem" in built ? built.problem : previewError;
  const nothing = preview !== null && preview.changed === 0;
  return (
    <Modal label="Scale load and generation" onClose={close} className="w-[min(960px,calc(100vw-2rem))] h-[min(640px,calc(100vh-2rem))] flex flex-col">
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-200">
        <h2 className="text-sm font-semibold text-slate-800 flex items-center gap-2">
          <Scale size={15} className="text-teal-700" /> Scale load and generation
        </h2>
        <Button type="button" variant="ghost" onClick={reset} title="Put every choice back to its default">
          Reset
        </Button>
      </div>
      <div className="flex flex-1 min-h-0">
        <nav role="tablist" aria-label="Scaling steps" aria-orientation="vertical" className="w-48 shrink-0 border-r border-slate-200 bg-slate-50 p-2 flex flex-col gap-0.5">
          {STEPS.map((t, i) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={step === t.id}
              onClick={() => setStep(t.id)}
              data-testid={`scale-tab-${t.id}`}
              className={`text-left rounded px-2 py-1.5 ${step === t.id ? "bg-white shadow-sm ring-1 ring-slate-200" : "hover:bg-slate-100"}`}
            >
              <span className="block text-xs font-medium text-slate-800">
                <span className="tabular-nums text-slate-400 mr-1.5">{i + 1}</span>
                {t.label}
              </span>
              <span className="block text-[11px] text-slate-500 ml-4">{t.hint}</span>
            </button>
          ))}
          <div className="mt-auto p-2 text-[11px] text-slate-500 leading-snug" data-testid="scale-bus-count">
            {preview ? `${count(preview.buses)} bus${preview.buses === 1 ? "" : "es"} chosen` : describeBuses(choice)}
          </div>
        </nav>
        <div role="tabpanel" className="flex-1 min-w-0 overflow-auto p-4 text-xs" data-testid={`scale-panel-${step}`}>
          {step === "buses" && (
            <>
              <p className="mb-3 text-[11px] text-slate-500 max-w-2xl">
                Leave a list empty to take every bus. A bus must pass every list you pick from and the kV range, so area 1 with 138 kV and up means the 138 kV and higher buses of area 1.
              </p>
              <div className="grid grid-cols-3 gap-4 mb-4">
                <Picks title="Areas" options={groups?.area ?? []} names={areaNames} chosen={choice.areas} onChange={(areas) => setChoice({ areas })} testId="scale-areas" />
                <Picks title="Zones" options={groups?.zone ?? []} chosen={choice.zones} onChange={(zones) => setChoice({ zones })} testId="scale-zones" />
                <Picks title="Owners" options={groups?.owner ?? []} chosen={choice.owners} onChange={(owners) => setChoice({ owners })} testId="scale-owners" />
              </div>
              <div className="flex flex-wrap items-end gap-4">
                <KvInput label="From (kV)" value={choice.kvMin} onChange={(kvMin) => setChoice({ kvMin })} testId="scale-kv-min" />
                <KvInput label="To (kV)" value={choice.kvMax} onChange={(kvMax) => setChoice({ kvMax })} testId="scale-kv-max" />
              </div>
            </>
          )}
          {step === "load" && <LoadStep choice={choice} set={setChoice} totals={preview?.load ?? null} />}
          {step === "generation" && <GenerationStep choice={choice} set={setChoice} report={preview} />}
          {step === "preview" && <PreviewStep report={preview} problem={problem} />}
        </div>
      </div>
      <div className="flex items-center gap-2 px-4 py-3 border-t border-slate-200">
        <span className="text-[11px] text-slate-500 min-w-0 flex-1 truncate" data-testid="scale-summary" title={describeScaling(choice)}>
          {problem ? <span className="text-amber-700">{problem}</span> : describeScaling(choice)}
        </span>
        <label className="flex items-center gap-1.5 text-[11px] text-slate-600 whitespace-nowrap" title="Run the power flow with the scaled case right away">
          <input type="checkbox" checked={choice.solveAfter} onChange={(e) => setChoice({ solveAfter: e.target.checked })} data-testid="scale-solve-after" />
          Solve after scaling
        </label>
        {step !== "preview" && (
          <Button type="button" onClick={() => setStep(STEPS[STEPS.findIndex((t) => t.id === step) + 1].id)}>
            Next
          </Button>
        )}
        <Button type="button" onClick={close}>
          Cancel
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={!!problem || busy || !preview || nothing}
          title={nothing ? "Nothing would change" : undefined}
          onClick={() => void scale()}
          data-testid="scale-apply"
          className="whitespace-nowrap"
        >
          <Scale size={13} /> {busy ? "Scaling…" : "Scale"}
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

function reportLine(r: ScaleReport): string {
  const parts: string[] = [];
  if (r.load) parts.push(`load ${fixed(r.load.mw_before, 1)} → ${fixed(r.load.mw_after, 1)} MW, ${fixed(r.load.mvar_before, 1)} → ${fixed(r.load.mvar_after, 1)} Mvar`);
  if (r.generation) parts.push(`generation ${fixed(r.generation.mw_before, 1)} → ${fixed(r.generation.mw_after, 1)} MW`);
  return `Scaled ${count(r.changed)} element${r.changed === 1 ? "" : "s"} at ${count(r.buses)} buses: ${parts.join("; ")}`;
}

function Group({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  return (
    <section className="mb-5">
      <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase mb-2">{title}</h3>
      <div className="flex flex-wrap items-end gap-x-5 gap-y-3">{children}</div>
      {note && <p className="mt-2 text-[11px] text-slate-500 max-w-2xl">{note}</p>}
    </section>
  );
}

function Check({ checked, onChange, children, hint, testId }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode; hint?: string; testId?: string }) {
  return (
    <label className="flex items-center gap-1.5 text-xs text-slate-700" title={hint}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} data-testid={testId} />
      {children}
    </label>
  );
}

function KvInput({ label, value, onChange, testId }: { label: string; value: number | null; onChange: (v: number | null) => void; testId: string }) {
  return (
    <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
      {label}
      <input
        className={`${inputClass} w-24`}
        type="number"
        min={0}
        step="any"
        value={value ?? ""}
        placeholder="any"
        onChange={(e) => onChange(e.target.value === "" ? null : Math.max(0, Number(e.target.value)))}
        data-testid={testId}
      />
    </label>
  );
}

const KINDS: { id: AmountKind; label: string }[] = [
  { id: "target", label: "Target total" },
  { id: "change", label: "Change by" },
  { id: "percent", label: "Change by %" },
];

/** Target, change or percent, the amount, and what the total is now. */
function AmountInput({
  label,
  unit,
  kind,
  value,
  now,
  after,
  disabled,
  onKind,
  onValue,
  testId,
}: {
  label: string;
  unit: string;
  kind: AmountKind;
  value: number | null;
  now: number | null;
  /** The total after scaling, from the preview. */
  after: number | null;
  disabled?: boolean;
  onKind: (k: AmountKind) => void;
  onValue: (v: number | null) => void;
  testId: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[11px] font-medium text-slate-600">{label}</span>
      <div className="flex items-center gap-1.5">
        <div role="radiogroup" aria-label={`${label}: how`} className="inline-flex rounded-md border border-slate-300 overflow-hidden">
          {KINDS.map((k) => (
            <button
              key={k.id}
              type="button"
              role="radio"
              aria-checked={kind === k.id}
              disabled={disabled}
              onClick={() => onKind(k.id)}
              data-testid={`${testId}-${k.id}`}
              className={`px-2 py-1.5 text-xs border-r last:border-r-0 border-slate-300 disabled:opacity-40 ${kind === k.id ? "bg-teal-700 text-white" : "bg-white text-slate-700 hover:bg-slate-50"}`}
            >
              {k.label}
            </button>
          ))}
        </div>
        <input
          className={`${inputClass} w-28 tabular-nums`}
          type="number"
          step="any"
          disabled={disabled}
          value={value ?? ""}
          placeholder={kind === "target" && now !== null ? fixed(now, 1) : "0"}
          onChange={(e) => onValue(e.target.value === "" ? null : Number(e.target.value))}
          aria-label={`${label}, ${kind === "percent" ? "%" : unit}`}
          data-testid={testId}
        />
        <span className="text-slate-500 w-8">{kind === "percent" ? "%" : unit}</span>
      </div>
      {now !== null && (
        <span className="text-[11px] text-slate-500 tabular-nums">
          Now {fixed(now, 1)} {unit}
          {after !== null && !disabled && (
            <>
              {" "}
              → {fixed(after, 1)} {unit}
            </>
          )}
        </span>
      )}
    </div>
  );
}

function LoadStep({ choice: c, set, totals }: { choice: ScaleChoice; set: (p: Partial<ScaleChoice>) => void; totals: ScaleTotals | null }) {
  return (
    <>
      <Group title="Load">
        <Check checked={c.scaleLoad} onChange={(scaleLoad) => set({ scaleLoad })} testId="scale-load-on">
          Scale load at the chosen buses
        </Check>
      </Group>
      <fieldset disabled={!c.scaleLoad} className="disabled:opacity-50">
        <Group
          title="Real power"
          note={c.mw === null ? "Leave blank to keep MW as it is and scale only Mvar." : totals ? `${count(totals.count)} load${totals.count === 1 ? "" : "s"} in service at the chosen buses.` : undefined}
        >
          <AmountInput label="Load MW" unit="MW" kind={c.mwKind} value={c.mw} now={totals?.mw_before ?? null} after={c.mw === null ? null : (totals?.mw_after ?? null)} onKind={(mwKind) => set({ mwKind })} onValue={(mw) => set({ mw })} testId="scale-load-mw" />
        </Group>
        <Group
          title="Reactive power"
          note={
            c.constantPowerFactor
              ? "Each load's Mvar moves by the same factor as its MW, so its power factor stays the same."
              : "Mvar scales on its own, or stays as it is when unchecked."
          }
        >
          <Check checked={c.constantPowerFactor} onChange={(constantPowerFactor) => set({ constantPowerFactor })} testId="scale-constant-pf">
            Constant power factor
          </Check>
          {!c.constantPowerFactor && (
            <Check checked={c.scaleMvar} onChange={(scaleMvar) => set({ scaleMvar })} testId="scale-mvar-on">
              Scale Mvar
            </Check>
          )}
        </Group>
        {!c.constantPowerFactor && c.scaleMvar && (
          <Group title="">
            <AmountInput label="Load Mvar" unit="Mvar" kind={c.mvarKind} value={c.mvar} now={totals?.mvar_before ?? null} after={c.mvar === null ? null : (totals?.mvar_after ?? null)} onKind={(mvarKind) => set({ mvarKind })} onValue={(mvar) => set({ mvar })} testId="scale-load-mvar" />
          </Group>
        )}
        <Group
          title="Which loads"
          note="Totals count the nominal load at 1 p.u.: constant power, current and admittance parts together. A percent change applies to each load that scales; a target or MW change counts the loads left alone too. Out-of-service loads never change."
        >
          <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
            Parts that scale
            <select className={inputClass} value={c.parts} onChange={(e) => set({ parts: e.target.value as ScaleChoice["parts"] })} data-testid="scale-parts">
              <option value="all">Constant power, current and admittance</option>
              <option value="constant_power">Constant power only</option>
            </select>
          </label>
          <Check
            checked={c.includeNegative}
            onChange={(includeNegative) => set({ includeNegative })}
            hint="Negative loads are often small generators modelled as load, so they're left alone unless this is on"
            testId="scale-negative"
          >
            Also scale negative loads
          </Check>
          <Check
            checked={c.includeNonConforming}
            onChange={(includeNonConforming) => set({ includeNonConforming })}
            hint="Non-conforming loads (SCALE = 0 in RAW) don't follow the system load level, such as industrial or station service load. PSS/E leaves them alone when scaling"
            testId="scale-non-conforming"
          >
            Also scale non-conforming loads
          </Check>
        </Group>
      </fieldset>
    </>
  );
}

const PARTICIPATION: { id: ScaleChoice["participation"]; label: string; hint: string }[] = [
  { id: "output", label: "Present output", hint: "Every unit scales by the same factor; units at 0 MW stay there" },
  { id: "pmax", label: "Pmax", hint: "Bigger units take more, whatever they produce now" },
  { id: "headroom", label: "Room to move", hint: "Pmax − PG going up, PG − Pmin going down" },
];

function GenerationStep({ choice: c, set, report }: { choice: ScaleChoice; set: (p: Partial<ScaleChoice>) => void; report: ScaleReport | null }) {
  const totals = report?.generation ?? null;
  return (
    <>
      <Group title="Generation">
        <Check checked={c.scaleGeneration} onChange={(scaleGeneration) => set({ scaleGeneration })} testId="scale-gen-on">
          Scale generation at the chosen buses
        </Check>
      </Group>
      <fieldset disabled={!c.scaleGeneration} className="disabled:opacity-50">
        <Group title="Real power" note={totals ? `${count(totals.count)} generator${totals.count === 1 ? "" : "s"} in service at the chosen buses. Offline units never change.` : undefined}>
          <AmountInput label="Generation MW" unit="MW" kind={c.genKind} value={c.gen} now={totals?.mw_before ?? null} after={c.gen === null ? null : (totals?.mw_after ?? null)} onKind={(genKind) => set({ genKind })} onValue={(gen) => set({ gen })} testId="scale-gen-mw" />
        </Group>
        <Group
          title="Limits"
          note={
            c.respectLimits
              ? "Units stop at Pmax going up and Pmin going down; the others pick up what they couldn't take."
              : "Units can go past Pmax and below Pmin."
          }
        >
          <div role="radiogroup" aria-label="Limits" className="flex gap-4">
            <label className="flex items-center gap-1.5">
              <input type="radio" name="limits" checked={c.respectLimits} onChange={() => set({ respectLimits: true })} data-testid="scale-respect-limits" />
              Respect Pmin and Pmax
            </label>
            <label className="flex items-center gap-1.5">
              <input type="radio" name="limits" checked={!c.respectLimits} onChange={() => set({ respectLimits: false })} data-testid="scale-ignore-limits" />
              Ignore Pmax
            </label>
          </div>
        </Group>
        <Group title="Share the change by">
          <div role="radiogroup" aria-label="Share the change by" className="flex flex-col gap-1.5">
            {PARTICIPATION.map((p) => (
              <label key={p.id} className="flex items-center gap-1.5">
                <input type="radio" name="participation" checked={c.participation === p.id} onChange={() => set({ participation: p.id })} data-testid={`scale-share-${p.id}`} />
                <span className="font-medium text-slate-700">{p.label}</span>
                <span className="text-slate-500">{p.hint}</span>
              </label>
            ))}
          </div>
        </Group>
      </fieldset>
      {c.scaleGeneration && report && Math.abs(report.shortfall_mw) > 0.0005 && (
        <Notice>
          The units can't take {fixed(Math.abs(report.shortfall_mw), 1)} MW of the change within their limits
          {report.at_limit ? `; ${count(report.at_limit)} end at a limit` : ""}.
        </Notice>
      )}
    </>
  );
}

function TotalsRow({ label, before, after, unit }: { label: string; before: number; after: number; unit: string }) {
  const d = after - before;
  return (
    <tr className="border-b border-slate-100">
      <td className="py-1 pr-4 text-slate-700">{label}</td>
      <td className="py-1 pr-4 text-right tabular-nums">{fixed(before, 1)}</td>
      <td className="py-1 pr-4 text-right tabular-nums font-medium">{fixed(after, 1)}</td>
      <td className={`py-1 pr-4 text-right tabular-nums ${d > 0 ? "text-teal-700" : d < 0 ? "text-amber-700" : "text-slate-400"}`}>
        {d > 0 ? "+" : ""}
        {fixed(d, 1)}
      </td>
      <td className="py-1 text-slate-500">{unit}</td>
    </tr>
  );
}

function PreviewStep({ report, problem }: { report: ScaleReport | null; problem: string | null }) {
  if (problem) return <Notice>{problem}</Notice>;
  if (!report) return <p className="text-slate-500">Working out the preview…</p>;
  const shown = report.rows.slice(0, ROWS_SHOWN);
  return (
    <div className="flex flex-col gap-4" data-testid="scale-preview">
      <table className="text-xs w-auto">
        <thead>
          <tr className="text-[10px] uppercase tracking-wider text-slate-500 text-left">
            <th className="pb-1 pr-4 font-semibold">{count(report.buses)} buses</th>
            <th className="pb-1 pr-4 font-semibold text-right">Before</th>
            <th className="pb-1 pr-4 font-semibold text-right">After</th>
            <th className="pb-1 pr-4 font-semibold text-right">Change</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {report.load && (
            <>
              <TotalsRow label={`Load (${count(report.load.count)})`} before={report.load.mw_before} after={report.load.mw_after} unit="MW" />
              <TotalsRow label="" before={report.load.mvar_before} after={report.load.mvar_after} unit="Mvar" />
            </>
          )}
          {report.generation && <TotalsRow label={`Generation (${count(report.generation.count)})`} before={report.generation.mw_before} after={report.generation.mw_after} unit="MW" />}
        </tbody>
      </table>
      {Math.abs(report.shortfall_mw) > 0.0005 && (
        <Notice>
          {fixed(Math.abs(report.shortfall_mw), 1)} MW of the generation change doesn't fit within the units' limits; the swing bus will make it up when the case is solved.
        </Notice>
      )}
      {report.notes.map((n) => (
        <p key={n} className="text-[11px] text-slate-500">
          {n}
        </p>
      ))}
      {report.load && report.generation === null && (
        <p className="text-[11px] text-slate-500">Generation isn't scaled, so the swing bus picks up the load change when the case is solved.</p>
      )}
      <div>
        <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase mb-1">
          {report.changed === 0 ? "Nothing changes" : `${count(report.changed)} change${report.changed === 1 ? "" : "s"}, biggest first`}
        </h3>
        {shown.length > 0 && (
          <div className="border border-slate-200 rounded overflow-auto max-h-72">
            <table className="text-xs w-full" data-testid="scale-preview-rows">
              <thead className="sticky top-0 bg-slate-50">
                <tr className="text-[10px] uppercase tracking-wider text-slate-500 text-left">
                  <th className="px-2 py-1 font-semibold">Element</th>
                  <th className="px-2 py-1 font-semibold text-right">Area</th>
                  <th className="px-2 py-1 font-semibold text-right">MW before</th>
                  <th className="px-2 py-1 font-semibold text-right">MW after</th>
                  <th className="px-2 py-1 font-semibold text-right">Mvar before</th>
                  <th className="px-2 py-1 font-semibold text-right">Mvar after</th>
                  <th className="px-2 py-1 font-semibold" />
                </tr>
              </thead>
              <tbody>
                {shown.map((r) => (
                  <tr key={r.element} className="border-t border-slate-100">
                    <td className="px-2 py-0.5 font-mono text-[11px]">{r.element}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums">{r.area}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums text-slate-500">{fixed(r.mw_before, 2)}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums">{fixed(r.mw_after, 2)}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums text-slate-500">{fixed(r.mvar_before, 2)}</td>
                    <td className="px-2 py-0.5 text-right tabular-nums">{fixed(r.mvar_after, 2)}</td>
                    <td className="px-2 py-0.5 text-[11px] text-amber-700">{r.at_limit ? "at limit" : ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {report.changed > shown.length && <p className="mt-1 text-[11px] text-slate-500">And {count(report.changed - shown.length)} smaller changes.</p>}
      </div>
    </div>
  );
}
