// N-1 contingency analysis: take each branch or generator out on its own,
// rank the outages by what they overload, and open any of them on the
// diagram. Or run the NERC TPL-001 planning events the core generates from
// the case (planning.rs). The run itself is stannum-core's (contingency.rs).
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { FileText, Network, Play, ShieldAlert, Square, X } from "lucide-react";
import { PLANNING_CATEGORIES, useContingencies, type ContingencySource } from "../../store/contingencyStore";
import { useWorkspace } from "../../store/workspaceStore";
import { useLayers } from "../../store/layerStore";
import { LISTS_FILTERS, api, pickFile, pickSavePath, saveTextExport } from "../../lib/analysis/api";
import { OverflowMenu } from "../../ui/OverflowMenu";
import type { CategoryCount, ContingencyDetail, ContingencyRow, SolveReport, SolveSettings } from "../../lib/analysis/types";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import { elementLabel } from "../../lib/elements";
import { RATINGS, ratingLabel, type Rating } from "../../lib/analysis/ratings";
import { Button, EmptyState, ErrorBanner, Metric, Notice, SearchBox, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { allRows, objectKey, type RowSource } from "../../ui/usePagedRows";
import type { PageRule } from "../../lib/analysis/types";
import { LoadingBar } from "../../ui/CellBars";
import { ElementLink } from "../../ui/DrawAround";
import { StudyActions } from "../../ui/StudyActions";

/** Whether an outage causes anything worth looking at. */
export function isProblem(r: ContingencyRow): boolean {
  return r.converged === false || r.overloads > 0 || r.interface_violations > 0 || r.voltage_violations > 0 || r.lost_load_mw > 0.05;
}

/** isProblem as rules the core applies: any one of them. */
const PROBLEM_RULES: PageRule[] = [
  { field: "converged", operator: "eq", value: "false", upper: null },
  { field: "overloads", operator: "gt", value: "0", upper: null },
  { field: "voltage_violations", operator: "gt", value: "0", upper: null },
  { field: "interface_violations", operator: "gt", value: "0", upper: null },
  { field: "lost_load_mw", operator: "gt", value: "0.05", upper: null },
];

/** The outage in words: "Base case" for P0, otherwise the element. */
export function outageLabel(r: ContingencyRow): string {
  return r.kind === "base" ? "Base case" : elementLabel(r.element);
}

function CategoryBadge({ category }: { category: string }) {
  return (
    <span className="rounded bg-indigo-50 px-1.5 py-px text-[10px] font-semibold text-indigo-800 tabular-nums" data-testid="category-badge">
      {category}
    </span>
  );
}

function ms(v: number): string {
  return v >= 1000 ? `${fixed(v / 1000, 1)} s` : `${fixed(v, 0)} ms`;
}

/** The outage's worst finding in words, for the table and the CSV. */
export function outcome(r: ContingencyRow): string {
  if (r.converged === false) return "Not converged";
  if (r.overloads > 0 && r.worst) return `${fixed(r.worst.pct, 1)}% on ${elementLabel(r.worst.element)}`;
  if (r.interface_violations > 0 && r.worst_interface) return `${fixed(r.worst_interface.pct, 1)}% on interface ${r.worst_interface.name}`;
  if (r.voltage_violations > 0 && r.worst_voltage) return `Bus ${r.worst_voltage.bus} at ${fixed(r.worst_voltage.vm, 3)} p.u.`;
  if (r.lost_load_mw > 0.05) return `Drops ${fixed(r.lost_load_mw, 1)} MW of load`;
  return "No new problems";
}

function columns(limit: number, planning: boolean, rating: Rating, baseRating: Rating): Column<ContingencyRow>[] {
  const category: Column<ContingencyRow>[] = planning
    ? [
        {
          key: "category",
          header: "Event",
          title: "NERC TPL-001 planning event category",
          value: (r) => r.category,
          render: (r) => (r.category ? <CategoryBadge category={r.category} /> : ""),
        },
      ]
    : [];
  return [
    { key: "rank", header: "#", value: (r) => r.rank, align: "right" },
    ...category,
    {
      key: "element",
      header: "Outage",
      value: (r) => outageLabel(r),
      render: (r) => (
        <span className="flex flex-col">
          <span className="font-medium text-slate-800">{outageLabel(r)}</span>
          {r.description && r.kind !== "base" && <span className="text-[10px] text-slate-500">{r.description}</span>}
        </span>
      ),
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
      render: (r) => (r.worst ? elementLabel(r.worst.element) : <span className="text-slate-500">Unrated</span>),
    },
    {
      key: "pct",
      sortField: "worst.pct",
      header: "Loading",
      title: `MVA as a percent of ${ratingLabel(rating)} after the outage`,
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
      title: `The same branch's loading before the outage, as a percent of ${ratingLabel(baseRating)}`,
      value: (r) => r.worst?.base_pct ?? null,
      render: (r) => <span className="text-slate-500">{fixed(r.worst?.base_pct, 1)}</span>,
      align: "right",
    },
    {
      key: "overloads",
      header: "Overloads",
      title: "Branches over the limit that are new or more than a point worse, and monitored interfaces over theirs",
      value: (r) => r.overloads + r.interface_violations,
      render: (r) =>
        r.overloads || r.interface_violations ? (
          <span className="font-semibold text-red-700">
            {r.overloads || ""}
            {r.interface_violations > 0 && (
              <span data-testid="interface-count">
                {r.overloads ? " + " : ""}
                {r.interface_violations} interface
                {r.interface_violations === 1 ? "" : "s"}
              </span>
            )}
          </span>
        ) : (
          ""
        ),
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
      <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase mb-1">{title}</h3>
      {children}
    </section>
  );
}

/** The last part of a path, for showing a chosen file. */
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** The most pairs the core runs, as in stannum-core's contingency::MAX_PAIRS. */
const MAX_PAIRS = 250_000;

/** How many pairs N-2 of `n` elements makes, and whether that is too many. */
function pairNote(n: number): string {
  const pairs = (n * (n - 1)) / 2;
  const base = `${count(n)} elements make ${count(pairs)} pairs.`;
  return pairs > MAX_PAIRS ? `${base} That is more than ${count(MAX_PAIRS)}: narrow them with the bus filters.` : base;
}

/** One outage in full, beside the table. */
function OutagePanel({
  detail,
  row,
  limit,
  rating,
  monitored,
  onPick,
  onClose,
}: {
  detail: ContingencyDetail | null;
  row: ContingencyRow;
  limit: number;
  rating: Rating;
  /** A lists file's monitors set each branch's limit. */
  monitored: boolean;
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
    // A pair is drawn around its first element; the layer marks both.
    drawAround(row.element.split("&")[0], "power");
  };
  return (
    <aside aria-label="Outage detail" className="w-80 shrink-0 border-l border-slate-200 bg-white flex flex-col min-h-0" data-testid="contingency-detail">
      <header className="flex items-start gap-2 px-3 py-2 border-b border-slate-200">
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-1.5 text-[10px] font-semibold tracking-wider text-slate-500 uppercase">
            Outage #{row.rank}
            {row.category && <CategoryBadge category={row.category} />}
          </p>
          <h2 className="text-sm font-semibold text-slate-800">{outageLabel(row)}</h2>
          {row.description && row.kind !== "base" && <p className="text-[11px] text-slate-600">{row.description}</p>}
          <p className="text-[11px] text-slate-500">
            {row.checked === "ac"
              ? row.converged
                ? `AC solve, converged in ${row.iterations} iteration${row.iterations === 1 ? "" : "s"}`
                : "AC solve did not converge"
              : "Linear screen only"}
          </p>
        </div>
        <button type="button" className="p-1 rounded text-slate-500 hover:bg-slate-100 hover:text-slate-700" onClick={onClose} aria-label="Close outage">
          <X size={14} />
        </button>
      </header>
      {row.kind === "base" ? (
        <p className="px-3 py-2 text-[11px] text-slate-600">
          The case as it stands, checked against the limits: every overload and voltage problem counts, not only new ones.
        </p>
      ) : (
        <div className="flex flex-wrap items-center gap-1 px-3 py-2">
          <Button variant={onDiagram ? "secondary" : "primary"} onClick={show} data-testid="show-on-diagram">
            <Network size={13} /> Show on diagram
          </Button>
          {(row.kind === "branch" || row.kind === "generator") && !row.element.includes("&") && <StudyActions element={row.element} />}
        </div>
      )}
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
            <Section
              title={
                monitored
                  ? `Overloads past their monitor's limit (${detail.overloads.length})`
                  : `Overloads above ${fixed(limit, 0)}% of ${ratingLabel(rating)} (${detail.overloads.length})`
              }
            >
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
                      <span className="tabular-nums text-slate-500 w-14 text-right">was {fixed(o.base_pct, 0)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Section>
            {detail.interfaces.length > 0 && (
              <Section title={`Interfaces (${detail.interfaces.length})`}>
                <ul className="flex flex-col gap-1" data-testid="detail-interfaces">
                  {detail.interfaces.map((i) => {
                    const over = row.worst_interface?.name === i.name || (i.pct !== null && i.pct > i.limit_pct && (i.base_pct ?? 0) <= i.limit_pct);
                    return (
                      <li key={i.name} className="flex items-center gap-2">
                        <span className="min-w-0 flex-1 truncate">{i.name}</span>
                        <span className={`tabular-nums ${over ? "font-semibold text-red-700" : "text-slate-700"}`}>
                          {fixed(i.mw, 1)} MW
                          {i.pct !== null ? ` · ${fixed(i.pct, 1)}%` : ""}
                        </span>
                        <span className="tabular-nums text-slate-500 w-20 text-right">was {fixed(i.base_mw, 1)}</span>
                      </li>
                    );
                  })}
                </ul>
              </Section>
            )}
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
                        <span className="tabular-nums text-slate-500 w-14 text-right">was {fixed(v.base_vm, 3)}</span>
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

/** The planning event categories, each with how many events it gives on the case. */
function PlanningCategories({
  counts,
  chosen,
  onChange,
}: {
  counts: CategoryCount[] | null;
  chosen: string[];
  onChange: (categories: string[]) => void;
}) {
  const label = (c: string) => counts?.find((k) => k.category === c);
  const toggle = (c: string, on: boolean) =>
    onChange(PLANNING_CATEGORIES.filter((k) => (k === c ? on : chosen.includes(k))));
  return (
    <fieldset className="flex flex-col gap-1" data-testid="planning-categories">
      <legend className="text-[11px] font-medium text-slate-600 mb-1">Planning events</legend>
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-x-3 gap-y-0.5">
        {PLANNING_CATEGORIES.map((c) => {
          const info = label(c);
          return (
            <label key={c} className="flex items-center gap-1.5 whitespace-nowrap" title={info?.label}>
              <input type="checkbox" checked={chosen.includes(c)} onChange={(e) => toggle(c, e.target.checked)} />
              <span className="font-semibold tabular-nums">{c}</span>
              <span className="text-slate-600">{info?.label ?? ""}</span>
              {info && <span className="tabular-nums text-slate-400">{count(info.count)}</span>}
            </label>
          );
        })}
      </div>
    </fieldset>
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
  const [category, setCategory] = useState("");
  const planned = !!s.summary && s.summary.categories.length > 0;
  const shownRating = s.summary?.rating ?? s.choice.rating;
  const shownBase = s.summary?.base_rating ?? s.choice.baseRating;
  const cols = useMemo(
    () => columns(s.summary?.limit_pct ?? s.choice.limitPct, planned, shownRating, shownBase),
    [s.summary, s.choice.limitPct, planned, shownRating, shownBase]
  );
  const planning = s.choice.source === "planning";
  const fromLists = s.choice.source === "lists";
  // Planning event counts, for the categories to pick from.
  const loadCounts = s.loadCounts;
  useEffect(() => {
    if (planning) void loadCounts();
  }, [planning, report, s.choice.kvMin, loadCounts]);
  useEffect(() => setCategory(""), [s.summary]);
  // The ranked rows are read from the core a page at a time.
  const [clicked, setClicked] = useState<ContingencyRow | null>(null);
  const runId = useMemo(() => ({}), [s.summary]);
  const source = useMemo<RowSource<ContingencyRow>>(
    () => ({
      key: `${objectKey(runId)}\u0000${problemsOnly}\u0000${category}`,
      fetch: (r) =>
        api.contingencyPage({
          ...r,
          rules: category
            ? [
                {
                  field: "category",
                  operator: "eq" as const,
                  value: category,
                  upper: null,
                },
              ]
            : [],
          any: problemsOnly ? PROBLEM_RULES : [],
        }),
    }),
    [runId, problemsOnly, category]
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
  const chosenCount = (s.counts ?? []).filter((c) => s.choice.categories.includes(c.category)).reduce((n, c) => n + c.count, 0);
  const noMonitors = s.choice.monitorFromLists && !s.choice.monitorsPath;
  const nothingChosen = noMonitors || fromLists ? noMonitors || !s.choice.listsPath : planning ? s.choice.categories.length === 0 : !s.choice.branches && !s.choice.generators && !s.choice.buses && !scope;
  // Branches and generators that N-2 pairs up (buses stay one at a time).
  const pairElements = scope
    ? scope.elements.filter((e) => !e.startsWith("bus:")).length
    : (s.choice.branches ? new Set(report?.branches.filter((b) => b.online && !b.element.startsWith("switch:")).map((b) => b.element)).size : 0) +
      (s.choice.generators ? (report?.generators.filter((g) => g.online).length ?? 0) : 0);
  async function exportViolations() {
    const path = await pickSavePath("contingency-violations.csv", [{ name: "CSV", extensions: ["csv"] }]);
    if (!path) return;
    await api.exportContingencyViolations(path);
  }
  async function exportCsv() {
    const rows = await allRows(source, null, search.trim());
    const csv = toCsv(
      rows.map((r) => ({
        rank: r.rank,
        category: r.category ?? "",
        outage: r.kind === "base" ? "base case" : r.element,
        description: r.description ?? "",
        solved: r.checked === "ac" ? (r.converged ? "ac" : "ac_not_converged") : "linear",
        worst_branch: r.worst?.element ?? "",
        loading_pct: r.worst?.pct ?? "",
        before_pct: r.worst?.base_pct ?? "",
        overloads: r.overloads,
        worst_interface: r.worst_interface?.name ?? "",
        interface_pct: r.worst_interface?.pct ?? "",
        interface_violations: r.interface_violations,
        worst_bus: r.worst_voltage?.bus ?? "",
        worst_vm: r.worst_voltage?.vm ?? "",
        voltage_violations: r.voltage_violations,
        cut_off_buses: r.cut_off_buses,
        lost_load_mw: r.lost_load_mw,
        outcome: outcome(r),
      }))
    );
    await saveTextExport("contingencies.csv", csv, {
      name: "CSV",
      extensions: ["csv"],
    });
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
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Outages
          <select
            className={inputClass}
            value={s.choice.source}
            onChange={(e) => s.setChoice({ source: e.target.value as ContingencySource })}
            aria-label="Outages"
            data-testid="contingency-source"
          >
            <option value="n1">N-1: each element on its own</option>
            <option value="planning">NERC TPL-001 planning events</option>
            <option value="lists">From a lists file (.stnl)</option>
          </select>
        </label>
        {fromLists ? (
          <div className="flex flex-col gap-1 min-w-0">
            <span className="text-[11px] font-medium text-slate-600">Lists file</span>
            <div className="flex items-center gap-2 min-w-0">
              <Button
                type="button"
                onClick={async () => {
                  const path = await pickFile(LISTS_FILTERS);
                  if (path) s.setChoice({ listsPath: path });
                }}
                data-testid="choose-lists"
              >
                <FileText size={13} /> {s.choice.listsPath ? "Change…" : "Choose…"}
              </Button>
              <span className="truncate max-w-64 text-[11px] text-slate-600" title={s.choice.listsPath ?? undefined} data-testid="lists-path">
                {s.choice.listsPath ? fileName(s.choice.listsPath) : "No file chosen"}
              </span>
            </div>
          </div>
        ) : planning ? (
          <>
            <PlanningCategories
              counts={s.counts}
              chosen={s.choice.categories}
              onChange={(categories) => s.setChoice({ categories })}
            />
            <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
              At or above (kV)
              <input
                className={`${inputClass} w-20`}
                type="number"
                min={0}
                step="any"
                value={s.choice.kvMin}
                onChange={(e) =>
                  s.setChoice({
                    kvMin: Math.max(0, Number(e.target.value) || 0),
                  })
                }
                aria-label="Lowest voltage"
                title="Branches, shunts and DC lines with a terminal at or above this voltage. Generators are not filtered by voltage."
              />
            </label>
          </>
        ) : (
          <fieldset className="flex flex-col gap-1">
            <legend className="text-[11px] font-medium text-slate-600 mb-1">Take out</legend>
            <div className="flex items-center gap-3">
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={s.choice.branches} onChange={(e) => s.setChoice({ branches: e.target.checked })} />
                Branches
              </label>
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={s.choice.generators} onChange={(e) => s.setChoice({ generators: e.target.checked })} />
                Generators
              </label>
              <label className="flex items-center gap-1.5" title="Each bus with every branch and DC line connected to it, cutting off its loads, generators and shunts">
                <input type="checkbox" checked={s.choice.buses} onChange={(e) => s.setChoice({ buses: e.target.checked })} data-testid="outage-buses" />
                Buses
              </label>
              <select
                className={`${inputClass} py-0.5`}
                value={s.choice.pairs ? "pairs" : "single"}
                onChange={(e) => s.setChoice({ pairs: e.target.value === "pairs" })}
                aria-label="How many at a time"
                data-testid="outage-depth"
              >
                <option value="single">One at a time (N-1)</option>
                <option value="pairs">Two at a time (N-2)</option>
              </select>
            </div>
            {s.choice.pairs && (
              <span className="text-[11px] text-slate-500" data-testid="pair-count">
                {pairNote(pairElements)}
              </span>
            )}
          </fieldset>
        )}
        <div className="flex flex-col gap-1 min-w-0">
          <label className="text-[11px] font-medium text-slate-600" htmlFor="contingency-checked">
            Checked
          </label>
          <div className="flex items-center gap-2 min-w-0">
            <select
              id="contingency-checked"
              className={inputClass}
              value={s.choice.monitorFromLists ? "lists" : "all"}
              onChange={(e) => {
                const lists = e.target.value === "lists";
                s.setChoice({
                  monitorFromLists: lists,
                  monitorsPath: s.choice.monitorsPath ?? (lists ? s.choice.listsPath : null),
                });
              }}
              title="What is checked after each outage: every rated branch and every bus, or only what a lists file's monitor records name (with their own limits, and interfaces)"
              data-testid="contingency-checked"
            >
              <option value="all">Every branch and bus</option>
              <option value="lists">Monitors from a lists file</option>
            </select>
            {s.choice.monitorFromLists && (
              <>
                <Button
                  type="button"
                  onClick={async () => {
                    const path = await pickFile(LISTS_FILTERS);
                    if (path) s.setChoice({ monitorsPath: path });
                  }}
                  data-testid="choose-monitors"
                >
                  <FileText size={13} /> {s.choice.monitorsPath ? "Change…" : "Choose…"}
                </Button>
                <span className="truncate max-w-48 text-[11px] text-slate-600" title={s.choice.monitorsPath ?? undefined} data-testid="monitors-path">
                  {s.choice.monitorsPath ? fileName(s.choice.monitorsPath) : "No file chosen"}
                </span>
              </>
            )}
          </div>
        </div>
        <fieldset className="flex flex-col gap-1">
          <legend className="text-[11px] font-medium text-slate-600 mb-1">Zero-impedance lines</legend>
          <div className="flex items-center gap-2">
            <label
              className="flex items-center gap-1.5"
              title="Leave out lines with no resistance and a reactance at or below the threshold, as PSS/E treats them (THRSHZ). A bus outage still opens them."
            >
              <input
                type="checkbox"
                checked={s.choice.skipZeroImpedance}
                onChange={(e) => s.setChoice({ skipZeroImpedance: e.target.checked })}
                data-testid="skip-zero-impedance"
              />
              Skip, |X| ≤
            </label>
            <input
              className={`${inputClass} w-20`}
              type="number"
              min={0}
              step="any"
              value={s.choice.zeroImpedancePu}
              disabled={!s.choice.skipZeroImpedance}
              onChange={(e) =>
                s.setChoice({
                  zeroImpedancePu: Math.max(0, Number(e.target.value) || 0),
                })
              }
              aria-label="Zero-impedance threshold, p.u."
              title="THRSHZ, per unit on the system base"
            />
            <span className="text-[11px] text-slate-500">p.u.</span>
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
              onChange={(e) =>
                s.setChoice({
                  acCount: Math.max(0, Math.round(Number(e.target.value) || 0)),
                })
              }
              aria-label="Worst checked in AC"
            />
          </label>
        )}
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Base case rating
          <select
            className={inputClass}
            value={s.choice.baseRating}
            onChange={(e) => s.setChoice({ baseRating: e.target.value as Rating })}
            aria-label="Base case rating"
            title="The rating for loadings before an outage, and for the base case (P0)"
            data-testid="base-rating"
          >
            {RATINGS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label} ({r.hint})
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Contingency rating
          <select
            className={inputClass}
            value={s.choice.rating}
            onChange={(e) => s.setChoice({ rating: e.target.value as Rating })}
            aria-label="Contingency rating"
            title="The rating for loadings after an outage"
            data-testid="contingency-rating"
          >
            {RATINGS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label} ({r.hint})
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Overloaded above (%)
          <input
            className={`${inputClass} w-20`}
            type="number"
            min={1}
            step="any"
            value={s.choice.limitPct}
            onChange={(e) =>
              s.setChoice({
                limitPct: Math.max(1, Number(e.target.value) || 100),
              })
            }
            aria-label="Overload limit"
          />
        </label>
        <label
          className="flex flex-col gap-1 text-[11px] font-medium text-slate-600"
          title="An overload already there before the outage is reported only when it grows by more than this many points"
        >
          Worse by (pts)
          <input
            className={`${inputClass} w-16`}
            type="number"
            min={0}
            step="any"
            value={s.choice.worseByPct}
            disabled={s.choice.reportExisting}
            onChange={(e) => s.setChoice({ worseByPct: Math.max(0, Number(e.target.value) || 0) })}
            aria-label="Worse by points"
            data-testid="contingency-worse-by"
          />
        </label>
        <label
          className="flex items-center gap-1.5 self-end pb-1.5 text-xs whitespace-nowrap text-slate-700"
          title="Also report overloads and voltage problems that were there before the outage and got no worse"
        >
          <input
            type="checkbox"
            checked={s.choice.reportExisting}
            onChange={(e) => s.setChoice({ reportExisting: e.target.checked })}
            data-testid="contingency-report-existing"
          />
          Include existing
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
              disabled={!acReady || nothingChosen}
              title={acReady ? undefined : "Solve the case with the AC model first"}
              data-testid="run-contingencies"
            >
              <Play size={13} /> Run contingencies
            </Button>
          )}
        </div>
        {planning && s.counts && (
          <p className="basis-full text-[11px] text-slate-500" data-testid="planning-total">
            {count(chosenCount)} event{chosenCount === 1 ? "" : "s"} in the categories chosen. The base case is always solved in AC; so are the largest shunt outages, as many as are checked in AC, since the screen can't see what a shunt does.
          </p>
        )}
        {scope && !planning && (
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
          {acReady && fromLists ? (
            <>
              Runs the contingencies a lists file names and the outages its contingency sets generate for this case. Equipment the case doesn't
              have, and events that aren't applied yet, are skipped and listed with the results.
            </>
          ) : acReady && planning ? (
            <>
              Runs the NERC TPL-001 planning events Stannum generates from the case: the base case (P0), each single element (P1), each section of a
              multi-section line (P2.1) and each bipolar DC line (P7.2). Events that need breakers, two stages or tower data are not generated yet.
            </>
          ) : acReady ? (
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
              tone={summary.with_overloads || summary.with_interface_problems ? "bad" : "good"}
              detail={
                summary.with_interface_problems
                  ? `${count(summary.with_interface_problems)} past an interface limit`
                  : summary.worst?.worst && summary.worst.overloads
                    ? `Worst ${fixed(summary.worst.worst.pct, 0)}%`
                    : summary.monitored
                      ? "Monitored branches, at their limits"
                      : `Above ${fixed(summary.limit_pct, 0)}% of ${ratingLabel(summary.rating)}`
              }
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
          {summary.categories.length > 0 && (
            <div className="flex flex-wrap gap-1.5 px-3 pb-2" data-testid="category-summary">
              {summary.categories.map((c) => {
                const problems = c.with_problems;
                return (
                  <button
                    key={c.category}
                    type="button"
                    onClick={() => setCategory(category === c.category ? "" : c.category)}
                    aria-pressed={category === c.category}
                    className={`flex items-center gap-1.5 rounded border px-2 py-1 text-[11px] ${
                      category === c.category ? "border-indigo-400 bg-indigo-50" : "border-slate-200 bg-white hover:bg-slate-50"
                    }`}
                    title={`${c.total} events; ${c.with_overloads} with overloads, ${c.with_voltage_problems} with voltage problems, ${c.not_converged} not converged`}
                  >
                    <span className="font-semibold text-slate-700">{c.category}</span>
                    <span className="tabular-nums text-slate-500">{count(c.total)}</span>
                    {problems > 0 ? (
                      <span className="tabular-nums font-semibold text-red-700">{count(problems)} with problems</span>
                    ) : (
                      <span className="text-teal-700">clear</span>
                    )}
                  </button>
                );
              })}
            </div>
          )}
          {summary.notes.length > 0 && (
            <div className="px-3 pb-2" data-testid="lists-notes">
              <Notice>
                <p className="font-medium">Skipped from the lists file:</p>
                <ul className="list-disc pl-4">
                  {summary.notes.map((n) => (
                    <li key={n}>{n}</li>
                  ))}
                </ul>
              </Notice>
            </div>
          )}
          {summary.cancelled && (
            <div className="px-3 pb-2">
              <Notice>Cancelled before the end. Outages not reached in AC keep their linear screen result.</Notice>
            </div>
          )}
          <div className="flex-1 min-h-0 flex border-t border-slate-200">
            <div className="flex-1 min-w-0 flex flex-col">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 px-3 py-2 border-b border-slate-200 bg-white">
                <SearchBox value={search} onChange={setSearch} placeholder="Search outages…" />
                {planned && (
                  <select className={inputClass} value={category} onChange={(e) => setCategory(e.target.value)} aria-label="Category">
                    <option value="">All categories</option>
                    {summary.categories.map((c) => (
                      <option key={c.category} value={c.category}>
                        {c.category}
                      </option>
                    ))}
                  </select>
                )}
                <label className="flex items-center gap-1.5 text-xs whitespace-nowrap text-slate-700">
                  <input type="checkbox" checked={problemsOnly} onChange={(e) => setProblemsOnly(e.target.checked)} />
                  Problems only
                  {problemCount === null ? "" : ` (${count(problemCount)})`}
                </label>
                <span className="ml-auto" data-testid="contingency-export">
                  <OverflowMenu
                    label="Export CSV"
                    showLabel
                    items={[
                      {
                        id: "violations",
                        label: "Every violation",
                        title: "One line per overload, voltage problem and interface violation of every outage",
                        onClick: () => void exportViolations(),
                      },
                      {
                        id: "outages",
                        label: "One row per outage",
                        title: "The table as shown: each outage's worst problem and its counts",
                        onClick: () => void exportCsv(),
                      },
                    ]}
                  />
                </span>
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
                  rowClassName={(r) =>
                    r.converged === false || r.overloads > 0 || r.interface_violations > 0
                      ? "bg-red-50/60"
                      : r.voltage_violations > 0
                        ? "bg-amber-50/60"
                        : ""
                  }
                />
              </div>
            </div>
            {openRow && (
              <OutagePanel
                row={openRow}
                detail={s.detail?.row.rank === openRow.rank ? s.detail : null}
                limit={summary.limit_pct}
                rating={summary.rating}
                monitored={summary.monitored}
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
