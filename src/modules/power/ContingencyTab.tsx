// N-1 contingency analysis: take each branch or generator out on its own,
// rank the outages by what they overload, and open any of them on the
// diagram. The run itself is stannum-core's (contingency.rs).
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Download, Network, Play, ShieldAlert, Square, X } from "lucide-react";
import { useContingencies } from "../../store/contingencyStore";
import { useWorkspace } from "../../store/workspaceStore";
import { useLayers } from "../../store/layerStore";
import { api, saveTextExport } from "../../lib/analysis/api";
import type { ContingencyDetail, ContingencyRow, SolveReport, SolveSettings } from "../../lib/analysis/types";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import { elementLabel } from "../../lib/elements";
import { Button, EmptyState, ErrorBanner, Metric, Notice, SearchBox, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { allRows, objectKey, type RowSource } from "../../ui/usePagedRows";
import type { PageRule } from "../../lib/analysis/types";
import { LoadingBar } from "../../ui/CellBars";
import { ElementLink } from "../../ui/DrawAround";
import { StudyActions } from "../../ui/StudyActions";

/** Whether an outage causes anything worth looking at. */
export function isProblem(r: ContingencyRow): boolean {
  return r.converged === false || r.overloads > 0 || r.voltage_violations > 0 || r.lost_load_mw > 0.05;
}

/** isProblem as rules the core applies: any one of them. */
const PROBLEM_RULES: PageRule[] = [
  { field: "converged", operator: "eq", value: "false", upper: null },
  { field: "overloads", operator: "gt", value: "0", upper: null },
  { field: "voltage_violations", operator: "gt", value: "0", upper: null },
  { field: "lost_load_mw", operator: "gt", value: "0.05", upper: null },
];

function ms(v: number): string {
  return v >= 1000 ? `${fixed(v / 1000, 1)} s` : `${fixed(v, 0)} ms`;
}

/** The outage's worst finding in words, for the table and the CSV. */
export function outcome(r: ContingencyRow): string {
  if (r.converged === false) return "Not converged";
  if (r.overloads > 0 && r.worst) return `${fixed(r.worst.pct, 1)}% on ${elementLabel(r.worst.element)}`;
  if (r.voltage_violations > 0 && r.worst_voltage) return `Bus ${r.worst_voltage.bus} at ${fixed(r.worst_voltage.vm, 3)} p.u.`;
  if (r.lost_load_mw > 0.05) return `Drops ${fixed(r.lost_load_mw, 1)} MW of load`;
  return "No new problems";
}

function columns(limit: number): Column<ContingencyRow>[] {
  return [
    { key: "rank", header: "#", value: (r) => r.rank, align: "right" },
    {
      key: "element",
      header: "Outage",
      value: (r) => elementLabel(r.element),
      render: (r) => <span className="font-medium text-slate-800">{elementLabel(r.element)}</span>,
    },
    {
      key: "checked",
      header: "Solved",
      title: "AC: a full AC solve from the base solution. Linear: the DC outage screen only.",
      value: (r) => (r.checked === "ac" ? (r.converged ? "AC" : "AC, not converged") : "Linear"),
      render: (r) =>
        r.checked === "ac" ? (
          r.converged ? (
            <span className="rounded bg-teal-50 px-1.5 py-px text-[10px] font-semibold text-teal-800">AC</span>
          ) : (
            <span className="rounded bg-red-100 px-1.5 py-px text-[10px] font-semibold text-red-800" title={r.note ?? undefined}>
              Not converged
            </span>
          )
        ) : (
          <span className="rounded bg-slate-100 px-1.5 py-px text-[10px] font-semibold text-slate-600">Linear</span>
        ),
    },
    {
      key: "worst",
      sortField: "worst.element",
      header: "Worst branch",
      title: "The most loaded branch that is over the limit and new or worse; otherwise the most loaded branch",
      value: (r) => (r.worst ? elementLabel(r.worst.element) : null),
      render: (r) => (r.worst ? elementLabel(r.worst.element) : <span className="text-slate-400">Unrated</span>),
    },
    {
      key: "pct",
      sortField: "worst.pct",
      header: "Loading",
      title: "MVA as a percent of rate A after the outage",
      value: (r) => r.worst?.pct ?? null,
      render: (r) =>
        r.worst ? (
          <LoadingBar pct={r.worst.pct}>
            <span className={r.worst.pct > limit ? "text-red-800 font-semibold" : r.worst.pct > 0.9 * limit ? "text-amber-800" : ""}>
              {fixed(r.worst.pct, 1)}
            </span>
          </LoadingBar>
        ) : (
          "—"
        ),
      align: "right",
    },
    {
      key: "was",
      sortField: "worst.base_pct",
      header: "Before",
      title: "The same branch's loading before the outage",
      value: (r) => r.worst?.base_pct ?? null,
      render: (r) => <span className="text-slate-500">{fixed(r.worst?.base_pct, 1)}</span>,
      align: "right",
    },
    {
      key: "overloads",
      header: "Overloads",
      title: "Branches over the limit that are new or more than a point worse",
      value: (r) => r.overloads,
      render: (r) => (r.overloads ? <span className="font-semibold text-red-700">{r.overloads}</span> : ""),
      align: "right",
    },
    {
      key: "voltage",
      sortField: "worst_voltage.outside",
      header: "Voltage",
      title: "AC only: the worst bus outside its limits that is new or worse",
      value: (r) => r.worst_voltage?.outside ?? null,
      render: (r) =>
        r.worst_voltage ? (
          <span className="text-amber-800">
            Bus {r.worst_voltage.bus}: {fixed(r.worst_voltage.vm, 3)}
            {r.voltage_violations > 1 && <span className="text-slate-500"> +{r.voltage_violations - 1}</span>}
          </span>
        ) : (
          ""
        ),
      align: "right",
    },
    {
      key: "cut",
      sortField: "cut_off_buses",
      header: "Splits off",
      title: "Buses left without a path to the rest of the network, and the load on them",
      value: (r) => r.cut_off_buses,
      render: (r) =>
        r.cut_off_buses ? (
          <span>
            {r.cut_off_buses} bus{r.cut_off_buses === 1 ? "" : "es"}
            {r.lost_load_mw > 0.05 && <span className="text-red-700">, {fixed(r.lost_load_mw, 0)} MW load</span>}
            {r.lost_generation_mw > 0.05 && <span className="text-slate-500">, {fixed(r.lost_generation_mw, 0)} MW gen</span>}
          </span>
        ) : (
          ""
        ),
      align: "right",
    },
  ];
}

function Section({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <section className="px-3 py-2 border-t border-slate-100">
      <h3 className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase mb-1">{title}</h3>
      {children}
    </section>
  );
}

/** One outage in full, beside the table. */
function OutagePanel({
  detail,
  row,
  limit,
  onPick,
  onClose,
}: {
  detail: ContingencyDetail | null;
  row: ContingencyRow;
  limit: number;
  onPick: (element: string) => void;
  onClose: () => void;
}) {
  const drawAround = useWorkspace((s) => s.drawAround);
  const setOnDiagram = useContingencies((s) => s.setOnDiagram);
  const onDiagram = useContingencies((s) => s.onDiagram);
  const toggleLayer = useLayers((s) => s.toggle);
  const moved = useMemo(
    () =>
      detail
        ? [...detail.branches]
            .filter((b) => !b.tripped)
            .sort((a, b) => Math.abs(b.mw - b.base_mw) - Math.abs(a.mw - a.base_mw))
            .slice(0, 8)
        : [],
    [detail]
  );
  const show = () => {
    setOnDiagram(true);
    toggleLayer("contingency", true);
    // Around the outage, the cause; each overload below has its own link.
    drawAround(row.element, "power");
  };
  return (
    <aside className="w-80 shrink-0 border-l border-slate-200 bg-white flex flex-col min-h-0" data-testid="contingency-detail">
      <header className="flex items-start gap-2 px-3 py-2 border-b border-slate-200">
        <div className="min-w-0 flex-1">
          <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">Outage #{row.rank}</p>
          <h2 className="text-sm font-semibold text-slate-800">{elementLabel(row.element)}</h2>
          <p className="text-[11px] text-slate-500">
            {row.checked === "ac"
              ? row.converged
                ? `AC solve, converged in ${row.iterations} iteration${row.iterations === 1 ? "" : "s"}`
                : "AC solve did not converge"
              : "Linear screen only"}
          </p>
        </div>
        <button type="button" className="p-1 rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700" onClick={onClose} aria-label="Close outage">
          <X size={14} />
        </button>
      </header>
      <div className="flex flex-wrap items-center gap-1 px-3 py-2">
        <Button variant={onDiagram ? "secondary" : "primary"} onClick={show} data-testid="show-on-diagram">
          <Network size={13} /> Show on diagram
        </Button>
        <StudyActions element={row.element} />
      </div>
      <div className="flex-1 min-h-0 overflow-auto text-xs">
        {row.note && (
          <div className="px-3 pb-2">
            <Notice>{row.note}</Notice>
          </div>
        )}
        {!detail ? (
          <p className="px-3 py-2 text-slate-500">Solving the outage…</p>
        ) : (
          <>
            <Section title={`Overloads above ${fixed(limit, 0)}% (${detail.overloads.length})`}>
              {detail.overloads.length === 0 ? (
                <p className="text-slate-500">No branch goes over the limit, or gets worse than it was.</p>
              ) : (
                <ul className="flex flex-col gap-1">
                  {detail.overloads.slice(0, 30).map((o) => (
                    <li key={o.element + o.from + o.to} className="flex items-center gap-2">
                      <span className="min-w-0 flex-1 truncate">
                        <ElementLink element={o.element} from="power" onOpen={() => onPick(o.element)} />
                      </span>
                      <span className="tabular-nums font-semibold text-red-700">{fixed(o.pct, 1)}%</span>
                      <span className="tabular-nums text-slate-400 w-14 text-right">was {fixed(o.base_pct, 0)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Section>
            {row.checked === "ac" && (
              <Section title={`Voltage problems (${detail.voltage_problems.length})`}>
                {detail.voltage_problems.length === 0 ? (
                  <p className="text-slate-500">Every bus stays within its limits, or no worse than before.</p>
                ) : (
                  <ul className="flex flex-col gap-1">
                    {detail.voltage_problems.slice(0, 30).map((v) => (
                      <li key={v.bus} className="flex items-center gap-2">
                        <span className="min-w-0 flex-1 truncate">
                          <ElementLink element={`bus:${v.bus}`} from="power" onOpen={() => onPick(`bus:${v.bus}`)}>
                            Bus {v.bus}
                            {v.name ? ` ${v.name}` : ""}
                          </ElementLink>
                        </span>
                        <span className="tabular-nums font-semibold text-amber-800">{fixed(v.vm, 3)}</span>
                        <span className="tabular-nums text-slate-400 w-14 text-right">was {fixed(v.base_vm, 3)}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </Section>
            )}
            {detail.cut_off.length > 0 && (
              <Section title={`Split off (${detail.cut_off.length} bus${detail.cut_off.length === 1 ? "" : "es"})`}>
                <p className="text-slate-600">
                  {fixed(row.lost_load_mw, 1)} MW of load and {fixed(row.lost_generation_mw, 1)} MW of generation lose their path
                  to the rest of the network: buses {detail.cut_off.slice(0, 12).join(", ")}
                  {detail.cut_off.length > 12 ? ` and ${detail.cut_off.length - 12} more` : ""}.
                </p>
              </Section>
            )}
            <Section title="Largest flow changes">
              {moved.length === 0 ? (
                <p className="text-slate-500">No branch flow moves by more than half a MW.</p>
              ) : (
                <ul className="flex flex-col gap-1">
                  {moved.map((b) => (
                    <li key={b.element + b.from + b.to} className="flex items-center gap-2">
                      <span className="min-w-0 flex-1 truncate">
                        <ElementLink element={b.element} from="power" onOpen={() => onPick(b.element)} />
                      </span>
                      <span className={`tabular-nums ${b.mw - b.base_mw > 0 ? "text-red-700" : "text-sky-700"}`}>
                        {b.mw - b.base_mw > 0 ? "+" : ""}
                        {fixed(b.mw - b.base_mw, 1)} MW
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </Section>
          </>
        )}
      </div>
    </aside>
  );
}

export function ContingencyTab({
  report,
  settings,
  scope,
  onPick,
}: {
  report: SolveReport | null;
  settings: SolveSettings;
  /** Bus filters narrow the outages to the elements touching matching buses. */
  scope: { elements: string[]; buses: number } | null;
  onPick: (element: string) => void;
}) {
  const s = useContingencies();
  const [search, setSearch] = useState("");
  const [problemsOnly, setProblemsOnly] = useState(false);
  const cols = useMemo(() => columns(s.summary?.limit_pct ?? s.choice.limitPct), [s.summary, s.choice.limitPct]);
  // The ranked rows are read from the core a page at a time.
  const [clicked, setClicked] = useState<ContingencyRow | null>(null);
  const runId = useMemo(() => ({}), [s.summary]);
  const source = useMemo<RowSource<ContingencyRow>>(
    () => ({
      key: `${objectKey(runId)}\u0000${problemsOnly}`,
      fetch: (r) => api.contingencyPage({ ...r, any: problemsOnly ? PROBLEM_RULES : [] }),
    }),
    [runId, problemsOnly]
  );
  const [problemCount, setProblemCount] = useState<number | null>(null);
  useEffect(() => {
    setProblemCount(null);
    if (!s.summary) return;
    let live = true;
    api
      .contingencyPage({ limit: 0, any: PROBLEM_RULES })
      .then((p) => live && setProblemCount(p.matched))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [s.summary]);
  const openRow =
    s.open === null
      ? undefined
      : clicked?.rank === s.open
        ? clicked
        : (s.rows.find((r) => r.rank === s.open) ?? (s.detail?.row.rank === s.open ? s.detail.row : undefined));
  // Open the worst outage once a run finishes.
  useEffect(() => {
    if (s.summary && s.open === null && s.rows.length) void s.openRow(s.rows[0].rank);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.summary]);

  const acReady = !!report && report.model === "ac" && report.converged;
  const run = () => void s.run(settings, scope ? scope.elements : null);
  async function exportCsv() {
    const rows = await allRows(source, null, search.trim());
    const csv = toCsv(
      rows.map((r) => ({
        rank: r.rank,
        outage: r.element,
        solved: r.checked === "ac" ? (r.converged ? "ac" : "ac_not_converged") : "linear",
        worst_branch: r.worst?.element ?? "",
        loading_pct: r.worst?.pct ?? "",
        before_pct: r.worst?.base_pct ?? "",
        overloads: r.overloads,
        worst_bus: r.worst_voltage?.bus ?? "",
        worst_vm: r.worst_voltage?.vm ?? "",
        voltage_violations: r.voltage_violations,
        cut_off_buses: r.cut_off_buses,
        lost_load_mw: r.lost_load_mw,
        outcome: outcome(r),
      }))
    );
    await saveTextExport("contingencies.csv", csv, { name: "CSV", extensions: ["csv"] });
  }

  const summary = s.summary;
  const progressPct = s.progress && s.progress.total ? (100 * s.progress.done) / s.progress.total : 0;
  return (
    <div className="h-full flex flex-col" data-testid="contingency-tab">
      <form
        className="flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2 border-b border-slate-200 bg-white text-xs"
        onSubmit={(e) => {
          e.preventDefault();
          run();
        }}
      >
        <fieldset className="flex flex-col gap-1">
          <legend className="text-[11px] font-medium text-slate-600 mb-1">Take out, one at a time</legend>
          <div className="flex items-center gap-3">
            <label className="flex items-center gap-1.5">
              <input type="checkbox" checked={s.choice.branches} onChange={(e) => s.setChoice({ branches: e.target.checked })} />
              Branches
            </label>
            <label className="flex items-center gap-1.5">
              <input type="checkbox" checked={s.choice.generators} onChange={(e) => s.setChoice({ generators: e.target.checked })} />
              Generators
            </label>
          </div>
        </fieldset>
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Method
          <select
            className={inputClass}
            value={s.choice.method}
            onChange={(e) => s.setChoice({ method: e.target.value as "screen" | "ac" })}
            aria-label="Method"
          >
            <option value="screen">Linear screen, then AC for the worst</option>
            <option value="ac">AC solve of every outage</option>
          </select>
        </label>
        {s.choice.method === "screen" && (
          <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
            Worst checked in AC
            <input
              className={`${inputClass} w-20`}
              type="number"
              min={0}
              step={1}
              value={s.choice.acCount}
              onChange={(e) => s.setChoice({ acCount: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
              aria-label="Worst checked in AC"
            />
          </label>
        )}
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Overloaded above (% of rate A)
          <input
            className={`${inputClass} w-20`}
            type="number"
            min={1}
            step="any"
            value={s.choice.limitPct}
            onChange={(e) => s.setChoice({ limitPct: Math.max(1, Number(e.target.value) || 100) })}
            aria-label="Overload limit"
          />
        </label>
        <div className="flex items-center gap-2 ml-auto">
          {s.running ? (
            <>
              <span className="flex flex-col gap-1 w-44" data-testid="contingency-progress">
                <span className="text-[11px] text-slate-600">
                  {s.progress?.phase === "ac" ? "AC solves" : "Screening"} {s.progress ? `${count(s.progress.done)} of ${count(s.progress.total)}` : "…"}
                </span>
                <span className="h-1.5 rounded bg-slate-100 overflow-hidden">
                  <span className="block h-full bg-teal-600 transition-[width]" style={{ width: `${progressPct}%` }} />
                </span>
              </span>
              <Button type="button" onClick={s.cancel}>
                <Square size={12} /> Cancel
              </Button>
            </>
          ) : (
            <Button
              type="submit"
              variant="primary"
              disabled={!acReady || (!s.choice.branches && !s.choice.generators && !scope)}
              title={acReady ? undefined : "Solve the case with the AC model first"}
              data-testid="run-contingencies"
            >
              <Play size={13} /> Run contingencies
            </Button>
          )}
        </div>
        {scope && (
          <p className="basis-full text-[11px] text-slate-500">
            The bus filters above narrow the outages to {count(scope.elements.length)} branches and generators touching{" "}
            {count(scope.buses)} matching buses.
          </p>
        )}
      </form>
      {s.error && (
        <div className="p-3 pb-0">
          <ErrorBanner>{s.error}</ErrorBanner>
        </div>
      )}
      {!summary ? (
        <EmptyState icon={<ShieldAlert size={22} />} title="What if it trips?">
          {acReady ? (
            <>
              Takes each {s.choice.branches && s.choice.generators ? "branch and generator" : s.choice.branches ? "branch" : "generator"} out on its own,
              screens every outage with linear outage factors, then solves the worst {s.choice.acCount} in AC from this solution. Outages
              are ranked by the overloads and voltage problems they cause.
            </>
          ) : (
            <>Solve the case with the AC model first: contingencies start from that solution.</>
          )}
        </EmptyState>
      ) : (
        <>
          <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-2 p-3" data-testid="contingency-summary">
            <Metric
              label="Outages"
              value={count(summary.total)}
              detail={`${count(summary.ac_checked)} solved in AC`}
            />
            <Metric
              label="Cause overloads"
              value={count(summary.with_overloads)}
              tone={summary.with_overloads ? "bad" : "good"}
              detail={summary.worst?.worst && summary.worst.overloads ? `Worst ${fixed(summary.worst.worst.pct, 0)}%` : `Above ${fixed(summary.limit_pct, 0)}% of rate A`}
            />
            <Metric
              label="Voltage problems"
              value={count(summary.with_voltage_problems)}
              tone={summary.with_voltage_problems ? "warn" : "good"}
              detail="From the AC solves"
            />
            <Metric label="Not converged" value={count(summary.not_converged)} tone={summary.not_converged ? "bad" : "good"} detail="AC solves" />
            <Metric label="Split the network" value={count(summary.islanding)} detail="Leave buses without a path" />
            <Metric
              label="Time"
              value={ms(summary.elapsed_ms)}
              detail={`Screen ${ms(summary.screen_ms)}, AC ${ms(summary.ac_ms)}`}
            />
          </div>
          {summary.cancelled && (
            <div className="px-3 pb-2">
              <Notice>Cancelled before the end. Outages not reached in AC keep their linear screen result.</Notice>
            </div>
          )}
          <div className="flex-1 min-h-0 flex border-t border-slate-200">
            <div className="flex-1 min-w-0 flex flex-col">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 px-3 py-2 border-b border-slate-200 bg-white">
                <SearchBox value={search} onChange={setSearch} placeholder="Search outages…" />
                <label className="flex items-center gap-1.5 text-xs whitespace-nowrap text-slate-700">
                  <input type="checkbox" checked={problemsOnly} onChange={(e) => setProblemsOnly(e.target.checked)} />
                  Problems only{problemCount === null ? "" : ` (${count(problemCount)})`}
                </label>
                <Button variant="ghost" className="ml-auto whitespace-nowrap" onClick={exportCsv}>
                  <Download size={13} /> Export CSV
                </Button>
              </div>
              <div className="flex-1 min-h-0 bg-white">
                <DataTable
                  source={source}
                  columns={cols}
                  rowKey={(r) => r.rank}
                  search={search}
                  selectedKey={s.open}
                  onRowClick={(r) => {
                    setClicked(r);
                    void s.openRow(r.rank);
                  }}
                  rowClassName={(r) => (r.converged === false || r.overloads > 0 ? "bg-red-50/60" : r.voltage_violations > 0 ? "bg-amber-50/60" : "")}
                />
              </div>
            </div>
            {openRow && (
              <OutagePanel
                row={openRow}
                detail={s.detail?.row.rank === openRow.rank ? s.detail : null}
                limit={summary.limit_pct}
                onPick={onPick}
                onClose={() => void s.openRow(null)}
              />
            )}
          </div>
        </>
      )}
    </div>
  );
}
