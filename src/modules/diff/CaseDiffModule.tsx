import { useEffect, useMemo, useState } from "react";
import { ArrowLeftRight, FileSearch, GitCompareArrows } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, pickFile, RAW_FILTERS } from "../../lib/analysis/api";
import type { DiffRecordDetail, DiffRecordRow, DiffStatus, DiffSummary } from "../../lib/analysis/types";
import { count } from "../../lib/analysis/format";
import { Button, EmptyState, ErrorBanner, Notice, Panel, SearchBox, StatusPill, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { elementLabel, elementSection } from "../../lib/elements";

const STATUSES: DiffStatus[] = ["added", "removed", "changed", "unchanged"];

function baseName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function FilePicker({ label, path, onPick }: { label: string; path: string; onPick: (p: string) => void }) {
  return (
    <div className="flex items-center gap-2 min-w-0 flex-1">
      <span className="text-[11px] font-medium text-slate-600 w-20 shrink-0">{label}</span>
      <div className={`${inputClass} flex-1 min-w-0 truncate`} title={path}>
        {path ? baseName(path) : <span className="text-slate-400">No file chosen</span>}
      </div>
      <Button
        onClick={async () => {
          const p = await pickFile(RAW_FILTERS);
          if (p) onPick(p);
        }}
      >
        Browse…
      </Button>
    </div>
  );
}

const recordColumns: Column<DiffRecordRow>[] = [
  { key: "key", header: "Record key", value: (r) => r.key, render: (r) => <span className="font-mono">{r.key}</span> },
  { key: "name", header: "Name", value: (r) => r.name, render: (r) => <span className="font-mono">{r.name}</span> },
  { key: "status", header: "Status", value: (r) => r.status, render: (r) => <StatusPill status={r.status} /> },
];

const rowTone: Record<DiffStatus, string> = {
  added: "bg-emerald-50/70",
  removed: "bg-red-50/70",
  changed: "bg-amber-50/70",
  unchanged: "",
};

export function CaseDiffModule({ active }: { active: boolean }) {
  const openSummary = useWorkspace((s) => s.caseSummary);
  const [baseline, setBaseline] = useState("");
  const [comparison, setComparison] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<DiffSummary | null>(null);
  const [section, setSection] = useState<string | null>(null);
  const [records, setRecords] = useState<DiffRecordRow[]>([]);
  const [recordsFor, setRecordsFor] = useState<string | null>(null);
  const [includeUnchanged, setIncludeUnchanged] = useState(false);
  const [statusFilter, setStatusFilter] = useState<DiffStatus | "">("");
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<number | null>(null);
  const [detail, setDetail] = useState<DiffRecordDetail | null>(null);
  const [changedOnly, setChangedOnly] = useState(false);
  const shared = useWorkspace((s) => s.selection);
  const select = useWorkspace((s) => s.select);
  // A selection from another module waiting for its section's records to load.
  const [pending, setPending] = useState<string | null>(null);
  const [missing, setMissing] = useState<string | null>(null);

  useEffect(() => {
    setMissing(null);
    if (!shared || shared.from === "diff" || !summary) return;
    const target = elementSection(shared.element);
    if (!target || !summary.sections.some((x) => x.name === target)) {
      setMissing(shared.element);
      return;
    }
    setPending(shared.element);
    setSection(target);
  }, [shared, summary]);

  useEffect(() => {
    if (!pending) return;
    const target = elementSection(pending);
    if (target !== section || recordsFor !== section) return;
    const row = records.find((r) => r.element === pending);
    setPending(null);
    if (!row) {
      setMissing(pending);
      return;
    }
    if (row.status === "unchanged") setIncludeUnchanged(true);
    if (statusFilter && statusFilter !== row.status) setStatusFilter("");
    setSearch("");
    setSelected(row.index);
  }, [pending, records, recordsFor, section, statusFilter]);

  // Offer the open RAW case as the baseline.
  useEffect(() => {
    if (openSummary?.diffable && !baseline) setBaseline(openSummary.path);
  }, [openSummary, baseline]);

  function clearResults() {
    setSummary(null);
    setSection(null);
    setRecords([]);
    setSelected(null);
    setDetail(null);
  }

  async function compare() {
    if (!baseline || !comparison) {
      setError("Choose both files before comparing.");
      return;
    }
    setBusy(true);
    setError(null);
    clearResults();
    try {
      const s = await api.diffCompare(baseline, comparison);
      setSummary(s);
      const first = s.sections.find((x) => x.counts.added + x.counts.removed + x.counts.changed > 0) ?? s.sections[0];
      setSection(first?.name ?? null);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    setSelected(null);
    setDetail(null);
    if (!summary || !section) {
      setRecords([]);
      return;
    }
    let cancelled = false;
    setRecords([]);
    setRecordsFor(null);
    const loading = section;
    api
      .diffSectionRecords(section)
      .then((r) => {
        if (cancelled) return;
        setRecords(r);
        setRecordsFor(loading);
      }).catch((e) => setError((e as Error).message));
    return () => {
      cancelled = true;
    };
  }, [summary, section]);

  useEffect(() => {
    if (selected === null || !section) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    api.diffRecordDetail(section, selected).then((d) => !cancelled && setDetail(d)).catch((e) => setError((e as Error).message));
    return () => {
      cancelled = true;
    };
  }, [selected, section]);

  const visibleRecords = useMemo(
    () =>
      records.filter(
        (r) => (includeUnchanged || r.status !== "unchanged") && (!statusFilter || r.status === statusFilter)
      ),
    [records, includeUnchanged, statusFilter]
  );

  const fields = useMemo(
    () => (detail ? detail.fields.filter((f) => !changedOnly || f.status !== "unchanged").map((f, i) => ({ ...f, i })) : []),
    [detail, changedOnly]
  );

  return (
    <div className="h-full flex flex-col gap-3 p-3 overflow-hidden" aria-hidden={!active}>
      <Panel eyebrow="RAW revisions 33 and 35" title="Compare two cases">
        <div className="flex flex-col gap-2 p-3">
          <div className="flex items-center gap-3">
            <FilePicker label="Baseline" path={baseline} onPick={(p) => { setBaseline(p); clearResults(); }} />
            <Button
              variant="ghost"
              title="Swap files"
              aria-label="Swap files"
              onClick={() => {
                setBaseline(comparison);
                setComparison(baseline);
                clearResults();
              }}
            >
              <ArrowLeftRight size={14} />
            </Button>
            <FilePicker label="Comparison" path={comparison} onPick={(p) => { setComparison(p); clearResults(); }} />
            <Button variant="primary" onClick={compare} disabled={busy || !baseline || !comparison}>
              <GitCompareArrows size={14} /> {busy ? "Comparing…" : "Compare files"}
            </Button>
          </div>
          {openSummary?.diffable && baseline !== openSummary.path && (
            <button className="self-start text-[11px] text-teal-700 underline" onClick={() => { setBaseline(openSummary.path); clearResults(); }}>
              Use the open case ({openSummary.name}) as the baseline
            </button>
          )}
        </div>
      </Panel>

      {error && <ErrorBanner>{error}</ErrorBanner>}
      {missing && summary && (
        <Notice>
          {elementLabel(missing)} is not in this comparison.
        </Notice>
      )}

      {!summary ? (
        <div className="flex-1 min-h-0">
          <EmptyState icon={<FileSearch size={22} />} title="Exact, record-level comparison">
            Records are matched by key in each section. Numbers compare exactly (1.0 equals 1.000, but any other change is
            flagged), string padding is shown in quotes, and formatting, comments and line endings are ignored.
          </EmptyState>
        </div>
      ) : (
        <>
          <div className="flex items-center gap-3 text-xs text-slate-600">
            <span>
              <strong>{baseName(summary.baseline.name)}</strong> (rev {summary.baseline.revision}) →{" "}
              <strong>{baseName(summary.comparison.name)}</strong> (rev {summary.comparison.revision})
            </span>
            <span className={`font-semibold ${summary.differing ? "text-amber-700" : "text-emerald-700"}`}>
              {summary.differing ? `${count(summary.differing)} differing records` : "No differences"}
            </span>
          </div>
          {!summary.complete && (
            <Notice>
              <strong>Incomplete comparison.</strong> Some content uses layouts that are not verified, so counts cover
              supported records only and do not establish full case equality.
              <details className="mt-1">
                <summary className="cursor-pointer">Unsupported content ({summary.diagnostics.length})</summary>
                <ul className="mt-1 max-h-40 overflow-y-auto font-mono text-[11px]">
                  {summary.diagnostics.map((d, i) => (
                    <li key={i} className="py-0.5">
                      {d.side === "baseline" ? "Baseline" : "Comparison"} · {d.section}, line {d.line}: {d.message}
                      {d.raw_lines.length > 0 && <pre className="whitespace-pre-wrap text-slate-600">{d.raw_lines.join("\n")}</pre>}
                    </li>
                  ))}
                </ul>
              </details>
            </Notice>
          )}
          <div className="flex-1 min-h-0 grid grid-cols-[230px_1fr_1fr] gap-3">
            <Panel eyebrow="Sections" title={`${summary.sections.length} sections`} bodyClassName="overflow-y-auto">
              <ul>
                {summary.sections.map((s) => {
                  const diff = s.counts.added + s.counts.removed + s.counts.changed;
                  return (
                    <li key={s.name}>
                      <button
                        onClick={() => setSection(s.name)}
                        aria-current={section === s.name}
                        className={`w-full text-left px-3 py-1.5 border-b border-slate-100 ${section === s.name ? "bg-teal-50" : "hover:bg-slate-50"}`}
                      >
                        <span className="text-xs font-medium text-slate-800 flex justify-between">
                          {s.name}
                          {diff > 0 && <span className="text-amber-700">{count(diff)}</span>}
                        </span>
                        <span className="text-[10px] text-slate-500">
                          +{s.counts.added} · −{s.counts.removed} · ~{s.counts.changed} · ={s.counts.unchanged}
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            </Panel>
            <Panel
              eyebrow="Records"
              title={section ?? ""}
              bodyClassName="flex flex-col"
            >
              <div className="flex flex-wrap items-center gap-2 px-3 py-1.5 border-b border-slate-200">
                <SearchBox value={search} onChange={setSearch} placeholder="Search key, name or status…" />
                <select className={inputClass} value={statusFilter} onChange={(e) => setStatusFilter(e.target.value as DiffStatus | "")} aria-label="Status filter">
                  <option value="">Any status</option>
                  {STATUSES.map((s) => (
                    <option key={s} value={s}>
                      {s[0].toUpperCase() + s.slice(1)}
                    </option>
                  ))}
                </select>
                <label className="flex items-center gap-1.5 text-xs text-slate-700">
                  <input type="checkbox" checked={includeUnchanged} onChange={(e) => setIncludeUnchanged(e.target.checked)} />
                  Include unchanged
                </label>
              </div>
              <div className="flex-1 min-h-0">
                <DataTable
                  key={section ?? ""}
                  rows={visibleRecords}
                  columns={recordColumns}
                  rowKey={(r) => r.index}
                  search={search}
                  onRowClick={(r) => {
                    setSelected(r.index);
                    setMissing(null);
                    if (r.element) select(r.element, "diff");
                  }}
                  selectedKey={selected}
                  rowClassName={(r) => rowTone[r.status]}
                  empty={records.length ? "No differences in this section. Include unchanged to inspect records." : "No records."}
                />
              </div>
            </Panel>
            <Panel
              eyebrow="Fields"
              title={detail ? <span className="font-mono">{detail.key}</span> : "Select a record"}
              actions={
                <label className="flex items-center gap-1.5 text-xs text-slate-700">
                  <input type="checkbox" checked={changedOnly} onChange={(e) => setChangedOnly(e.target.checked)} />
                  Changed fields only
                </label>
              }
              bodyClassName="flex flex-col"
            >
              {detail ? (
                <>
                  <div className="flex-1 min-h-0">
                    <DataTable
                      key={`${detail.section}-${detail.key}`}
                      rows={fields}
                      rowKey={(f) => f.i}
                      rowClassName={(f) => rowTone[f.status]}
                      columns={[
                        { key: "name", header: "Field", value: (f) => f.name },
                        { key: "before", header: "Baseline", value: (f) => f.before, render: (f) => <span className="font-mono">{f.before ?? <em className="text-slate-400">Absent</em>}</span> },
                        { key: "after", header: "Comparison", value: (f) => f.after, render: (f) => <span className="font-mono">{f.after ?? <em className="text-slate-400">Absent</em>}</span> },
                        { key: "status", header: "Status", value: (f) => f.status, render: (f) => <StatusPill status={f.status} /> },
                      ]}
                    />
                  </div>
                  <details className="border-t border-slate-200 px-3 py-2 text-[11px]" open>
                    <summary className="cursor-pointer font-medium text-slate-700">Original lines</summary>
                    <p className="mt-1 text-slate-500">Baseline</p>
                    <pre className="font-mono whitespace-pre-wrap break-all bg-slate-50 rounded p-1.5 max-h-28 overflow-auto">{detail.before_lines?.join("\n") ?? "Absent"}</pre>
                    <p className="mt-1 text-slate-500">Comparison</p>
                    <pre className="font-mono whitespace-pre-wrap break-all bg-slate-50 rounded p-1.5 max-h-28 overflow-auto">{detail.after_lines?.join("\n") ?? "Absent"}</pre>
                  </details>
                </>
              ) : (
                <div className="p-6 text-center text-xs text-slate-500">Choose a record to see its fields side by side.</div>
              )}
            </Panel>
          </div>
        </>
      )}
    </div>
  );
}
