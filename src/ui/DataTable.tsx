import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";

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
}

interface DataTableProps<T> {
  rows: T[];
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
  rows,
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

  const filtered = useMemo(() => filterRows(rows, columns, search), [rows, columns, search]);
  const sorted = useMemo(() => {
    if (!sort) return filtered;
    const col = columns.find((c) => c.key === sort.key);
    if (!col) return filtered;
    const sign = sort.dir === "asc" ? 1 : -1;
    // Stable: ties keep input order.
    return filtered
      .map((row, i) => ({ row, i }))
      .sort((a, b) => sign * compare(col.value(a.row), col.value(b.row)) || a.i - b.i)
      .map((x) => x.row);
  }, [filtered, sort, columns]);

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
  const last = Math.min(sorted.length, Math.ceil((view.top + view.height) / ROW_HEIGHT) + OVERSCAN);
  const visible = sorted.slice(first, last);
  const above = first * ROW_HEIGHT;
  const below = (sorted.length - last) * ROW_HEIGHT;

  // When the selection changes (here or in another module), scroll it into
  // view. Sorting or scrolling by hand doesn't re-trigger this.
  const firstSelected = sorted.findIndex(selectedRow);
  const revealKey = firstSelected >= 0 ? rowKey(sorted[firstSelected]) : null;
  useEffect(() => {
    const el = scrollRef.current;
    if (revealKey === null || !el) return;
    const top = firstSelected * ROW_HEIGHT;
    const bodyHeight = el.clientHeight - HEADER_HEIGHT;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW_HEIGHT > el.scrollTop + bodyHeight) el.scrollTop = top + ROW_HEIGHT - bodyHeight;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revealKey]);

  function toggleSort(key: string) {
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
            {visible.map((row) => {
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
        {!visible.length && <div className="p-6 text-center text-xs text-slate-500">{empty}</div>}
      </div>
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-slate-200 text-[11px] text-slate-500 bg-white">
        <span>
          {sorted.length.toLocaleString()} row{sorted.length === 1 ? "" : "s"}
          {sorted.length !== rows.length && ` of ${rows.length.toLocaleString()}`}
        </span>
      </div>
    </div>
  );
}
