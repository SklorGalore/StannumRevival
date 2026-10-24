import { useMemo, useState, type ReactNode } from "react";
import { ChevronDown, ChevronUp, ChevronLeft, ChevronRight } from "lucide-react";

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
  pageSize?: number;
  onRowClick?: (row: T) => void;
  selectedKey?: string | number | null;
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
  pageSize = 100,
  onRowClick,
  selectedKey,
  rowClassName,
  empty = "No rows.",
  initialSort = null,
}: DataTableProps<T>) {
  const [sort, setSort] = useState(initialSort);
  const [page, setPage] = useState(0);

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

  const pages = Math.max(1, Math.ceil(sorted.length / pageSize));
  const current = Math.min(page, pages - 1);
  const visible = sorted.slice(current * pageSize, (current + 1) * pageSize);

  function toggleSort(key: string) {
    setPage(0);
    setSort((s) =>
      !s || s.key !== key ? { key, dir: "asc" } : s.dir === "asc" ? { key, dir: "desc" } : null
    );
  }

  return (
    <div className="flex flex-col min-h-0 h-full">
      <div className="flex-1 min-h-0 overflow-auto">
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
            {visible.map((row) => {
              const key = rowKey(row);
              const selected = selectedKey !== undefined && selectedKey === key;
              return (
                <tr
                  key={key}
                  onClick={onRowClick ? () => onRowClick(row) : undefined}
                  aria-selected={selected}
                  className={`${onRowClick ? "cursor-pointer" : ""} ${
                    selected ? "bg-teal-50 outline outline-1 -outline-offset-1 outline-teal-400" : "hover:bg-slate-50"
                  } ${rowClassName?.(row) ?? ""}`}
                >
                  {columns.map((c) => (
                    <td
                      key={c.key}
                      className={`px-2.5 py-1.5 border-b border-slate-100 whitespace-nowrap ${
                        c.align === "right" ? "text-right" : "text-left"
                      }`}
                    >
                      {c.render ? c.render(row) : String(c.value(row) ?? "")}
                    </td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
        {!visible.length && <div className="p-6 text-center text-xs text-slate-500">{empty}</div>}
      </div>
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-slate-200 text-[11px] text-slate-500 bg-white">
        <span>
          {sorted.length.toLocaleString()} row{sorted.length === 1 ? "" : "s"}
          {sorted.length !== rows.length && ` of ${rows.length.toLocaleString()}`}
        </span>
        {pages > 1 && (
          <span className="flex items-center gap-1">
            <button
              className="p-1 rounded hover:bg-slate-100 disabled:opacity-30"
              disabled={current === 0}
              onClick={() => setPage(current - 1)}
              aria-label="Previous page"
            >
              <ChevronLeft size={14} />
            </button>
            Page {current + 1} of {pages}
            <button
              className="p-1 rounded hover:bg-slate-100 disabled:opacity-30"
              disabled={current >= pages - 1}
              onClick={() => setPage(current + 1)}
              aria-label="Next page"
            >
              <ChevronRight size={14} />
            </button>
          </span>
        )}
      </div>
    </div>
  );
}
