// N-1-1: one element out, the system adjusted, then every contingency on
// top. The run is stannum-core's (n11.rs): each first outage solved in AC,
// a redispatch back within the limit, then an N-1 run from there. This
// sets it up and lists the first outages and the worst pairs.
import { useMemo, useState } from "react";
import { Download, Layers, Play, Square } from "lucide-react";
import { n11Settings, pairRows, useN11 } from "../../store/n11Store";
import { saveCsvExport } from "../../lib/analysis/api";
import type { SolveReport } from "../../lib/analysis/types";
import type { FirstOutage, PairRow } from "../../lib/analysis/commands.gen";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import { Button, EmptyState, ErrorBanner, Metric, Notice, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";

const ADJUSTED: Record<string, string> = {
  relieved: "Relieved",
  partly: "Partly",
  nothing_to_do: "Not needed",
  failed: "Failed",
};

function pct(v: number | null | undefined, limit: number) {
  if (v === null || v === undefined) return <span className="text-slate-400">—</span>;
  return <span className={v > limit ? "text-red-700 font-medium" : ""}>{fixed(v, 1)}%</span>;
}

function firstColumns(limit: number, onPick: (element: string) => void): Column<FirstOutage>[] {
  return [
    {
      key: "name",
      header: "First outage",
      value: (f) => f.name,
      render: (f) => (
        <button type="button" className="text-left text-teal-700 hover:underline" onClick={() => onPick(f.element)}>
          {f.name}
        </button>
      ),
    },
    {
      key: "alone",
      header: "Alone",
      title: "The worst loading the first outage leaves, before any adjustment",
      value: (f) => (f.converged ? (f.worst?.pct ?? 0) : Number.POSITIVE_INFINITY),
      render: (f) => (f.converged ? pct(f.worst?.pct, limit) : <span className="text-red-700 font-medium">No solve</span>),
      align: "right",
    },
    {
      key: "adjust",
      header: "Adjustment",
      title: "What the redispatch did after the first outage",
      value: (f) => f.adjustment?.status ?? "",
      render: (f) =>
        f.adjustment ? (
          <span className={f.adjustment.status === "partly" || f.adjustment.status === "failed" ? "text-amber-700" : ""}>
            {ADJUSTED[f.adjustment.status] ?? f.adjustment.status}
            {f.adjustment.units_moved > 0 ? ` · ${fixed(f.adjustment.moved_mw, 1)} MW` : ""}
          </span>
        ) : (
          <span className="text-slate-400">—</span>
        ),
    },
    {
      key: "after",
      header: "After adjusting",
      title: "The most loaded branch after the redispatch",
      value: (f) => f.adjustment?.worst_after_pct ?? null,
      render: (f) => pct(f.adjustment?.worst_after_pct, limit),
      align: "right",
    },
    { key: "seconds", header: "Seconds", value: (f) => f.seconds, align: "right" },
    {
      key: "problems",
      header: "With problems",
      title: "Second outages that overload a branch, put a bus outside its limits, or don't solve",
      value: (f) => f.seconds_with_problems,
      render: (f) => <span className={f.seconds_with_problems > 0 ? "text-red-700 font-medium" : ""}>{count(f.seconds_with_problems)}</span>,
      align: "right",
    },
    { key: "worst", header: "Worst second outage", value: (f) => f.worst_second ?? "" },
  ];
}

function pairColumns(limit: number, onPick: (element: string) => void): Column<PairRow>[] {
  const link = (element: string, name: string) => (
    <button type="button" className="text-left text-teal-700 hover:underline" onClick={() => onPick(element)}>
      {name}
    </button>
  );
  return [
    { key: "rank", header: "#", value: (p) => p.rank, align: "right" },
    { key: "first", header: "First", value: (p) => p.first_name, render: (p) => link(p.first, p.first_name) },
    { key: "second", header: "Then", value: (p) => p.second_name, render: (p) => link(p.second, p.second_name) },
    {
      key: "worst",
      header: "Worst branch",
      value: (p) => p.worst?.name ?? "",
      render: (p) => (p.worst ? link(p.worst.element, p.worst.name) : <span className="text-slate-400">—</span>),
    },
    {
      key: "pct",
      header: "Loading",
      title: "The worst branch after both outages",
      value: (p) => (p.converged === false ? Number.POSITIVE_INFINITY : (p.worst?.pct ?? null)),
      render: (p) => (p.converged === false ? <span className="text-red-700 font-medium">No solve</span> : pct(p.worst?.pct, limit)),
      align: "right",
    },
    {
      key: "before",
      header: "Before",
      title: "The same branch after the first outage and the adjustment",
      value: (p) => p.worst?.base_pct ?? null,
      render: (p) => pct(p.worst?.base_pct, limit),
      align: "right",
    },
    { key: "overloads", header: "Overloads", value: (p) => p.overloads, align: "right" },
    {
      key: "voltage",
      header: "Voltage",
      title: "Buses outside their voltage limits, new or worse",
      value: (p) => p.voltage_violations,
      render: (p) =>
        p.worst_voltage ? (
          <span className="text-red-700" title={`Worst: bus ${p.worst_voltage.bus} at ${fixed(p.worst_voltage.vm, 3)} p.u.`}>
            {count(p.voltage_violations)}
          </span>
        ) : (
          count(p.voltage_violations)
        ),
      align: "right",
    },
    { key: "checked", header: "Checked", value: (p) => (p.checked === "ac" ? "AC" : "Screen") },
    { key: "note", header: "Note", value: (p) => p.note ?? "" },
  ];
}

export function N11Tab({ report, onPick }: { report: SolveReport | null; onPick: (element: string) => void }) {
  const s = useN11();
  const c = s.choice;
  const [only, setOnly] = useState<string | null>(null);
  const r = s.results;
  const sum = r?.summary;
  const limit = sum?.limit_pct ?? c.limitPct ?? 100;
  const firstCols = useMemo(() => firstColumns(limit, onPick), [limit, onPick]);
  const pairCols = useMemo(() => pairColumns(limit, onPick), [limit, onPick]);
  const acReady = !!report && report.model === "ac" && report.converged;
  const ready = n11Settings(c);
  const blocked = !acReady ? "Solve the case with the AC model first." : "problem" in ready ? ready.problem : null;
  const pairs = useMemo(() => (r ? (only ? r.pairs.filter((p) => p.first === only) : r.pairs) : []), [r, only]);
  const firsts = r?.firsts ?? [];

  async function exportCsv() {
    if (!r || !("settings" in ready)) return;
    await saveCsvExport("n-1-1.csv", "N-1-1 pairs", { ...ready.settings }, toCsv(pairRows(r.pairs)));
  }

  const label = "flex flex-col gap-1 text-[11px] font-medium text-slate-600";
  const p = s.progress;
  const firstName = only ? r?.firsts.find((f) => f.element === only)?.name : null;

  return (
    <div className="h-full flex flex-col" data-testid="n11-tab">
      <form
        className="flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2 border-b border-slate-200 bg-white text-xs"
        onSubmit={(e) => {
          e.preventDefault();
          if (!blocked && !s.running) {
            setOnly(null);
            void s.run();
          }
        }}
      >
        <label className={label}>
          Rating
          <select
            className={inputClass}
            value={c.rating}
            onChange={(e) => s.setChoice({ rating: e.target.value as typeof c.rating })}
            data-testid="n11-rating"
          >
            <option value="a">Rate A</option>
            <option value="b">Rate B</option>
            <option value="c">Rate C</option>
          </select>
        </label>
        <label className={label}>
          Limit, %
          <input
            className={`${inputClass} w-16 tabular-nums`}
            type="number"
            min={1}
            step="any"
            value={c.limitPct ?? ""}
            onChange={(e) => s.setChoice({ limitPct: e.target.value === "" ? null : Number(e.target.value) })}
            aria-label="Loading limit, percent"
            data-testid="n11-limit"
          />
        </label>
        <fieldset className="flex flex-col gap-1">
          <legend className="text-[11px] font-medium text-slate-600">Take out</legend>
          <div className="flex items-center gap-3 h-7">
            <label className="flex items-center gap-1">
              <input type="checkbox" checked={c.branches} onChange={(e) => s.setChoice({ branches: e.target.checked })} /> Branches
            </label>
            <label className="flex items-center gap-1">
              <input type="checkbox" checked={c.generators} onChange={(e) => s.setChoice({ generators: e.target.checked })} /> Generators
            </label>
          </div>
        </fieldset>
        <label className={label}>
          First outages
          <select
            className={inputClass}
            value={c.listFirst ? "list" : "all"}
            onChange={(e) => s.setChoice({ listFirst: e.target.value === "list" })}
            data-testid="n11-first-mode"
          >
            <option value="all">Every one of them</option>
            <option value="list">Only these…</option>
          </select>
        </label>
        {c.listFirst && (
          <label className={`${label} flex-1 min-w-56`}>
            First outage ids
            <input
              className={inputClass}
              value={c.first}
              placeholder="line:1:2:1, generator:3:1"
              aria-label="First outages"
              title="Element ids, comma separated; each goes out first, then every other element"
              onChange={(e) => s.setChoice({ first: e.target.value })}
              data-testid="n11-first"
            />
          </label>
        )}
        <label className={label}>
          Between the outages
          <select
            className={inputClass}
            value={c.adjustment}
            onChange={(e) => s.setChoice({ adjustment: e.target.value as typeof c.adjustment })}
            title="The system adjustment after the first outage"
            data-testid="n11-adjustment"
          >
            <option value="redispatch">Redispatch within the limit</option>
            <option value="none">No adjustment</option>
          </select>
        </label>
        <div className="flex items-center gap-2 ml-auto">
          {s.running ? (
            <Button type="button" onClick={s.cancel} data-testid="n11-cancel">
              <Square size={13} /> Cancel
            </Button>
          ) : (
            <Button type="submit" variant="primary" disabled={!!blocked} title={blocked ?? undefined} data-testid="n11-run">
              <Play size={13} /> Run N-1-1
            </Button>
          )}
        </div>
      </form>
      {s.running && (
        <div className="flex items-center gap-2 px-3 py-1 text-[11px] text-slate-600 border-b border-slate-100" role="status">
          <span className="inline-block h-1.5 w-24 overflow-hidden rounded bg-slate-100">
            <span
              className="block h-full bg-teal-600 transition-all"
              style={{ width: `${(100 * (p?.firsts_done ?? 0)) / Math.max(1, p?.firsts_total ?? 1)}%` }}
            />
          </span>
          {p && p.firsts_total > 0
            ? `First outage ${count(Math.min(p.firsts_done + 1, p.firsts_total))} of ${count(p.firsts_total)}…`
            : "Starting…"}
        </div>
      )}
      {s.error && (
        <div className="px-3 pt-2">
          <ErrorBanner>{s.error}</ErrorBanner>
        </div>
      )}
      {!r || !sum ? (
        <EmptyState icon={<Layers size={22} />} title="N-1-1">
          {!acReady ? (
            blocked
          ) : (
            <>
              One element out, the system adjusted, then every contingency on top, as for NERC TPL-001 P6 events. Each first outage is
              solved in AC with every control of your last solve; the redispatch then moves generators as little as possible to bring
              branches back within the limit, and each second outage is judged against that adjusted case.
            </>
          )}
        </EmptyState>
      ) : (
        <div className="flex-1 min-h-0 overflow-auto">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-2 px-3 pt-2" data-testid="n11-summary">
            <Metric
              label="Pairs"
              value={count(sum.pairs)}
              detail={`${count(sum.firsts)} first outages · ${count(sum.pairs_ac)} in AC · ${fixed(sum.elapsed_ms / 1000, 1)} s`}
            />
            <Metric
              label="With overloads"
              value={count(sum.pairs_with_overloads)}
              tone={sum.pairs_with_overloads > 0 ? "bad" : "good"}
              detail={sum.worst?.worst ? `Worst ${sum.worst.worst.name} at ${fixed(sum.worst.worst.pct, 1)}%` : "No branch over its limit"}
            />
            <Metric
              label="Voltage and solves"
              value={`${count(sum.pairs_with_voltage_problems)} · ${count(sum.pairs_not_converged)}`}
              tone={sum.pairs_with_voltage_problems + sum.pairs_not_converged > 0 ? "warn" : "neutral"}
              detail="Pairs with a bus outside its limits · pairs that don't solve"
            />
            <Metric
              label="Adjusted"
              value={sum.adjustment === "none" ? "—" : count(sum.firsts_adjusted)}
              tone={sum.firsts_still_over > 0 ? "warn" : "neutral"}
              detail={
                sum.adjustment === "none"
                  ? "No adjustment between the outages"
                  : `First outages redispatched · ${count(sum.firsts_still_over)} still over after`
              }
            />
          </div>
          {sum.notes.length > 0 && (
            <div className="px-3 pt-2 flex flex-col gap-1">
              {sum.notes.map((n) => (
                <Notice key={n}>{n}</Notice>
              ))}
            </div>
          )}
          <div className="flex items-center gap-2 px-3 pt-3">
            <h3 className="flex-1 text-xs font-medium text-slate-700">First outages, most second-outage problems first</h3>
          </div>
          <div className="px-3 py-2 h-56">
            <DataTable
              rows={firsts}
              columns={firstCols}
              rowKey={(f) => f.element}
              label="First outages"
              onRowClick={(f) => setOnly(only === f.element ? null : f.element)}
              selectedKey={only ?? undefined}
            />
          </div>
          <div className="flex items-center gap-2 px-3 pt-2">
            <h3 className="flex-1 text-xs font-medium text-slate-700">
              {firstName ? `Pairs after ${firstName}` : "Worst pairs"}
              {sum.pairs > sum.kept_pairs && !only ? ` (worst ${count(sum.kept_pairs)} of ${count(sum.pairs)})` : ""}
            </h3>
            {only && (
              <Button type="button" variant="ghost" onClick={() => setOnly(null)}>
                Show every pair
              </Button>
            )}
            <Button type="button" onClick={exportCsv} title="Save the pairs as CSV">
              <Download size={13} /> CSV
            </Button>
          </div>
          <div className="px-3 py-2 h-80">
            <DataTable rows={pairs} columns={pairCols} rowKey={(p) => `${p.first}|${p.second}`} label="N-1-1 pairs" />
          </div>
        </div>
      )}
    </div>
  );
}
