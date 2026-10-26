// Two solves side by side: what the study's edits changed.
import { useMemo } from "react";
import { ArrowRight } from "lucide-react";
import type { SolveReport } from "../../lib/analysis/types";
import { compareSolves, type BranchChange, type BusChange } from "../../lib/analysis/compareSolves";
import { count, fixed } from "../../lib/analysis/format";
import { busElement } from "../../lib/elements";
import { useStudy, type CompareSide } from "../../store/studyStore";
import { DataTable, type Column } from "../../ui/DataTable";
import { Panel, inputClass } from "../../ui/primitives";

const signed = (v: number, digits: number) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${fixed(Math.abs(v), digits)}`;

const pct = (v: number | null) => (v === null ? "—" : `${fixed(v, 1)}%`);

const branchColumns: Column<BranchChange>[] = [
  { key: "from", header: "From", value: (r) => r.from, align: "right" },
  { key: "to", header: "To", value: (r) => r.to, align: "right" },
  { key: "circuit", header: "Circuit", value: (r) => r.circuit },
  {
    key: "status",
    header: "Status",
    value: (r) => (r.onlineA === r.onlineB ? (r.onlineB ? "Online" : "Offline") : r.onlineB ? "Put back" : "Taken out"),
    render: (r) =>
      r.onlineA === r.onlineB ? (r.onlineB ? "Online" : "Offline") : <span className="font-medium text-amber-800">{r.onlineB ? "Put back" : "Taken out"}</span>,
  },
  { key: "pA", header: "P before (MW)", value: (r) => r.pA, render: (r) => fixed(r.pA), align: "right" },
  { key: "pB", header: "P after (MW)", value: (r) => r.pB, render: (r) => fixed(r.pB), align: "right" },
  {
    key: "dp",
    header: "Change (MW)",
    value: (r) => r.dp,
    render: (r) => <span className="font-medium">{signed(r.dp, 2)}</span>,
    align: "right",
  },
  {
    key: "loading",
    header: "Loading",
    value: (r) => r.loadingB,
    render: (r) => (
      <span className={(r.loadingB ?? 0) > 100 ? "text-red-700 font-semibold" : ""}>
        {pct(r.loadingA)} → {pct(r.loadingB)}
      </span>
    ),
    align: "right",
  },
];

const busColumns: Column<BusChange>[] = [
  { key: "id", header: "Bus", value: (r) => r.id, align: "right" },
  { key: "name", header: "Name", value: (r) => r.name },
  { key: "vmA", header: "Vm before", value: (r) => r.vmA, render: (r) => fixed(r.vmA, 4), align: "right" },
  { key: "vmB", header: "Vm after", value: (r) => r.vmB, render: (r) => fixed(r.vmB, 4), align: "right" },
  { key: "dvm", header: "Change (p.u.)", value: (r) => r.dvm, render: (r) => <span className="font-medium">{signed(r.dvm, 4)}</span>, align: "right" },
  { key: "dva", header: "Angle change (°)", value: (r) => r.vaB - r.vaA, render: (r) => signed(r.vaB - r.vaA, 2), align: "right" },
];

function SidePicker({ label, value, onChange, current }: { label: string; value: CompareSide; onChange: (v: CompareSide) => void; current: boolean }) {
  const saved = useStudy((s) => s.saved);
  return (
    <label className="flex items-center gap-1.5 text-xs text-slate-600">
      {label}
      <select
        aria-label={label}
        className={`${inputClass} max-w-[16rem]`}
        value={String(value)}
        onChange={(e) => onChange(e.target.value === "current" ? "current" : Number(e.target.value))}
      >
        {saved.map((s) => (
          <option key={s.id} value={s.id}>
            {s.name}
          </option>
        ))}
        {current && <option value="current">Current solve</option>}
      </select>
    </label>
  );
}

export function CompareSolves({ current, onShow }: { current: SolveReport | null; onShow: (element: string) => void }) {
  const saved = useStudy((s) => s.saved);
  const compare = useStudy((s) => s.compare);
  const setCompare = useStudy((s) => s.setCompare);
  const pickReport = (side: CompareSide) => (side === "current" ? current : (saved.find((s) => s.id === side)?.report ?? null));
  const a = compare ? pickReport(compare.a) : null;
  const b = compare ? pickReport(compare.b) : null;
  const result = useMemo(() => (a && b ? compareSolves(a, b) : null), [a, b]);
  if (!compare || saved.length === 0) return null;
  const moved = result?.branches.filter((r) => Math.abs(r.dp) >= 0.01 || r.onlineA !== r.onlineB) ?? [];
  const shifted = result?.buses.filter((r) => Math.abs(r.dvm) >= 1e-4) ?? [];
  return (
    <Panel
      className="shrink-0"
      title="Compare solves"
      eyebrow="What changed"
      actions={
        <span className="flex flex-wrap items-center gap-2">
          <SidePicker label="Before" value={compare.a} onChange={(v) => setCompare({ ...compare, a: v })} current={!!current} />
          <ArrowRight size={13} className="text-slate-400" />
          <SidePicker label="After" value={compare.b} onChange={(v) => setCompare({ ...compare, b: v })} current={!!current} />
        </span>
      }
    >
      {!result ? (
        <p className="p-4 text-xs text-slate-500">Solve the case to compare against the current solve.</p>
      ) : compare.a === compare.b ? (
        <p className="p-4 text-xs text-slate-500">Pick two different solves.</p>
      ) : (
        <div className="flex flex-col gap-3 p-3" data-testid="solve-comparison">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-2">
            {result.totals.map((t) => {
              const d = t.b - t.a;
              return (
                <div key={t.label} className="rounded-md border border-slate-200 px-3 py-2">
                  <p className="text-[10px] font-semibold uppercase tracking-wider text-slate-400">{t.label}</p>
                  <p className="text-sm font-semibold text-slate-800 tabular-nums">
                    {t.unit ? fixed(t.b, 1) : count(t.b)} {t.unit}
                  </p>
                  <p className={`text-[11px] tabular-nums ${d === 0 ? "text-slate-400" : "text-slate-600"}`}>
                    {d === 0 ? "No change" : `${signed(d, t.unit ? 2 : 0)} ${t.unit} from ${t.unit ? fixed(t.a, 1) : count(t.a)}`}
                  </p>
                </div>
              );
            })}
          </div>
          {(result.appeared.length > 0 || result.cleared.length > 0) && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-2 text-xs">
              {[
                { title: "New violations", list: result.appeared, tone: "text-red-700" },
                { title: "Cleared violations", list: result.cleared, tone: "text-emerald-700" },
              ].map(
                (g) =>
                  g.list.length > 0 && (
                    <div key={g.title} className="rounded-md border border-slate-200 px-3 py-2">
                      <p className={`font-semibold ${g.tone}`}>
                        {g.title} ({g.list.length})
                      </p>
                      <ul className="mt-1 flex flex-col gap-0.5">
                        {g.list.slice(0, 8).map((v) => (
                          <li key={v.label}>
                            <button type="button" className="text-left hover:underline" onClick={() => onShow(v.element)}>
                              {v.label}: <span className="tabular-nums">{v.amount}</span>
                            </button>
                          </li>
                        ))}
                        {g.list.length > 8 && <li className="text-slate-500">and {g.list.length - 8} more</li>}
                      </ul>
                    </div>
                  )
              )}
            </div>
          )}
          <div className="grid grid-cols-1 2xl:grid-cols-2 gap-3">
            <section className="flex flex-col">
              <p className="text-xs font-medium text-slate-700 mb-1">
                Branch flows that moved ({count(moved.length)}), largest first
              </p>
              <div className="h-72 rounded-md border border-slate-200 overflow-hidden" data-testid="compare-branches">
                <DataTable rows={moved} columns={branchColumns} rowKey={(r) => r.index} onRowClick={(r) => onShow(r.element)} empty="No branch flow changed." />
              </div>
            </section>
            <section className="flex flex-col">
              <p className="text-xs font-medium text-slate-700 mb-1">
                Bus voltages that moved ({count(shifted.length)}), largest first
              </p>
              <div className="h-72 rounded-md border border-slate-200 overflow-hidden" data-testid="compare-buses">
                <DataTable rows={shifted} columns={busColumns} rowKey={(r) => r.id} onRowClick={(r) => onShow(busElement(r.id))} empty="No bus voltage changed." />
              </div>
            </section>
          </div>
        </div>
      )}
    </Panel>
  );
}
