// Records: every record in the open case as a table, one record type at a
// time, with every field. Sort by a column, search, or filter with chips;
// picking a row opens it in the element inspector, with "Draw around".
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { AlertTriangle, ChevronRight, Download, FolderOpen, Sheet } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, saveTextExport } from "../../lib/analysis/api";
import type { CaseRecord, RecordBook, RecordTable } from "../../lib/analysis/types";
import { count, toCsv } from "../../lib/analysis/format";
import type { FilterRule } from "../../lib/filters";
import { elementLabel } from "../../lib/elements";
import { Button, EmptyState, ErrorBanner, Notice, SearchBox, inputClass } from "../../ui/primitives";
import { DataTable, filterRows, type Column } from "../../ui/DataTable";
import { FilterChips } from "../../ui/FilterChips";
import { ElementInspector, inspectorClass } from "../../ui/ElementInspector";
import { DrawAroundButton } from "../../ui/DrawAround";
import { useWidth } from "../../ui/useWidth";
import { columnOptions, csvRows, passes, recordFields, sectionFor } from "./records";

/** Below this width the record types fold into a menu. */
const LIST_MIN = 720;
/** Below this width the inspector floats over the table. */
const INSPECTOR_MIN = 1060;

function columnsFor(table: RecordTable, raw: boolean): Column<CaseRecord>[] {
  const cols: Column<CaseRecord>[] = table.columns.map((c, i) => ({
    key: String(i),
    header: c.name,
    title: `${c.name} (${c.kind})`,
    value: (r) => r.cells[i] ?? null,
    align: c.kind === "text" ? "left" : "right",
  }));
  if (raw) cols.push({ key: "line", header: "Line", title: "Line in the RAW file where the record starts", value: (r) => r.line, align: "right" });
  return cols;
}

/** The record types, with counts; empty ones fold away at the bottom. */
function SectionList({ book, value, onChange }: { book: RecordBook; value: string | null; onChange: (id: string) => void }) {
  const filled = book.sections.filter((s) => s.count > 0 || s.skipped > 0);
  const empty = book.sections.filter((s) => s.count === 0 && s.skipped === 0);
  const item = (s: RecordBook["sections"][number]) => (
    <button
      key={s.id}
      type="button"
      onClick={() => onChange(s.id)}
      aria-current={value === s.id ? "true" : undefined}
      title={s.skipped ? `${count(s.skipped)} not listed` : undefined}
      className={`w-full flex items-center gap-1.5 rounded px-2 py-1 text-left text-xs ${
        value === s.id ? "bg-teal-50 text-teal-900 font-medium ring-1 ring-inset ring-teal-200" : "text-slate-700 hover:bg-slate-100"
      } ${s.count === 0 ? "text-slate-400" : ""}`}
    >
      <span className="flex-1 min-w-0 truncate">{s.label}</span>
      {s.skipped > 0 && <AlertTriangle size={11} className="shrink-0 text-amber-600" aria-label="Some not listed" />}
      <span className="shrink-0 tabular-nums text-[11px] text-slate-500">{count(s.count)}</span>
    </button>
  );
  return (
    <nav aria-label="Record types" className="w-52 shrink-0 border-r border-slate-200 bg-white overflow-y-auto p-1.5 flex flex-col gap-0.5" data-testid="record-types">
      {filled.map(item)}
      {empty.length > 0 && (
        <details className="mt-1.5 group">
          <summary className="flex items-center gap-1 cursor-pointer select-none px-2 py-1 text-[11px] text-slate-500 hover:text-slate-700 list-none">
            <ChevronRight size={11} className="transition-transform group-open:rotate-90" />
            Empty ({empty.length})
          </summary>
          <div className="flex flex-col gap-0.5">{empty.map(item)}</div>
        </details>
      )}
    </nav>
  );
}

export function RecordsModule({ active }: { active: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const openCase = useWorkspace((s) => s.openCase);
  const selection = useWorkspace((s) => s.selection);
  const linked = useWorkspace((s) => s.linked);
  const select = useWorkspace((s) => s.select);
  const split = useWorkspace((s) => s.split);

  const [book, setBook] = useState<RecordBook | null>(null);
  const [tables, setTables] = useState<Record<string, RecordTable>>({});
  const [error, setError] = useState<string | null>(null);
  const [section, setSection] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [rulesBy, setRulesBy] = useState<Record<string, FilterRule[]>>({});
  // Picked here; while linked (or after a search), the shared selection wins.
  const [picked, setPicked] = useState<string | null>(null);
  const [pickedRow, setPickedRow] = useState<CaseRecord | null>(null);
  const selected = linked || selection?.from === "search" ? (selection?.element ?? null) : picked;

  const [rootRef, width] = useWidth<HTMLDivElement>(1200);
  const showList = width >= LIST_MIN;
  const dockInspector = width >= INSPECTOR_MIN;

  const readable = !!summary && (summary.power.available || summary.diffable);

  // A new case starts over.
  useEffect(() => {
    setBook(null);
    setTables({});
    setError(null);
    setSection(null);
    setSearch("");
    setRulesBy({});
    setPicked(null);
    setPickedRow(null);
  }, [caseVersion]);

  // The record types, read the first time the module is shown for a case.
  useEffect(() => {
    if (!active || !readable || book || error) return;
    const version = caseVersion;
    api
      .caseRecords()
      .then((b) => {
        if (version !== useWorkspace.getState().caseVersion) return;
        setBook(b);
        setSection((s) => s ?? b.sections.find((x) => x.count > 0)?.id ?? b.sections[0]?.id ?? null);
      })
      .catch((e: Error) => version === useWorkspace.getState().caseVersion && setError(e.message));
  }, [active, readable, book, error, caseVersion]);

  // Each table's rows, read when first shown.
  const table = section ? tables[section] : undefined;
  useEffect(() => {
    if (!active || !section || table) return;
    const version = caseVersion;
    api
      .recordTable(section)
      .then((t) => version === useWorkspace.getState().caseVersion && setTables((all) => ({ ...all, [t.id]: t })))
      .catch((e: Error) => version === useWorkspace.getState().caseVersion && setError(e.message));
  }, [active, section, table, caseVersion]);

  // Follow a selection made elsewhere to the table that lists it.
  useEffect(() => {
    if (!book || !selection || selection.from === "records") return;
    if (table?.rows.some((r) => r.element === selection.element)) return;
    const next = sectionFor(book, selection.element);
    if (next && next !== section) {
      setSection(next);
      setSearch("");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selection, book]);

  const raw = book?.source === "raw";
  const columns = useMemo(() => (table ? columnsFor(table, raw) : []), [table, raw]);
  const fields = useMemo(() => (table ? recordFields(table) : []), [table]);
  const rules = (section && rulesBy[section]) || [];
  // The picked record stays listed even when a filter would hide it.
  const shown = useMemo(
    () => (table ? (rules.length ? table.rows.filter((r) => passes(r, rules) || (selected !== null && r.element === selected)) : table.rows) : []),
    [table, rules, selected]
  );
  const keys = useMemo(() => new Map(table?.rows.map((r, i) => [r, i]) ?? []), [table]);

  const pick = (row: CaseRecord) => {
    setPickedRow(row);
    if (row.element) {
      setPicked(row.element);
      select(row.element, "records");
    }
  };
  const inspect = (element: string) => {
    setPicked(element);
    setPickedRow(null);
    select(element, "records");
  };

  if (!summary) {
    return (
      <EmptyState
        icon={<Sheet size={22} />}
        title="Records"
        action={
          <Button variant="primary" onClick={() => openCase()}>
            <FolderOpen size={14} /> Open case…
          </Button>
        }
      >
        Open a PSS/E RAW or MATPOWER case to see every bus, load, generator, branch, transformer, shunt and DC record as a table, with every
        field. Sort, search and filter them, and pick one to inspect it.
      </EmptyState>
    );
  }
  if (!readable) {
    return (
      <div className="p-6 max-w-2xl">
        <ErrorBanner>
          <strong>Stannum couldn't read this case's records.</strong> {summary.power.error}
        </ErrorBanner>
      </div>
    );
  }

  const exportCsv = async () => {
    if (!table) return;
    const rows = filterRows(shown, columns, search);
    await saveTextExport(`${summary.name.replace(/\.(raw|m)$/i, "")}-${table.label.toLowerCase().replace(/\s+/g, "-")}.csv`, toCsv(csvRows(table, rows)), {
      name: "CSV",
      extensions: ["csv"],
    });
  };

  let body: ReactNode;
  if (error) body = <div className="p-4"><ErrorBanner>{error}</ErrorBanner></div>;
  else if (!book) body = <p className="p-4 text-xs text-slate-500">Reading the case's records…</p>;
  else if (!section || !table) body = <p className="p-4 text-xs text-slate-500">Reading records…</p>;
  else {
    const filtered = rules.length > 0;
    body = (
      <>
        <div className="flex flex-wrap items-center gap-2 px-3 py-1.5 border-b border-slate-200 bg-white">
          {!showList && (
            <select
              aria-label="Record type"
              className={`${inputClass} max-w-48`}
              value={section}
              onChange={(e) => {
                setSection(e.target.value);
                setSearch("");
              }}
            >
              {book.sections.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label} ({count(s.count)})
                </option>
              ))}
            </select>
          )}
          <SearchBox value={search} onChange={setSearch} placeholder={`Search ${table.label.toLowerCase()}…`} />
          <span className="text-[11px] text-slate-500 truncate" data-testid="records-summary">
            {count(table.columns.length)} fields
            {raw && book.revision !== null ? ` · RAW revision ${book.revision}` : book.source === "model" ? " · power flow model" : ""}
          </span>
          <span className="ml-auto flex items-center gap-1">
            {selected && !(dockInspector || !split) && (
              // The inspector isn't shown here (a narrow pane side by side), so its actions are.
              <span className="hidden @2xl:flex items-center gap-1 min-w-0 text-xs text-slate-600" data-testid="records-selection">
                <span className="truncate font-medium text-slate-800">{elementLabel(selected)}</span>
                <DrawAroundButton element={selected} from="records" />
              </span>
            )}
            <Button variant="ghost" onClick={exportCsv} title="Save the rows shown as CSV">
              <Download size={13} /> CSV
            </Button>
          </span>
        </div>
        <FilterChips
          fields={fields}
          options={(key) => columnOptions(table, key)}
          rules={rules}
          onRules={(next) => setRulesBy((all) => ({ ...all, [section]: next }))}
          note={filtered ? `${count(shown.length)} of ${count(table.rows.length)} ${table.label.toLowerCase()}` : "Filter on any field"}
        />
        {(book.note || table.note) && (
          <div className="px-3 pt-2 flex flex-col gap-1.5">
            {book.note && <Notice>{book.note}</Notice>}
            {table.note && <Notice>{table.note}</Notice>}
          </div>
        )}
        <div className="flex-1 min-h-0" data-testid="records-table">
          <DataTable
            key={section}
            rows={shown}
            columns={columns}
            rowKey={(r) => keys.get(r) ?? r.line}
            search={search}
            onRowClick={pick}
            isSelected={(r) => (selected !== null && r.element === selected) || r === pickedRow}
            rowClassName={(r) => (r.element ? "" : "text-slate-600")}
            empty={table.rows.length ? "No records match." : `The case has no ${table.label.toLowerCase()}.`}
          />
        </div>
      </>
    );
  }

  return (
    <div ref={rootRef} className="@container h-full flex relative bg-slate-50" aria-hidden={!active} data-testid="records-module">
      {book && showList && (
        <SectionList
          book={book}
          value={section}
          onChange={(id) => {
            setSection(id);
            setSearch("");
          }}
        />
      )}
      <div className="flex-1 min-w-0 flex flex-col">{body}</div>
      {selected && (dockInspector || !split) && (
        // Side by side in a narrow pane, the other pane's inspector shows it.
        <aside className={inspectorClass(!dockInspector)} data-testid="records-inspector">
          <ElementInspector
            element={selected}
            from="records"
            onPick={inspect}
            onClose={() => {
              setPicked(null);
              setPickedRow(null);
              select(null, "records");
            }}
          />
        </aside>
      )}
    </div>
  );
}
