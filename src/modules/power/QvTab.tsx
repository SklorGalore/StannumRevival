// QV curves (reactive power margin): how much reactive power each chosen
// bus can give up before its voltage collapses. The study is stannum-core's
// (qv.rs): a condenser holds each bus at one voltage after another with
// full AC solves. This sets it up and draws the curves.
import { useMemo } from "react";
import { Activity, Download, Play, Square } from "lucide-react";
import { MAX_QV_BUSES, lowestVoltageBuses, qvRows, qvSettings, useQv } from "../../store/qvStore";
import { validPositive } from "../../store/pvStore";
import { busList } from "../../store/transferStore";
import { saveCsvExport } from "../../lib/analysis/api";
import { useCaseGroups } from "../../ui/useCaseGroups";
import type { SolveReport } from "../../lib/analysis/types";
import type { QvCurve } from "../../lib/analysis/commands.gen";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import { Button, EmptyState, ErrorBanner, Metric, Notice, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { QvChart } from "./QvChart";
import { curveLabel } from "./PvChart";

const STOPPED: Record<string, string> = {
  collapse: "Lower voltages didn't solve",
  v_low: "Reached the lowest voltage",
  max_points: "Reached the most solves",
  cancelled: "Cancelled",
  failed: "Didn't solve with the condenser",
};

/** How many buses "Add lowest voltages" adds. */
const PICK = 5;

function curveColumns(onPick: (bus: number) => void): Column<QvCurve>[] {
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
    { key: "base", header: "Solved V (p.u.)", value: (c) => c.base_vm_pu, render: (c) => fixed(c.base_vm_pu, 4), align: "right" },
    {
      key: "margin",
      header: "Margin (Mvar)",
      title: "How far the bottom of the curve lies below zero: the reactive load the bus can take before its voltage collapses",
      value: (c) => c.margin_mvar,
      render: (c) =>
        c.limited_by === "failed" ? (
          <span className="text-slate-400">—</span>
        ) : (
          <span className={c.margin_mvar < 0 ? "text-red-700 font-medium" : ""}>
            {c.bottom_found ? "" : "≥ "}
            {fixed(c.margin_mvar, 1)}
          </span>
        ),
      align: "right",
    },
    {
      key: "vbottom",
      header: "V at bottom (p.u.)",
      title: "The voltage at the bottom of the curve",
      value: (c) => c.v_at_q_min_pu,
      render: (c) => fixed(c.v_at_q_min_pu, 3),
      align: "right",
    },
    {
      key: "low",
      header: "Lowest V (p.u.)",
      title: "The lowest voltage that solved",
      value: (c) => c.v_pu[0] ?? null,
      render: (c) => (c.v_pu.length ? fixed(c.v_pu[0], 3) : "—"),
      align: "right",
    },
    { key: "stop", header: "Stopped", value: (c) => STOPPED[c.limited_by] ?? c.limited_by },
    { key: "solves", header: "Solves", value: (c) => c.solves, align: "right" },
  ];
}

export function QvTab({ report, onPick }: { report: SolveReport | null; onPick: (element: string) => void }) {
  const s = useQv();
  const c = s.choice;
  const groups = useCaseGroups();
  const areaNames = useMemo(() => new Map((report?.areas ?? []).map((a) => [a.number, a.name.trim()])), [report]);
  const pickBus = (bus: number) => onPick(`bus:${bus}`);
  const curveCols = useMemo(() => curveColumns(pickBus), [onPick]); // eslint-disable-line react-hooks/exhaustive-deps
  const areas = groups?.area ?? [];

  const r = s.results;
  const sum = r?.summary;
  const buses = busList(c.buses);
  const acReady = !!report && report.model === "ac" && report.converged;
  const blocked = !acReady
    ? "Solve the case with the AC model first."
    : buses.length === 0
      ? "List the buses to sweep."
      : buses.length > MAX_QV_BUSES
        ? `Sweep ${MAX_QV_BUSES} buses at most.`
        : !validPositive(c.vHigh) || !validPositive(c.vLow) || !validPositive(c.step)
          ? "Voltages and the step must be numbers above zero, or blank."
          : null;

  function addLowest(area: number | null) {
    if (!report) return;
    const picked = lowestVoltageBuses(report.buses, area, PICK + buses.length).filter((b) => !buses.includes(b));
    s.setChoice({ buses: [...buses, ...picked.slice(0, PICK)].join(", ") });
  }

  async function exportCsv() {
    if (!r) return;
    await saveCsvExport("qv-curves.csv", "QV curves", { ...qvSettings(c) }, toCsv(qvRows(r)));
  }

  const numberInput = (value: string, onChange: (v: string) => void, aria: string, title: string, placeholder: string) => (
    <input
      className={`${inputClass} w-16`}
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
  const done = s.progress?.done ?? 0;

  return (
    <div className="h-full flex flex-col" data-testid="qv-tab">
      <form
        className="flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2 border-b border-slate-200 bg-white text-xs"
        onSubmit={(e) => {
          e.preventDefault();
          if (!blocked && !s.running) void s.run();
        }}
      >
        <label className={`${label} flex-1 min-w-56`}>
          Buses
          <input
            className={inputClass}
            value={c.buses}
            placeholder="7314, 7340, 7050"
            aria-label="Buses to sweep"
            title="Bus numbers, each swept on its own"
            onChange={(e) => s.setChoice({ buses: e.target.value })}
            data-testid="qv-buses"
          />
        </label>
        <label className={label}>
          Add the {PICK} lowest voltages
          <select
            className={inputClass}
            value=""
            aria-label={`Add the ${PICK} lowest-voltage load buses`}
            title="Add the load buses with the lowest solved voltage"
            disabled={!report}
            onChange={(e) => {
              if (e.target.value === "") return;
              addLowest(e.target.value === "all" ? null : Number(e.target.value));
            }}
            data-testid="qv-add-lowest"
          >
            <option value="">Pick…</option>
            <option value="all">In the whole case</option>
            {areas.map(([v]) => (
              <option key={v} value={v}>
                In area {v}
                {areaNames.get(v) ? ` ${areaNames.get(v)}` : ""}
              </option>
            ))}
          </select>
        </label>
        <label className={label}>
          High (p.u.)
          {numberInput(c.vHigh, (vHigh) => s.setChoice({ vHigh }), "Highest voltage, p.u.", "The highest voltage set. Blank for 1.1 p.u.", "1.1")}
        </label>
        <label className={label}>
          Low (p.u.)
          {numberInput(
            c.vLow,
            (vLow) => s.setChoice({ vLow }),
            "Lowest voltage, p.u.",
            "The lowest voltage tried; the sweep usually stops before it, when the voltage is too low to solve. Blank for 0.3 p.u.",
            "0.3",
          )}
        </label>
        <label className={label}>
          Step (p.u.)
          {numberInput(c.step, (step) => s.setChoice({ step }), "Voltage step, p.u.", "The voltage step; halved near the end. Blank for 0.01 p.u.", "0.01")}
        </label>
        <div className="flex items-center gap-2 ml-auto">
          {s.running ? (
            <Button type="button" onClick={s.cancel} data-testid="qv-cancel">
              <Square size={13} /> Cancel
            </Button>
          ) : (
            <Button type="submit" variant="primary" disabled={!!blocked} title={blocked ?? undefined} data-testid="qv-run">
              <Play size={13} /> Draw QV curves
            </Button>
          )}
        </div>
      </form>
      {s.running && (
        <div className="flex items-center gap-2 px-3 py-1 text-[11px] text-slate-600 border-b border-slate-100" role="status">
          <span className="inline-block h-1.5 w-24 overflow-hidden rounded bg-slate-100">
            <span className="block h-full bg-teal-600 transition-all" style={{ width: `${(100 * done) / Math.max(1, buses.length)}%` }} />
          </span>
          Sweeping bus {count(Math.min(done + 1, buses.length))} of {count(buses.length)}…
        </div>
      )}
      {s.error && (
        <div className="px-3 pt-2">
          <ErrorBanner>{s.error}</ErrorBanner>
        </div>
      )}
      {!r || !sum ? (
        <EmptyState icon={<Activity size={22} />} title="QV curves">
          {blocked && !acReady ? (
            blocked
          ) : (
            <>
              How much reactive power each bus can give up before its voltage collapses. A condenser holds the bus at one voltage after
              another, each a full AC solve with every control and reactive limit of your last solve; the bottom of its curve is the margin.
            </>
          )}
        </EmptyState>
      ) : (
        <div className="flex-1 min-h-0 overflow-auto">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-2 px-3 pt-2" data-testid="qv-summary">
            <Metric
              label="Weakest bus"
              value={sum.weakest_bus ? `${sum.weakest_bus}${sum.weakest_bus_name.trim() ? ` ${sum.weakest_bus_name.trim()}` : ""}` : "—"}
              detail="The smallest reactive power margin"
            />
            <Metric
              label="Smallest margin"
              value={sum.weakest_margin_mvar === null || sum.weakest_margin_mvar === undefined ? "—" : `${fixed(sum.weakest_margin_mvar, 1)} Mvar`}
              tone={sum.weakest_margin_mvar !== null && sum.weakest_margin_mvar !== undefined && sum.weakest_margin_mvar < 0 ? "bad" : "neutral"}
              detail={
                weakest
                  ? weakest.margin_mvar < 0
                    ? "Below zero: the bus can't hold its voltage unaided"
                    : `Bottom at ${fixed(weakest.v_at_q_min_pu, 2)} p.u.`
                  : "How far the bottom lies below zero"
              }
            />
            <Metric label="Buses" value={count(sum.buses)} detail={`Swept from ${fixed(sum.v_high_pu, 2)} p.u. down, ${fixed(sum.step_pu, 3)} p.u. apart`} />
            <Metric
              label="Solves"
              value={count(sum.solves)}
              detail={`${count(sum.failed_solves)} failed and halved · ${fixed(sum.elapsed_ms / 1000, 1)} s`}
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
            <QvChart results={r} onPick={pickBus} />
          </div>
          <div className="flex items-center gap-2 px-3 pt-3">
            <h3 className="flex-1 text-xs font-medium text-slate-700">Margins by bus</h3>
            <Button type="button" onClick={exportCsv} title="Save every point of every curve as CSV">
              <Download size={13} /> CSV
            </Button>
          </div>
          <div className="px-3 py-2 h-64">
            <DataTable rows={r.curves} columns={curveCols} rowKey={(x) => x.bus} label="QV margins by bus" />
          </div>
        </div>
      )}
    </div>
  );
}
