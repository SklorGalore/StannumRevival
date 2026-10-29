// Records: every record in the open case as a table, one record type at a
// time, with every field. Sort by a column, search, or filter with chips;
// picking a row opens it in the element inspector, with "Draw around".
// Double-click a cell to edit it; Apply writes the edits into the case, which
// every module then reloads, and Save as writes the edited case to a file.
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { AlertTriangle, Check, ChevronRight, Download, FolderOpen, Lock, Save, Sheet, Undo2, X } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, saveTextExport } from "../../lib/analysis/api";
import type { CaseRecord, RecordBook, RecordColumnKind, RecordEdit, RecordTable } from "../../lib/analysis/types";
import { count, toCsv } from "../../lib/analysis/format";
import type { FieldDefinition, FilterRule } from "../../lib/filters";
import { elementLabel } from "../../lib/elements";
import { Button, EmptyState, ErrorBanner, Notice, SearchBox, inputClass } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { allRows, type RowSource } from "../../ui/usePagedRows";
import { FilterChips } from "../../ui/FilterChips";
import { ElementInspector, inspectorClass } from "../../ui/ElementInspector";
import { DrawAroundButton } from "../../ui/DrawAround";
import { useWidth } from "../../ui/useWidth";
import { useStudyFile } from "../../store/studyFileStore";
import { RecentFiles } from "../../ui/RecentFiles";
import { SampleCases } from "../../ui/SampleCases";
import { cellProblem, csvRows, sectionFor } from "./records";

/** Below this width the record types fold into a menu. */
const LIST_MIN = 720;
/** Below this width the inspector floats over the table. */
const INSPECTOR_MIN = 1060;

/** A cell being edited, by table position. */
/** Longer than any system's double-click time. */
const DOUBLE_CLICK_MS = 1000;

interface CellRef {
  row: number;
  column: number;
}

const editKey = (section: string, row: number, column: number) => `${section}\u0000${row}\u0000${column}`;

function shownValue(v: CaseRecord["cells"][number]): string {
  return v === null || v === undefined ? "" : String(v);
}

/** The text box over a cell while it is edited. Enter or Tab keeps the value, Esc drops it. */
function CellEditor({
  initial,
  kind,
  align,
  onDone,
}: {
  initial: string;
  kind: RecordColumnKind;
  align: "left" | "right";
  /** value null: cancelled; next: Tab pressed. */
  onDone: (value: string | null, next?: boolean) => void;
}) {
  const [text, setText] = useState(initial);
  const problem = cellProblem(kind, text);
  const done = useRef(false);
  const finish = (value: string | null, next?: boolean) => {
    if (done.current) return;
    done.current = true;
    onDone(value, next);
  };
  return (
    <input
      autoFocus
      aria-label="Cell value"
      aria-invalid={problem !== null}
      title={problem ?? undefined}
      value={text}
      onFocus={(e) => e.currentTarget.select()}
      onClick={(e) => e.stopPropagation()}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") finish(null);
        else if ((e.key === "Enter" || e.key === "Tab") && !problem) {
          e.preventDefault();
          finish(text, e.key === "Tab");
        } else if (e.key === "Tab") e.preventDefault();
      }}
      onBlur={() => finish(problem ? null : text)}
      className={`w-full min-w-16 h-6 px-1 rounded-sm border text-xs tabular-nums bg-white focus:outline-none ${
        align === "right" ? "text-right" : "text-left"
      } ${problem ? "border-red-400 ring-2 ring-red-200" : "border-teal-500 ring-2 ring-teal-500/30"}`}
    />
  );
}

/** A record type's columns and counts. Its rows are read a page at a time. */
interface TableInfo {
  id: string;
  label: string;
  columns: RecordTable["columns"];
  skipped: number;
  note: string | null;
  total: number;
  fields: FieldDefinition[];
}

/** A record with its position in the whole table, which edits refer to. */
type PagedRecord = CaseRecord & { position: number };

interface EditState {
  editable: boolean;
  section: string;
  rowIndex: (r: PagedRecord) => number;
  pending: Record<string, string>;
  editing: CellRef | null;
  start: (cell: CellRef) => void;
  finish: (cell: CellRef, value: string | null, next?: boolean) => void;
}

function columnsFor(table: TableInfo, raw: boolean, edit: EditState): Column<PagedRecord>[] {
  const cols: Column<PagedRecord>[] = table.columns.map((c, i) => {
    const align = c.kind === "text" ? "left" : "right";
    const canEdit = edit.editable && c.editable;
    return {
      key: String(i),
      header: canEdit || !edit.editable ? c.name : (
        <span className="inline-flex items-center gap-0.5">
          {c.name}
          <Lock size={9} className="text-slate-400" aria-hidden="true" />
        </span>
      ),
      title: `${c.name} (${c.kind})${edit.editable && !c.editable ? ": identifies the record, so it can't be edited" : ""}`,
      value: (r) => r.cells[i] ?? null,
      align,
      render: (r) => {
        const row = edit.rowIndex(r);
        const pending = edit.pending[editKey(edit.section, row, i)];
        if (edit.editing && edit.editing.row === row && edit.editing.column === i)
          return (
            <CellEditor
              initial={pending ?? shownValue(r.cells[i])}
              kind={c.kind}
              align={align}
              onDone={(value, next) => edit.finish({ row, column: i }, value, next)}
            />
          );
        const text = pending ?? shownValue(r.cells[i]);
        if (!canEdit) return text;
        return (
          <span
            className={`block -mx-2.5 px-2.5 leading-7 cursor-text ${pending !== undefined ? "bg-amber-100 text-amber-950 font-medium" : ""}`}
            data-pending={pending !== undefined ? "" : undefined}
            data-cell={`${row}:${i}`}
            title={pending !== undefined ? `Was ${shownValue(r.cells[i]) || "empty"}` : "Double-click to edit"}
            onDoubleClick={(e) => {
              e.stopPropagation();
              edit.start({ row, column: i });
            }}
          >
            {text || "\u00a0"}
          </span>
        );
      },
    };
  });
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
  const openFile = useStudyFile((s) => s.openFile);
  const selection = useWorkspace((s) => s.selection);
  const linked = useWorkspace((s) => s.linked);
  const select = useWorkspace((s) => s.select);
  const split = useWorkspace((s) => s.split);
  const editedVersion = useWorkspace((s) => s.editedVersion);
  const applyRecordEdits = useWorkspace((s) => s.applyRecordEdits);
  const saveCaseAs = useWorkspace((s) => s.saveCaseAs);

  const [book, setBook] = useState<RecordBook | null>(null);
  const [tables, setTables] = useState<Record<string, TableInfo>>({});
  const [error, setError] = useState<string | null>(null);
  const [section, setSection] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [rulesBy, setRulesBy] = useState<Record<string, FilterRule[]>>({});
  // Picked here; while linked (or after a search), the shared selection wins.
  const [picked, setPicked] = useState<string | null>(null);
  const [pickedRow, setPickedRow] = useState<PagedRecord | null>(null);
  // Typed values not yet applied to the case, by section, row and column.
  const [pending, setPending] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<CellRef | null>(null);
  // The editable cell under a double-click's first click (null when it hit
  // none). Picking a record can open the inspector and narrow the columns
  // between the two clicks, so the second may land on another cell.
  const firstClick = useRef<{ cell: CellRef | null; at: number } | null>(null);
  const [applying, setApplying] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);
  const selected = linked || selection?.from === "search" ? (selection?.element ?? null) : picked;

  const [rootRef, width] = useWidth<HTMLDivElement>(1200);
  const showList = width >= LIST_MIN;
  const dockInspector = width >= INSPECTOR_MIN;

  const readable = !!summary && (summary.power.available || summary.diffable);

  // A new case starts over. The same case with edits applied is read again,
  // keeping the table, search and filters in view.
  useEffect(() => {
    setBook(null);
    setTables({});
    setError(null);
    setPickedRow(null);
    setEditing(null);
    if (caseVersion === editedVersion) return;
    setSection(null);
    setSearch("");
    setRulesBy({});
    setPicked(null);
    setPending({});
    setEditError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  // Each table's columns and filter fields, read when first shown. Its
  // rows are read a page at a time, as they scroll into view.
  const table = section ? tables[section] : undefined;
  useEffect(() => {
    if (!active || !section || table) return;
    const version = caseVersion;
    Promise.all([api.recordPage(section, { limit: 0 }), api.recordFields(section)])
      .then(([t, types]) => {
        if (version !== useWorkspace.getState().caseVersion) return;
        const fields = t.columns.map((c, i) => ({ key: String(i), label: c.name, type: types[i] ?? "text" }));
        const info = { id: t.id, label: t.label, columns: t.columns, skipped: t.skipped, note: t.note, total: t.page.total, fields };
        setTables((all) => ({ ...all, [t.id]: info }));
      })
      .catch((e: Error) => version === useWorkspace.getState().caseVersion && setError(e.message));
  }, [active, section, table, caseVersion]);

  // Follow a selection made elsewhere to the table that lists it.
  useEffect(() => {
    if (!book || !selection || selection.from === "records") return;
    const next = sectionFor(book, selection.element);
    if (next && next !== section) {
      setSection(next);
      setSearch("");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selection, book]);

  const raw = book?.source === "raw";
  // Rows read so far, by position, for the edits made on them.
  const loaded = useRef(new Map<number, PagedRecord>());
  const editState: EditState = {
    editable: !!book?.editable,
    section: section ?? "",
    rowIndex: (r) => r.position,
    pending,
    editing,
    start: (cell) => {
      const first = firstClick.current;
      firstClick.current = null;
      if (first && performance.now() - first.at < DOUBLE_CLICK_MS) {
        if (!first.cell) return;
        cell = first.cell;
      }
      setEditError(null);
      setEditing(cell);
    },
    finish: (cell, value, next) => {
      setEditing(null);
      if (value !== null && table && section) {
        const key = editKey(section, cell.row, cell.column);
        const original = shownValue(loaded.current.get(cell.row)?.cells[cell.column] ?? null);
        setPending((all) => {
          const copy = { ...all };
          // Typing the value back drops the edit.
          if (value.trim() === original.trim()) delete copy[key];
          else copy[key] = value;
          return copy;
        });
      }
      if (next && table) {
        // Tab: on to the next editable field of the record.
        const after = table.columns.findIndex((c, i) => i > cell.column && c.editable);
        if (after >= 0) setEditing({ row: cell.row, column: after });
      }
    },
  };
  const columns = useMemo(
    () => (table ? columnsFor(table, raw, editState) : []),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [table, raw, pending, editing, book?.editable]
  );
  const fields = table?.fields ?? [];
  const rules = (section && rulesBy[section]) || [];
  // The picked record stays listed even when a filter would hide it.
  // The core filters, sorts and searches; the picked record stays listed
  // even when a filter would hide it.
  const pageRules = useMemo(
    () => rules.map((r) => ({ field: r.field, operator: r.operator, value: r.value, upper: r.upper ?? null })),
    [rules]
  );
  const source = useMemo<RowSource<PagedRecord> | undefined>(() => {
    if (!section || !table) return undefined;
    const keep = pageRules.length ? selected : null;
    return {
      key: `${caseVersion}\u0000${editedVersion}\u0000${section}\u0000${JSON.stringify(pageRules)}\u0000${keep}`,
      fetch: async (r) => {
        const { page } = await api.recordPage(section, { ...r, rules: pageRules, keep });
        const rows = page.rows.map((row, i) => ({ ...row, position: page.positions[i] }));
        for (const row of rows) loaded.current.set(row.position, row);
        return { ...page, rows };
      },
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [section, table, caseVersion, editedVersion, pageRules, pageRules.length ? selected : null]);
  // How many pass the filters, for the note under them.
  const [matched, setMatched] = useState<number | null>(null);
  useEffect(() => {
    setMatched(null);
    if (!source || !pageRules.length) return;
    let live = true;
    source
      .fetch({ offset: 0, limit: 0, sort: null, search: "", find: null })
      .then((p) => live && setMatched(p.matched))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [source, pageRules]);
  useEffect(() => {
    loaded.current = new Map();
  }, [section, caseVersion, editedVersion]);

  const pendingList: RecordEdit[] = Object.entries(pending).map(([key, value]) => {
    const [sec, row, column] = key.split("\u0000");
    return { section: sec, row: Number(row), column: Number(column), value };
  });
  const apply = async () => {
    setApplying(true);
    setEditError(null);
    try {
      await applyRecordEdits(pendingList);
      setPending({});
    } catch (e) {
      setEditError((e as Error).message);
    } finally {
      setApplying(false);
    }
  };
  const unsaved = summary?.record_edits ?? 0;

  const pick = (row: PagedRecord) => {
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
          <Button variant="primary" onClick={() => void openFile()}>
            <FolderOpen size={14} /> Open case…
          </Button>
        }
        footer={
          <>
            <RecentFiles />
            <SampleCases />
          </>
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
    if (!table || !source) return;
    const rows = await allRows(source, null, search.trim());
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
            {book.editable ? " · double-click a cell to edit" : " · read only"}
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
          options={(key) => api.recordValues(table.id, key)}
          rules={rules}
          onRules={(next) => setRulesBy((all) => ({ ...all, [section]: next }))}
          note={
            filtered ? `${matched === null ? "…" : count(matched)} of ${count(table.total)} ${table.label.toLowerCase()}` : "Filter on any field"
          }
        />
        {(pendingList.length > 0 || unsaved > 0 || editError) && (
          <div
            className="flex flex-wrap items-center gap-2 px-3 py-1.5 border-b border-amber-200 bg-amber-50 text-xs text-amber-950"
            data-testid="records-edits"
            role="status"
          >
            {pendingList.length > 0 ? (
              <>
                <span className="font-medium">
                  {count(pendingList.length)} change{pendingList.length === 1 ? "" : "s"} not applied
                </span>
                <span className="text-amber-800/80 hidden @3xl:inline">Apply writes them into the case; every module then uses the new values.</span>
                <span className="ml-auto flex items-center gap-1">
                  <Button variant="primary" onClick={apply} disabled={applying}>
                    <Check size={13} /> {applying ? "Applying…" : "Apply"}
                  </Button>
                  <Button variant="ghost" onClick={() => setPending({})} disabled={applying}>
                    <X size={13} /> Discard
                  </Button>
                </span>
              </>
            ) : unsaved > 0 ? (
              <>
                <span className="font-medium">
                  {count(unsaved)} edited value{unsaved === 1 ? "" : "s"} not saved to a file
                </span>
                <span className="text-amber-800/80 hidden @3xl:inline">{summary.name} on disk is unchanged.</span>
                <span className="ml-auto flex items-center gap-1">
                  <Button variant="primary" onClick={() => void saveCaseAs().catch((e: Error) => setEditError(e.message))}>
                    <Save size={13} /> Save as…
                  </Button>
                  <Button
                    variant="ghost"
                    title={`Open ${summary.name} again as it is on disk`}
                    onClick={() => void openCase(summary.path)}
                  >
                    <Undo2 size={13} /> Revert to file
                  </Button>
                </span>
              </>
            ) : null}
            {editError && (
              <span className="basis-full text-red-700" role="alert">
                {editError}
              </span>
            )}
          </div>
        )}
        {(book.note || table.note) && (
          <div className="px-3 pt-2 flex flex-col gap-1.5">
            {book.note && <Notice>{book.note}</Notice>}
            {table.note && <Notice>{table.note}</Notice>}
          </div>
        )}
        <div
          className="flex-1 min-h-0"
          data-testid="records-table"
          onClickCapture={(e) => {
            if (e.detail !== 1) return;
            const at = (e.target as HTMLElement).closest<HTMLElement>("[data-cell]")?.dataset.cell;
            const [row, column] = at ? at.split(":").map(Number) : [];
            firstClick.current = { cell: at ? { row, column } : null, at: performance.now() };
          }}
        >
          <DataTable
            key={section}
            source={source}
            find={selected}
            columns={columns}
            rowKey={(r) => r.position}
            search={search}
            onRowClick={pick}
            isSelected={(r) => (selected !== null && r.element === selected) || r.position === pickedRow?.position}
            rowClassName={(r) => (r.element ? "" : "text-slate-600")}
            empty={table.total ? "No records match." : `The case has no ${table.label.toLowerCase()}.`}
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
