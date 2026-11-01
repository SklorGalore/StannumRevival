// Secure transfer limits: how far power can move from a source to a sink
// with every contingency still secure. The study is stannum-core's
// (secure.rs): every contingency solved in AC at each step, major steps
// until a new problem, then minor steps to pin it down. The contingencies
// and monitors are the contingency setup's. This sets it up, then draws any
// recorded channel (a bus's PV curve, a flow) for the base case and the
// contingencies picked.
import { useEffect, useMemo, useState } from "react";
import { Download, Play, Plus, ShieldCheck, Square, Trash2 } from "lucide-react";
import { QUANTITIES, channelElement, curveRows, useSecure } from "../../store/secureStore";
import { useContingencies } from "../../store/contingencyStore";
import { validPositive } from "../../store/pvStore";
import { busList, type Side } from "../../store/transferStore";
import { saveTextExport } from "../../lib/analysis/api";
import { useCaseGroups } from "../../ui/useCaseGroups";
import type { SolveReport, SolveSettings } from "../../lib/analysis/types";
import type { ChannelInfo, Quantity, Scaling, SecureContingency, SecurePoint, SecureViolation } from "../../lib/analysis/commands.gen";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import { Button, EmptyState, ErrorBanner, Metric, Notice, Tabs, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { ContingencySetup, setupSummary } from "./ContingencySetup";
import { SINK_SCALINGS, SOURCE_SCALINGS, SidePicker } from "./PvTab";
import { SecureChart } from "./SecureChart";

const LIMITED: Record<string, string> = {
  thermal: "A branch or interface overload",
  voltage: "A bus voltage outside its limits",
  not_converged: "A contingency stops solving",
  base_collapse: "The base case stops solving: its nose",
  generation: "The source or sink ran out of room",
  max_transfer: "Reached the largest transfer asked for",
  max_points: "Reached the most steps",
  cancelled: "Cancelled",
};

const STOPPED: Record<string, string> = {
  violation: "at the first problem",
  base_collapse: "at the base case's nose",
  generation: "when the source or sink ran out of room",
  max_transfer: "at the largest transfer asked for",
  max_points: "at the most steps",
  cancelled: "when cancelled",
};

const KINDS: Record<string, string> = {
  branch: "Overload",
  voltage: "Voltage",
  interface: "Interface",
  not_converged: "No solution",
};

type View = "contingencies" | "problems" | "steps" | "channels";

function mw(v: number | null | undefined): string {
  return v === null || v === undefined ? "—" : `${fixed(v, 1)} MW`;
}

function channelLabel(c: ChannelInfo): string {
  const q = QUANTITIES.find((x) => x.value === c.quantity);
  return `${c.name} · ${q?.label ?? c.quantity}`;
}

function contingencyColumns(picks: number[], toggle: (i: number) => void): Column<SecureContingency>[] {
  return [
    {
      key: "plot",
      header: "Plot",
      value: (c) => (picks.includes(c.index) || c.index === 0 ? 1 : 0),
      render: (c) => (
        <input
          type="checkbox"
          checked={c.index === 0 || picks.includes(c.index)}
          disabled={c.index === 0}
          aria-label={`Plot ${c.name}`}
          onChange={() => toggle(c.index)}
        />
      ),
    },
    { key: "name", header: "Contingency", value: (c) => c.name },
    {
      key: "limit",
      header: "Its limit (MW)",
      title: "The last clean transfer before this contingency's first problem",
      value: (c) => c.limit_mw ?? null,
      render: (c) => (c.limit_mw === null || c.limit_mw === undefined ? <span className="text-slate-400">—</span> : fixed(c.limit_mw, 1)),
      align: "right",
    },
    {
      key: "by",
      header: "Limited by",
      value: (c) => (c.failed_at_start ? "fails with no transfer" : c.limited_by ? (KINDS[c.limited_by] ?? c.limited_by) : ""),
      render: (c) =>
        c.failed_at_start ? (
          <span className="text-slate-500">Fails with no transfer (left out)</span>
        ) : c.limited_by ? (
          <span className={c.limited_by === "not_converged" ? "text-red-700" : "text-amber-700"}>{KINDS[c.limited_by] ?? c.limited_by}</span>
        ) : (
          <span className="text-slate-400">—</span>
        ),
    },
    { key: "element", header: "Element", value: (c) => c.limiting_element ?? "" },
  ];
}

const violationColumns: Column<SecureViolation>[] = [
  { key: "at", header: "At (MW)", value: (v) => v.transfer_mw, render: (v) => fixed(v.transfer_mw, 1), align: "right" },
  {
    key: "clean",
    header: "Clean at (MW)",
    title: "The last step before it, where it wasn't there",
    value: (v) => v.clean_mw,
    render: (v) => fixed(v.clean_mw, 1),
    align: "right",
  },
  { key: "kind", header: "Problem", value: (v) => KINDS[v.kind] ?? v.kind },
  { key: "contingency", header: "Contingency", value: (v) => v.contingency },
  { key: "element", header: "Element", value: (v) => v.element },
  {
    key: "value",
    header: "Value",
    value: (v) => v.value ?? null,
    render: (v) => (v.value === null || v.value === undefined ? "—" : fixed(v.value, v.kind === "voltage" ? 4 : 1)),
    align: "right",
  },
  {
    key: "limit",
    header: "Limit",
    value: (v) => v.limit ?? null,
    render: (v) => (v.limit === null || v.limit === undefined ? "—" : fixed(v.limit, v.kind === "voltage" ? 3 : 0)),
    align: "right",
  },
  {
    key: "stops",
    header: "Counts",
    title: "Whether this kind of problem stops the run, as set up",
    value: (v) => (v.stops ? "yes" : "no"),
    render: (v) => (v.stops ? "Yes" : <span className="text-slate-400">No</span>),
  },
];

const pointColumns: Column<SecurePoint>[] = [
  { key: "transfer", header: "Transfer (MW)", value: (p) => p.transfer_mw, render: (p) => fixed(p.transfer_mw, 1), align: "right" },
  { key: "minor", header: "Step", value: (p) => (p.minor ? "minor" : "major"), render: (p) => (p.minor ? "Minor" : "Major") },
  {
    key: "vmin",
    header: "Lowest V (p.u.)",
    title: "The lowest recorded bus voltage in the base case",
    value: (p) => p.base_min_vm_pu ?? null,
    render: (p) => (p.base_min_vm_pu === null || p.base_min_vm_pu === undefined ? "—" : fixed(p.base_min_vm_pu, 4)),
    align: "right",
  },
  { key: "solved", header: "Solved", value: (p) => p.solved, align: "right" },
  { key: "nc", header: "No solution", value: (p) => p.not_converged, align: "right" },
  { key: "thermal", header: "With overloads", value: (p) => p.with_thermal, align: "right" },
  { key: "voltage", header: "With voltage problems", value: (p) => p.with_voltage, align: "right" },
  { key: "new", header: "New problems", value: (p) => p.new_problems, align: "right" },
];

const channelColumns = (pick: (k: number) => void, shown: number): Column<ChannelInfo & { k: number }>[] => [
  {
    key: "name",
    header: "Channel",
    value: (c) => channelLabel(c),
    render: (c) => (
      <button type="button" className={`text-left hover:underline ${c.k === shown ? "font-medium text-slate-900" : "text-teal-700"}`} onClick={() => pick(c.k)}>
        {channelLabel(c)}
      </button>
    ),
  },
  { key: "element", header: "Element", value: (c) => c.element },
  { key: "unit", header: "Unit", value: (c) => c.unit },
  { key: "low", header: "Low", value: (c) => c.low ?? null, render: (c) => (c.low === null || c.low === undefined ? "—" : fixed(c.low, 2)), align: "right" },
  { key: "high", header: "High", value: (c) => c.high ?? null, render: (c) => (c.high === null || c.high === undefined ? "—" : fixed(c.high, 2)), align: "right" },
];

export function SecureTab({
  report,
  settings,
  scope,
  onPick,
}: {
  report: SolveReport | null;
  settings: SolveSettings;
  scope: { elements: string[]; buses: number } | null;
  onPick: (element: string) => void;
}) {
  const s = useSecure();
  const c = s.choice;
  const contingencyChoice = useContingencies((x) => x.choice);
  const groups = useCaseGroups();
  const areaNames = useMemo(() => new Map((report?.areas ?? []).map((a) => [a.number, a.name.trim()])), [report]);
  const [view, setView] = useState<View>("contingencies");
  const [setupOpen, setSetupOpen] = useState(false);
  const [newQuantity, setNewQuantity] = useState<Quantity>("mw");
  const [newElement, setNewElement] = useState("");

  // Start with the case's first area as the source and the last as the sink.
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
  const fromLists = contingencyChoice.source === "lists";
  const blocked = !acReady
    ? "Solve the case with the AC model first."
    : noBuses(c.sink) || (!c.allSource && noBuses(c.source))
      ? "List the sink's (and source's) buses."
      : !validPositive(c.majorMw) || !validPositive(c.minorMw) || !validPositive(c.maxMw)
        ? "Steps and the largest transfer must be numbers above zero, or blank."
        : fromLists && !contingencyChoice.listsPath
          ? "Pick the lists file in the contingency setup."
          : null;
  const run = () => {
    if (blocked || s.running) return;
    setSetupOpen(false);
    void s.run(settings, scope ? scope.elements : null);
  };
  const contingencyCols = useMemo(() => contingencyColumns(s.picks, (i) => void s.togglePick(i)), [s.picks]); // eslint-disable-line react-hooks/exhaustive-deps
  const channels = useMemo(() => (r?.channels ?? []).map((ch, k) => ({ ...ch, k })), [r]);
  const channelCols = useMemo(() => channelColumns((k) => void s.show(k), s.channel), [s.channel]); // eslint-disable-line react-hooks/exhaustive-deps

  function addChannel() {
    const element = channelElement(newQuantity, newElement);
    if (!element) return;
    if (c.channels.some((x) => x.quantity === newQuantity && x.element === element)) return;
    s.setChoice({ channels: [...c.channels, { quantity: newQuantity, element }] });
    setNewElement("");
  }

  async function exportCsv() {
    if (!s.curves) return;
    await saveTextExport("secure-transfer-channel.csv", toCsv(curveRows(s.curves)), { name: "CSV", extensions: ["csv"] });
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
  const p = s.progress;
  const quantity = QUANTITIES.find((q) => q.value === newQuantity)!;

  return (
    <div className="h-full flex flex-col" data-testid="secure-tab">
      <form
        className="flex flex-col gap-2 px-3 py-2 border-b border-slate-200 bg-white text-xs"
        onSubmit={(e) => {
          e.preventDefault();
          run();
        }}
      >
        <div className="flex flex-wrap items-end gap-x-4 gap-y-2">
          <div className={label}>
            Source
            <div className="flex items-center gap-1">
              <select
                className={inputClass}
                value={c.allSource ? "all" : "side"}
                onChange={(e) => s.setChoice({ allSource: e.target.value === "all" })}
                aria-label="Source"
                data-testid="secure-source-kind"
              >
                <option value="side">A subsystem…</option>
                <option value="all">Every bus</option>
              </select>
              {!c.allSource && (
                <SidePicker label="Source" side={c.source} onChange={(source) => s.setChoice({ source })} groups={groups} areaNames={areaNames} testId="secure-source" />
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
          <div className={label}>
            Sink
            <div className="flex items-center gap-1">
              <SidePicker label="Sink" side={c.sink} onChange={(sink) => s.setChoice({ sink })} groups={groups} areaNames={areaNames} testId="secure-sink" />
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
            </div>
          </div>
          <label className={label}>
            Major step (MW)
            {numberInput(c.majorMw, (majorMw) => s.setChoice({ majorMw }), "Major step, MW", "The step while nothing new is found. Blank for 5% of what the sink starts with", "Auto")}
          </label>
          <label className={label}>
            Minor step (MW)
            {numberInput(c.minorMw, (minorMw) => s.setChoice({ minorMw }), "Minor step, MW", "The step that pins down where a problem starts. Blank for a tenth of the major step", "Auto")}
          </label>
          <label className={label}>
            Stop at (MW)
            {numberInput(c.maxMw, (maxMw) => s.setChoice({ maxMw }), "Stop at, MW", "The largest transfer. Blank to go on", "None")}
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <span className="text-[11px] font-medium text-slate-600">Problems that stop it:</span>
          <label className="flex items-center gap-1.5" title="New branch overloads, and monitored interfaces over their limit">
            <input type="checkbox" checked={c.stopThermal} onChange={(e) => s.setChoice({ stopThermal: e.target.checked })} />
            Thermal
          </label>
          <label className="flex items-center gap-1.5" title="New bus voltages outside their limits">
            <input type="checkbox" checked={c.stopVoltage} onChange={(e) => s.setChoice({ stopVoltage: e.target.checked })} />
            Voltage
          </label>
          <span className="text-slate-400" title="A contingency, or the base case, that stops solving always counts">
            (a failed solve always does)
          </span>
          <label className="flex items-center gap-1.5 ml-2" title="Off: pin down the first problem, then carry on to the nose and log every problem met">
            <input type="checkbox" checked={c.stopAtViolation} onChange={(e) => s.setChoice({ stopAtViolation: e.target.checked })} data-testid="secure-stop-first" />
            Stop at the first problem
          </label>
          <label className="flex items-center gap-1.5" title="Stop when the source's generators reach Pmax, the sink's reach Pmin, or a side's loads reach zero">
            <input type="checkbox" checked={c.respectGeneration} onChange={(e) => s.setChoice({ respectGeneration: e.target.checked })} />
            Stop at generator limits
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[11px] font-medium text-slate-600">Contingencies:</span>
          <span className="text-slate-600 truncate max-w-[60ch]" title={setupSummary(contingencyChoice, !!scope)} data-testid="secure-setup-summary">
            {setupSummary(contingencyChoice, !!scope).replace(/ · (linear screen|AC for every outage)[^·]*$/, "")}
          </span>
          <Button type="button" onClick={() => setSetupOpen(true)} data-testid="secure-setup">
            Contingency setup…
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[11px] font-medium text-slate-600" title="The sink's bus voltages (or the voltage monitors' buses) are always recorded">
            Also record:
          </span>
          {c.channels.map((ch) => (
            <span key={`${ch.quantity} ${ch.element}`} className="inline-flex items-center gap-1 rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5">
              {QUANTITIES.find((q) => q.value === ch.quantity)?.label} {ch.element}
              <button
                type="button"
                className="text-slate-400 hover:text-red-600"
                aria-label={`Stop recording ${ch.quantity} of ${ch.element}`}
                onClick={() => s.setChoice({ channels: c.channels.filter((x) => x !== ch) })}
              >
                <Trash2 size={11} />
              </button>
            </span>
          ))}
          <select className={inputClass} value={newQuantity} onChange={(e) => setNewQuantity(e.target.value as Quantity)} aria-label="Quantity to record">
            {QUANTITIES.map((q) => (
              <option key={q.value} value={q.value}>
                {q.label}
              </option>
            ))}
          </select>
          <input
            className={`${inputClass} w-44`}
            value={newElement}
            placeholder={quantity.placeholder}
            aria-label={`The ${quantity.element} to record`}
            onChange={(e) => setNewElement(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                addChannel();
              }
            }}
          />
          <Button type="button" onClick={addChannel} disabled={!newElement.trim()} title="Record this at every step for every contingency">
            <Plus size={12} /> Add
          </Button>
          <div className="flex items-center gap-2 ml-auto">
            {s.running ? (
              <Button type="button" onClick={s.cancel} data-testid="secure-cancel">
                <Square size={13} /> Cancel
              </Button>
            ) : (
              <Button type="submit" variant="primary" disabled={!!blocked} title={blocked ?? undefined} data-testid="secure-run">
                <Play size={13} /> Find the secure limit
              </Button>
            )}
          </div>
        </div>
      </form>
      {setupOpen && (
        <ContingencySetup
          report={report}
          solve={settings}
          scope={scope}
          canRun={!blocked && !s.running}
          runTitle={blocked ?? "Close the setup and find the secure transfer limit"}
          onRun={run}
          onClose={() => setSetupOpen(false)}
        />
      )}
      {s.running && (
        <div className="flex items-center gap-2 px-3 py-1 text-[11px] text-slate-600 border-b border-slate-100" role="status" data-testid="secure-progress">
          <span className="inline-block h-1.5 w-32 overflow-hidden rounded bg-slate-100">
            <span className="block h-full bg-teal-600 transition-all" style={{ width: p && p.total ? `${(100 * p.done) / p.total}%` : "10%" }} />
          </span>
          {p && p.total
            ? `Step ${count(p.steps + 1)} at ${fixed(p.transfer_mw, 1)} MW: ${count(p.done)} of ${count(p.total)} contingencies solved…`
            : "Solving the base case…"}
        </div>
      )}
      {s.error && (
        <div className="px-3 pt-2">
          <ErrorBanner>{s.error}</ErrorBanner>
        </div>
      )}
      {!r || !sum ? (
        <EmptyState icon={<ShieldCheck size={22} />} title="Secure transfer limits">
          {blocked && !acReady ? (
            blocked
          ) : (
            <>
              How far power can move from the source to the sink with every contingency still secure. At each step the base case and every
              contingency are solved in AC; a new problem is pinned down with the minor step. Every step's values of the recorded channels are
              kept, so you can draw any bus's PV curve, or any flow, under every contingency. Every contingency is solved at every step, so
              narrow the monitors on a big case.
            </>
          )}
        </EmptyState>
      ) : (
        <div className="flex-1 min-h-0 overflow-auto">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-2 px-3 pt-2" data-testid="secure-summary">
            <Metric
              label="Secure limit"
              value={`+${mw(sum.limit_mw)}`}
              tone={sum.limiting_contingency ? "bad" : "good"}
              detail={`${LIMITED[sum.limited_by] ?? sum.limited_by}${sum.resolution_mw > 0 ? ` · within ${fixed(sum.resolution_mw, 1)} MW` : ""}`}
            />
            <Metric
              label="Limiting contingency"
              value={sum.limiting_contingency ?? "—"}
              detail={sum.limiting_element ? `On ${sum.limiting_element}` : sum.limiting_contingency ? "Stops solving" : "None found"}
            />
            <Metric
              label="Studied up to"
              value={`+${mw(sum.last_transfer_mw)}`}
              detail={`Stopped ${STOPPED[sum.stopped_by] ?? sum.stopped_by}${sum.nose_mw !== null && sum.nose_mw !== undefined ? ` · base nose ${mw(sum.nose_mw)}` : ""}`}
            />
            <Metric
              label="Solves"
              value={`${count(sum.contingency_solves)} contingency`}
              detail={`${count(r.points.length)} steps · ${count(sum.contingencies)} contingencies · ${fixed(sum.elapsed_ms / 1000, 1)} s · ${
                sum.results_bytes > 1e6 ? `${fixed(sum.results_bytes / 1e6, 1)} MB` : `${fixed(sum.results_bytes / 1e3, 0)} kB`
              } of results`}
            />
          </div>
          {sum.notes.length > 0 && (
            <div className="px-3 pt-2 flex flex-col gap-1">
              {sum.notes.map((n) => (
                <Notice key={n}>{n}</Notice>
              ))}
            </div>
          )}
          {r.channels.length > 0 && (
            <div className="px-3 pt-3">
              <div className="flex items-center gap-2 pb-1 text-xs">
                <label className="flex items-center gap-1.5 text-[11px] font-medium text-slate-600">
                  Channel
                  <select
                    className={`${inputClass} max-w-96`}
                    value={s.channel}
                    onChange={(e) => void s.show(Number(e.target.value))}
                    aria-label="Channel to draw"
                    data-testid="secure-channel"
                  >
                    {r.channels.map((ch, k) => (
                      <option key={ch.column} value={k}>
                        {channelLabel(ch)}
                      </option>
                    ))}
                  </select>
                </label>
                <span className="text-[11px] text-slate-500">
                  Base case and {count(s.picks.length)} contingenc{s.picks.length === 1 ? "y" : "ies"}; tick more in the list below.
                </span>
                <Button type="button" className="ml-auto" onClick={exportCsv} disabled={!s.curves} title="Save this channel's curves as CSV">
                  <Download size={13} /> CSV
                </Button>
              </div>
              {s.curves && <SecureChart curves={s.curves} summary={sum} />}
            </div>
          )}
          <div className="px-3 pt-3">
            <Tabs<View>
              label="Secure transfer results"
              value={view}
              onChange={setView}
              tabs={[
                { id: "contingencies", label: "Contingencies", count: r.contingencies.length },
                { id: "problems", label: "Problems", count: r.violations.length },
                { id: "steps", label: "Steps", count: r.points.length },
                { id: "channels", label: "Channels", count: r.channels.length },
              ]}
            />
          </div>
          <div className="px-3 py-2 h-80">
            {view === "contingencies" ? (
              <DataTable rows={r.contingencies} columns={contingencyCols} rowKey={(x) => x.index} label="Each contingency's limit" />
            ) : view === "problems" ? (
              <DataTable
                rows={r.violations}
                columns={violationColumns}
                rowKey={(v) => `${v.contingency_index} ${v.kind} ${v.element_id}`}
                label="Problems, the first time each was seen"
                onRowClick={(v) => {
                  if (v.element_id && !v.element_id.startsWith("interface:")) onPick(v.element_id);
                }}
              />
            ) : view === "steps" ? (
              <DataTable rows={r.points} columns={pointColumns} rowKey={(x) => `${x.transfer_mw}`} label="Steps" />
            ) : (
              <DataTable rows={channels} columns={channelCols} rowKey={(x) => x.column} label="Recorded channels" />
            )}
          </div>
          <p className="px-3 pb-2 text-[11px] text-slate-500">
            Major step {fixed(sum.major_step_mw, 1)} MW, minor {fixed(sum.minor_step_mw, 1)} MW. {count(sum.existing_problems)} problems already there
            with no transfer don't count unless they get worse. Results file: {sum.results_file}
          </p>
        </div>
      )}
    </div>
  );
}
