import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { usePagedRows, type RowSource } from "./usePagedRows";

/** Every body row is this tall, so only the rows in view need drawing. */
export const ROW_HEIGHT = 28;
const HEADER_HEIGHT = 33;
/** Rows drawn beyond the edges of the view, for smooth scrolling. */
const OVERSCAN = 12;

export interface Column<T> {
  key: string;
  header: ReactNode;
  /** Value used for sorting, searching and CSV export. */
  value: (row: T) => string | number | boolean | null;
  render?: (row: T) => ReactNode;
  align?: "left" | "right";
  title?: string;
  /** With a source: the core's field this column sorts by (defaults to `key`), or null when it can't sort. */
  sortField?: string | null;
}

interface DataTableProps<T> {
  /** Every row, sorted and searched here. */
  rows?: T[];
  /** Or rows read from the core a block at a time, sorted and searched there. */
  source?: RowSource<T>;
  /** With a source: the selected element, looked up so it can be scrolled into view. */
  find?: string | null;
  columns: Column<T>[];
  rowKey: (row: T) => string | number;
  search?: string;
  onRowClick?: (row: T) => void;
  selectedKey?: string | number | null;
  /** Marks rows as selected when one key is not enough (overrides selectedKey). */
  isSelected?: (row: T) => boolean;
  rowClassName?: (row: T) => string;
  empty?: ReactNode;
  /** Keep the incoming order until the user sorts. */
  initialSort?: { key: string; dir: "asc" | "desc" } | null;
}

function compare(a: unknown, b: unknown): number {
  if (a === b) return 0;
  if (a === null || a === undefined) return 1;
  if (b === null || b === undefined) return -1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b), undefined, { numeric: true });
}

/** Filter rows by a free-text query across every column value. */
export function filterRows<T>(rows: T[], columns: Column<T>[], query: string): T[] {
  const q = query.trim().toLowerCase();
  if (!q) return rows;
  return rows.filter((row) =>
    columns.some((c) => String(c.value(row) ?? "").toLowerCase().includes(q))
  );
}

export function DataTable<T>({
  rows = [],
  source,
  find = null,
  columns,
  rowKey,
  search = "",
  onRowClick,
  selectedKey,
  isSelected,
  rowClassName,
  empty = "No rows.",
  initialSort = null,
}: DataTableProps<T>) {
  const [sort, setSort] = useState(initialSort);
  const remote = !!source;

  const filtered = useMemo(() => (remote ? [] : filterRows(rows, columns, search)), [remote, rows, columns, search]);
  const sorted = useMemo(() => {
    if (remote || !sort) return filtered;
    const col = columns.find((c) => c.key === sort.key);
    if (!col) return filtered;
    const sign = sort.dir === "asc" ? 1 : -1;
    // Stable: ties keep input order.
    return filtered
      .map((row, i) => ({ row, i }))
      .sort((a, b) => {
        const x = col.value(a.row);
        const y = col.value(b.row);
        // Missing values stay last in both directions.
        const missing = x === null || x === undefined || y === null || y === undefined;
        return (missing ? compare(x, y) : sign * compare(x, y)) || a.i - b.i;
      })
      .map((x) => x.row);
  }, [remote, filtered, sort, columns]);

  const selectedRow = (row: T) =>
    isSelected ? isSelected(row) : selectedKey !== undefined && selectedKey !== null && selectedKey === rowKey(row);

  // Only the rows in view (plus a margin) are drawn; spacer rows keep the
  // scroll height, so a case with tens of thousands of rows scrolls freely.
  const scrollRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ top: 0, height: 600 });
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => setView({ top: el.scrollTop, height: el.clientHeight || 600 });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const first = Math.max(0, Math.floor(view.top / ROW_HEIGHT) - OVERSCAN);
  const wanted = Math.ceil((view.top + view.height) / ROW_HEIGHT) + OVERSCAN;

  // From the core: the search waits for a pause in typing.
  const [quietSearch, setQuietSearch] = useState(search);
  useEffect(() => {
    if (!remote) return;
    const t = setTimeout(() => setQuietSearch(search), 200);
    return () => clearTimeout(t);
  }, [remote, search]);
  const sortColumn = sort ? columns.find((c) => c.key === sort.key) : undefined;
  const remoteSort =
    sort && sortColumn && sortColumn.sortField !== null ? { field: sortColumn.sortField ?? sortColumn.key, descending: sort.dir === "desc" } : null;
  const paged = usePagedRows(source, remoteSort, quietSearch.trim(), first, wanted, find);

  const length = remote ? (paged.matched ?? 0) : sorted.length;
  const total = remote ? (paged.total ?? 0) : rows.length;
  const last = Math.min(length, wanted);
  const visible: { row: T | undefined; i: number }[] = [];
  for (let i = first; i < last; i++) visible.push({ row: remote ? paged.rowAt(i) : sorted[i], i });
  // Fewer rows than before (a search, a filter): scroll back within them.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const end = Math.max(0, length * ROW_HEIGHT + HEADER_HEIGHT - el.clientHeight);
    if (view.top > end) {
      el.scrollTop = end;
      setView({ top: end, height: el.clientHeight || 600 });
    }
  }, [length, view.top]);
  const above = first * ROW_HEIGHT;
  const below = (length - last) * ROW_HEIGHT;

  // When the selection changes (here or in another module), scroll it into
  // view. Sorting or scrolling by hand doesn't re-trigger this.
  const firstSelected = remote ? -1 : sorted.findIndex(selectedRow);
  const revealKey = remote ? (paged.found.index === null ? null : `${paged.found.n}`) : firstSelected >= 0 ? rowKey(sorted[firstSelected]) : null;
  const revealAt = remote ? (paged.found.index ?? -1) : firstSelected;
  useEffect(() => {
    const el = scrollRef.current;
    if (revealKey === null || !el || revealAt < 0) return;
    const top = revealAt * ROW_HEIGHT;
    const bodyHeight = el.clientHeight - HEADER_HEIGHT;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW_HEIGHT > el.scrollTop + bodyHeight) el.scrollTop = top + ROW_HEIGHT - bodyHeight;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revealKey]);

  function toggleSort(key: string) {
    if (remote && columns.find((c) => c.key === key)?.sortField === null) return;
    setSort((s) =>
      !s || s.key !== key ? { key, dir: "asc" } : s.dir === "asc" ? { key, dir: "desc" } : null
    );
  }

  return (
    <div className="flex flex-col min-h-0 h-full">
      <div
        ref={scrollRef}
        className="flex-1 min-h-0 overflow-auto"
        onScroll={(e) => {
          const el = e.currentTarget;
          setView({ top: el.scrollTop, height: el.clientHeight || 600 });
        }}
      >
        <table className="w-full text-xs tabular-nums border-separate border-spacing-0">
          <thead className="sticky top-0 z-10 bg-slate-50">
            <tr>
              {columns.map((c) => (
                <th
                  key={c.key}
                  scope="col"
                  title={c.title}
                  onClick={() => toggleSort(c.key)}
                  className={`px-2.5 py-2 font-semibold text-slate-600 border-b border-slate-200 whitespace-nowrap cursor-pointer select-none hover:bg-slate-100 ${
                    c.align === "right" ? "text-right" : "text-left"
                  }`}
                >
                  <span className="inline-flex items-center gap-0.5">
                    {c.header}
                    {sort?.key === c.key &&
                      (sort.dir === "asc" ? <ChevronUp size={12} /> : <ChevronDown size={12} />)}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {above > 0 && (
              <tr aria-hidden="true" data-spacer style={{ height: above }}>
                <td colSpan={columns.length} />
              </tr>
            )}
            {visible.map(({ row, i }) => {
              if (row === undefined)
                return (
                  <tr key={`loading-${i}`} aria-busy="true" style={{ height: ROW_HEIGHT }} data-loading>
                    <td colSpan={columns.length} className="px-2.5 py-0 border-b border-slate-100 text-slate-300">
                      …
                    </td>
                  </tr>
                );
              const key = rowKey(row);
              const selected = selectedRow(row);
              return (
                <tr
                  key={key}
                  onClick={onRowClick ? () => onRowClick(row) : undefined}
                  aria-selected={selected}
                  style={{ height: ROW_HEIGHT }}
                  className={`${onRowClick ? "cursor-pointer" : ""} ${
                    selected ? "bg-teal-50 outline outline-1 -outline-offset-1 outline-teal-400" : "hover:bg-slate-50"
                  } ${rowClassName?.(row) ?? ""}`}
                >
                  {columns.map((c) => (
                    <td
                      key={c.key}
                      className={`px-2.5 py-0 border-b border-slate-100 whitespace-nowrap ${
                        c.align === "right" ? "text-right" : "text-left"
                      }`}
                    >
                      {c.render ? c.render(row) : String(c.value(row) ?? "")}
                    </td>
                  ))}
                </tr>
              );
            })}
            {below > 0 && (
              <tr aria-hidden="true" data-spacer style={{ height: below }}>
                <td colSpan={columns.length} />
              </tr>
            )}
          </tbody>
        </table>
        {!visible.length && (!remote || paged.matched !== null) && (
          <div className="p-6 text-center text-xs text-slate-500">{empty}</div>
        )}
        {remote && paged.matched === null && !paged.error && <div className="p-6 text-center text-xs text-slate-500">Reading rows…</div>}
        {paged.error && <div className="p-6 text-center text-xs text-red-700" role="alert">{paged.error}</div>}
      </div>
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-slate-200 text-[11px] text-slate-500 bg-white">
        <span data-testid="table-count">
          {length.toLocaleString()} row{length === 1 ? "" : "s"}
          {length !== total && ` of ${total.toLocaleString()}`}
        </span>
      </div>
    </div>
  );
}
