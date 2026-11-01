// PV curves (voltage stability): how far load in an area, zone, owner or
// list of buses can grow, or power can move there from a source, before the
// network runs out of voltage support. The study is stannum-core's (pv.rs):
// stepped AC solves up to the nose. This sets it up and draws the curves.
import { useEffect, useMemo, useState } from "react";
import { Download, Play, Square, TrendingDown } from "lucide-react";
import { usePv, pvRows, pvSettings, validPositive } from "../../store/pvStore";
import { busList, type Side, type SideKind } from "../../store/transferStore";
import { saveCsvExport } from "../../lib/analysis/api";
import { useCaseGroups, type CaseGroups } from "../../ui/useCaseGroups";
import type { SolveReport } from "../../lib/analysis/types";
import type { PvCurve, PvPoint, Scaling } from "../../lib/analysis/commands.gen";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import { Button, EmptyState, ErrorBanner, Metric, Notice, Tabs, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { PvChart, curveLabel } from "./PvChart";

const KINDS: { value: SideKind; label: string }[] = [
  { value: "area", label: "Area" },
  { value: "zone", label: "Zone" },
  { value: "owner", label: "Owner" },
  { value: "buses", label: "Buses" },
];

export const SINK_SCALINGS: { value: Scaling; label: string; hint: string }[] = [
  { value: "load", label: "Load grows", hint: "Every load in the sink grows by the same factor, at constant power factor" },
  { value: "generation", label: "Generation drops", hint: "The sink's online generators lower their output, in proportion to their MW" },
  { value: "reserve", label: "Generation drops (to Pmin)", hint: "The sink's online generators lower their output, in proportion to their room to Pmin" },
];

export const SOURCE_SCALINGS: { value: Scaling; label: string; hint: string }[] = [
  { value: "generation", label: "Generation output", hint: "Online generators raise output in proportion to their MW now" },
  { value: "reserve", label: "Generation reserve", hint: "Online generators raise output in proportion to their room to Pmax" },
  { value: "load", label: "Load drops", hint: "The source's loads drop, in proportion to their MW" },
];

const STOPPED: Record<string, string> = {
  collapse: "The network can't carry more: the nose",
  generation: "The source or sink ran out of room",
  max_transfer: "Reached the largest transfer asked for",
  max_points: "Reached the most steps",
  cancelled: "Cancelled",
};

type View = "curves" | "points";

function mw(v: number | null | undefined): string {
  return v === null || v === undefined ? "—" : `${fixed(v, 1)} MW`;
}

export function SidePicker({
  label,
  side,
  onChange,
  groups,
  areaNames,
  testId,
}: {
  label: string;
  side: Side;
  onChange: (s: Side) => void;
  groups: CaseGroups | null;
  areaNames: Map<number, string>;
  testId: string;
}) {
  const kinds = KINDS.filter((k) => k.value === "buses" || (groups?.[k.value as "area"]?.length ?? 0) > 0);
  const options = side.kind === "buses" ? [] : (groups?.[side.kind] ?? []);
  return (
    <div className="flex items-center gap-1" data-testid={testId}>
      <select
        className={inputClass}
        value={side.kind}
        aria-label={`${label}: by`}
        onChange={(e) => {
          const kind = e.target.value as SideKind;
          onChange({ ...side, kind, value: kind === "buses" ? 0 : (groups?.[kind]?.[0]?.[0] ?? 0) });
        }}
      >
        {kinds.map((k) => (
          <option key={k.value} value={k.value}>
            {k.label}
          </option>
        ))}
      </select>
      {side.kind === "buses" ? (
        <input
          className={`${inputClass} w-36`}
          value={side.buses}
          placeholder="101, 102, 205"
          aria-label={`${label}: bus numbers`}
          onChange={(e) => onChange({ ...side, buses: e.target.value })}
        />
      ) : (
        <select
          className={`${inputClass} max-w-52`}
          value={side.value}
          aria-label={`${label}: ${side.kind}`}
          onChange={(e) => onChange({ ...side, value: Number(e.target.value) })}
        >
          {options.map(([v, n]) => {
            const name = side.kind === "area" ? areaNames.get(v) : undefined;
            return (
              <option key={v} value={v}>
                {v}
                {name ? ` ${name}` : ""} ({count(n)} buses)
              </option>
            );
          })}
        </select>
      )}
    </div>
  );
}

function curveColumns(onPick: (bus: number) => void): Column<PvCurve>[] {
  return [
    {
      key: "bus",
      header: "Bus",
      value: (c) => curveLabel(c),
      render: (c) => (
        <button type="button" className="text-left text-teal-700 hover:underline" onClick={() => onPick(c.bus)}>
          {curveLabel(c)}
        </button>
      ),
    },
    { key: "start", header: "Start (p.u.)", value: (c) => c.vm_pu[0], render: (c) => fixed(c.vm_pu[0], 4), align: "right" },
    {
      key: "end",
      header: "Last (p.u.)",
      title: "Voltage at the last step that solved",
      value: (c) => c.vm_pu[c.vm_pu.length - 1],
      render: (c) => fixed(c.vm_pu[c.vm_pu.length - 1], 4),
      align: "right",
    },
    { key: "drop", header: "Drop (p.u.)", value: (c) => c.drop_pu, render: (c) => fixed(c.drop_pu, 4), align: "right" },
    {
      key: "sens",
      header: "dV per 100 MW",
      title: "Voltage drop per 100 MW over the last tenth of the curve: the largest is the weakest bus at the nose",
      value: (c) => c.sensitivity_pu_per_100mw,
      render: (c) => fixed(c.sensitivity_pu_per_100mw, 4),
      align: "right",
    },
    { key: "vmin", header: "Low limit", value: (c) => c.vmin_pu, render: (c) => fixed(c.vmin_pu, 2), align: "right" },
    {
      key: "cross",
      header: "Below limit at",
      title: "The transfer at which the bus falls below its low limit",
      value: (c) => c.vmin_transfer_mw ?? null,
      render: (c) =>
        c.vmin_transfer_mw === null || c.vmin_transfer_mw === undefined ? (
          <span className="text-slate-400">—</span>
        ) : (
          <span className="text-amber-700">{mw(c.vmin_transfer_mw)}</span>
        ),
      align: "right",
    },
  ];
}

const pointColumns: Column<PvPoint>[] = [
  { key: "transfer", header: "Transfer (MW)", value: (p) => p.transfer_mw, render: (p) => fixed(p.transfer_mw, 1), align: "right" },
  { key: "sink", header: "Sink load (MW)", value: (p) => p.sink_load_mw, render: (p) => fixed(p.sink_load_mw, 1), align: "right" },
  { key: "losses", header: "Losses (MW)", value: (p) => p.losses_mw, render: (p) => fixed(p.losses_mw, 1), align: "right" },
  { key: "min", header: "Lowest V (p.u.)", value: (p) => p.min_vm_pu, render: (p) => fixed(p.min_vm_pu, 4), align: "right" },
  { key: "minbus", header: "At bus", value: (p) => p.min_vm_bus, align: "right" },
  {
    key: "qlim",
    header: "At Q limit",
    title: "Online generators held at a reactive power limit",
    value: (p) => p.q_limited,
    align: "right",
  },
  {
    key: "qres",
    header: "Q reserve (Mvar)",
    title: "Room to Qmax left, summed over online generators",
    value: (p) => p.q_reserve_mvar,
    render: (p) => fixed(p.q_reserve_mvar, 0),
    align: "right",
  },
  { key: "it", header: "Iterations", value: (p) => p.iterations, align: "right" },
];

export function PvTab({ report, onPick }: { report: SolveReport | null; onPick: (element: string) => void }) {
  const s = usePv();
  const c = s.choice;
  const groups = useCaseGroups();
  const areaNames = useMemo(() => new Map((report?.areas ?? []).map((a) => [a.number, a.name.trim()])), [report]);
  const [view, setView] = useState<View>("curves");
  const pickBus = (bus: number) => onPick(`bus:${bus}`);

  // Start with the case's first area (or zone) once its groups are known.
  useEffect(() => {
    if (!groups) return;
    const known = (side: Side) => side.kind === "buses" || (groups[side.kind]?.some(([v]) => v === side.value) ?? false);
    if (known(c.sink) && known(c.source)) return;
    const kind = groups.area.length > 1 ? "area" : groups.zone.length > 1 ? "zone" : "area";
    const values = groups[kind].map(([v]) => v);
    s.setChoice({
      sink: { kind, value: values[values.length - 1] ?? 0, buses: "" },
      source: { kind, value: values[0] ?? 0, buses: "" },
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [groups]);

  const r = s.results;
  const sum = r?.summary;
  const acReady = !!report && report.model === "ac" && report.converged;
  const noBuses = (side: Side) => side.kind === "buses" && busList(side.buses).length === 0;
  const blocked = !acReady
    ? "Solve the case with the AC model first."
    : noBuses(c.sink) || (!c.allSource && noBuses(c.source))
      ? "List the sink's buses."
      : !validPositive(c.stepMw) || !validPositive(c.maxMw) || !validPositive(c.vminPu)
        ? "Steps, the largest transfer and the low limit must be numbers above zero, or blank."
        : null;
  const curveCols = useMemo(() => curveColumns(pickBus), [onPick]); // eslint-disable-line react-hooks/exhaustive-deps

  async function exportCsv() {
    if (!r) return;
    await saveCsvExport(
      "pv-curve.csv",
      "PV curve",
      { ...pvSettings(c), nose_mw: r.summary.nose_mw == null ? null : Number(r.summary.nose_mw.toFixed(1)) },
      toCsv(pvRows(r)),
    );
  }

  const numberInput = (value: string, onChange: (v: string) => void, aria: string, title: string, placeholder: string) => (
    <input
      className={`${inputClass} w-20`}
      inputMode="decimal"
      value={value}
      placeholder={placeholder}
      aria-label={aria}
      title={title}
      onChange={(e) => onChange(e.target.value)}
    />
  );
  const label = "flex flex-col gap-1 text-[11px] font-medium text-slate-600";
  const weakest = r?.curves.find((x) => x.bus === sum?.weakest_bus);

  return (
    <div className="h-full flex flex-col" data-testid="pv-tab">
      <form
        className="flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2 border-b border-slate-200 bg-white text-xs"
        onSubmit={(e) => {
          e.preventDefault();
          if (!blocked && !s.running) void s.run();
        }}
      >
        <div className={label}>
          Sink
          <SidePicker label="Sink" side={c.sink} onChange={(sink) => s.setChoice({ sink })} groups={groups} areaNames={areaNames} testId="pv-sink" />
        </div>
        <label className={label}>
          In the sink
          <select
            className={inputClass}
            value={c.sinkScaling}
            onChange={(e) => s.setChoice({ sinkScaling: e.target.value as Scaling })}
            title={SINK_SCALINGS.find((x) => x.value === c.sinkScaling)?.hint}
            aria-label="In the sink"
          >
            {SINK_SCALINGS.map((x) => (
              <option key={x.value} value={x.value} title={x.hint}>
                {x.label}
              </option>
            ))}
          </select>
        </label>
        <div className={label}>
          Picked up by
          <div className="flex items-center gap-1">
            <select
              className={inputClass}
              value={c.allSource ? "all" : "side"}
              onChange={(e) => s.setChoice({ allSource: e.target.value === "all" })}
              aria-label="Picked up by"
              data-testid="pv-source-kind"
            >
              <option value="all">Every bus</option>
              <option value="side">A source…</option>
            </select>
            {!c.allSource && (
              <SidePicker label="Source" side={c.source} onChange={(source) => s.setChoice({ source })} groups={groups} areaNames={areaNames} testId="pv-source" />
            )}
            <select
              className={inputClass}
              value={c.sourceScaling}
              onChange={(e) => s.setChoice({ sourceScaling: e.target.value as Scaling })}
              title={SOURCE_SCALINGS.find((x) => x.value === c.sourceScaling)?.hint}
              aria-label="Source shared out by"
            >
              {SOURCE_SCALINGS.map((x) => (
                <option key={x.value} value={x.value} title={x.hint}>
                  {x.label}
                </option>
              ))}
            </select>
          </div>
        </div>
        <label className={label}>
          Step (MW)
          {numberInput(c.stepMw, (stepMw) => s.setChoice({ stepMw }), "First step, MW", "The first step; halved near the nose. Blank for 5% of what the sink starts with", "Auto")}
        </label>
        <label className={label}>
          Stop at (MW)
          {numberInput(c.maxMw, (maxMw) => s.setChoice({ maxMw }), "Stop at, MW", "Stop at this transfer even when the network could carry more. Blank to go on to the nose", "Nose")}
        </label>
        <label className={label}>
          Low limit (p.u.)
          {numberInput(c.vminPu, (vminPu) => s.setChoice({ vminPu }), "Low voltage limit, p.u.", "Low voltage limit for every bus. Blank for each bus's own normal low limit from the case", "Case")}
        </label>
        <label
          className="flex items-center gap-1.5 mb-1.5"
          title="Stop when the source's generators reach Pmax (summed), the sink's reach Pmin, or a side's loads reach zero"
        >
          <input type="checkbox" checked={c.respectGeneration} onChange={(e) => s.setChoice({ respectGeneration: e.target.checked })} />
          Stop at generator limits
        </label>
        <div className="flex items-center gap-2 ml-auto">
          {s.running ? (
            <Button type="button" onClick={s.cancel} data-testid="pv-cancel">
              <Square size={13} /> Cancel
            </Button>
          ) : (
            <Button type="submit" variant="primary" disabled={!!blocked} title={blocked ?? undefined} data-testid="pv-run">
              <Play size={13} /> Trace the PV curve
            </Button>
          )}
        </div>
      </form>
      {s.running && (
        <div className="flex items-center gap-2 px-3 py-1 text-[11px] text-slate-600 border-b border-slate-100" role="status">
          <span className="inline-block h-1.5 w-24 overflow-hidden rounded bg-slate-100">
            <span className="block h-full w-1/3 animate-pulse bg-teal-600" />
          </span>
          Solving step {count((s.progress?.done ?? 0) + 1)}…
        </div>
      )}
      {s.error && (
        <div className="px-3 pt-2">
          <ErrorBanner>{s.error}</ErrorBanner>
        </div>
      )}
      {!r || !sum ? (
        <EmptyState icon={<TrendingDown size={22} />} title="PV curves">
          {blocked && !acReady ? (
            blocked
          ) : (
            <>
              How far the sink's load can grow, or power can move to it, before the network runs out of voltage support. Each step is a
              full AC solve with every control and reactive limit of your last solve, up to the nose of the curve.
            </>
          )}
        </EmptyState>
      ) : (
        <div className="flex-1 min-h-0 overflow-auto">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-2 px-3 pt-2" data-testid="pv-summary">
            <Metric
              label={sum.limited_by === "collapse" ? "Nose" : "Largest transfer"}
              value={`+${mw(sum.nose_mw)}`}
              tone={sum.limited_by === "collapse" ? "warn" : "neutral"}
              detail={STOPPED[sum.limited_by] ?? sum.limited_by}
            />
            <Metric
              label="Voltage limit"
              value={sum.voltage_limit_mw === null || sum.voltage_limit_mw === undefined ? "Not reached" : `+${mw(sum.voltage_limit_mw)}`}
              tone={sum.voltage_limit_mw !== null && sum.voltage_limit_mw !== undefined ? "bad" : "good"}
              detail={sum.voltage_limit_bus ? `Bus ${sum.voltage_limit_bus} drops below its low limit first` : "No monitored bus drops below its low limit"}
            />
            <Metric
              label="Weakest bus"
              value={sum.weakest_bus ? `${sum.weakest_bus}${sum.weakest_bus_name.trim() ? ` ${sum.weakest_bus_name.trim()}` : ""}` : "—"}
              detail={weakest ? `Falls ${fixed(weakest.sensitivity_pu_per_100mw, 4)} p.u. per 100 MW near the nose` : "Falls fastest near the nose"}
            />
            <Metric
              label="Steps"
              value={`${count(r.points.length - 1)} solved`}
              detail={`${count(sum.failed_solves)} failed and halved · ${fixed(sum.elapsed_ms / 1000, 1)} s · ${count(sum.monitored)} buses watched`}
            />
          </div>
          {sum.notes.length > 0 && (
            <div className="px-3 pt-2 flex flex-col gap-1">
              {sum.notes.map((n) => (
                <Notice key={n}>{n}</Notice>
              ))}
            </div>
          )}
          <div className="px-3 pt-3">
            <PvChart results={r} onPick={pickBus} />
          </div>
          <div className="flex items-center gap-2 px-3 pt-3">
            <div className="flex-1 min-w-0">
              <Tabs<View>
                label="PV curve results"
                value={view}
                onChange={setView}
                tabs={[
                  { id: "curves", label: "Buses", count: r.curves.length },
                  { id: "points", label: "Steps", count: r.points.length },
                ]}
              />
            </div>
            <Button type="button" onClick={exportCsv} title="Save every step with each bus's voltage as CSV">
              <Download size={13} /> CSV
            </Button>
          </div>
          <div className="px-3 py-2 h-72">
            {view === "curves" ? (
              <DataTable rows={r.curves} columns={curveCols} rowKey={(x) => x.bus} label="Buses with a curve" />
            ) : (
              <DataTable rows={r.points} columns={pointColumns} rowKey={(p) => p.transfer_mw} label="Steps of the PV curve" />
            )}
          </div>
          <p className="px-3 pb-2 text-[11px] text-slate-500">
            {sum.limited_by === "collapse"
              ? `The nose lies between ${fixed(sum.nose_mw, 1)} and ${fixed(sum.nose_mw + sum.resolution_mw, 1)} MW. `
              : ""}
            First step {fixed(sum.step_mw, 1)} MW; {count(sum.source.units)} source and {count(sum.sink.units)} sink units. Curves of the
            buses whose voltage falls furthest.
          </p>
        </div>
      )}
    </div>
  );
}
