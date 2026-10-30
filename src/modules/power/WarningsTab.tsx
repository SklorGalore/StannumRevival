import { useMemo, useState } from "react";
import { ChevronRight } from "lucide-react";
import type { RawWarning } from "../../lib/analysis/types";
import { DataTable } from "../../ui/DataTable";
import { ElementLink } from "../../ui/DrawAround";
import { count } from "../../lib/analysis/format";
import { groupWarnings, lineSpan } from "./warnings";

/**
 * The RAW import's warnings, one row per kind of message with how often it
 * came up. Picking a kind lists each record it applies to, with a link to
 * the element.
 */
export function WarningsTab({ warnings, onShow }: { warnings: RawWarning[]; onShow: (element: string) => void }) {
  const groups = useMemo(() => groupWarnings(warnings), [warnings]);
  const [open, setOpen] = useState<string | null>(groups.length === 1 ? groups[0].message : null);
  const group = groups.find((g) => g.message === open) ?? null;
  return (
    <div className="h-full flex flex-col bg-white" data-testid="import-warnings">
      <p className="px-3 py-2 text-xs text-slate-500 border-b border-slate-200">
        {count(warnings.length)} warning{warnings.length === 1 ? "" : "s"} of {groups.length} kind{groups.length === 1 ? "" : "s"}. Each
        is a value in the file that Stannum ignored or approximated.
      </p>
      <ul className={`overflow-y-auto divide-y divide-slate-100 ${group ? "max-h-[40%] border-b border-slate-200" : "flex-1"}`}>
        {groups.map((g) => {
          const on = g.message === open;
          return (
            <li key={g.message}>
              <button
                type="button"
                aria-expanded={on}
                onClick={() => setOpen(on ? null : g.message)}
                className={`w-full flex items-start gap-2 px-3 py-2 text-left text-xs hover:bg-slate-50 ${on ? "bg-teal-50/60" : ""}`}
              >
                <ChevronRight size={13} className={`mt-0.5 shrink-0 text-slate-500 transition-transform ${on ? "rotate-90" : ""}`} />
                <span className="shrink-0 min-w-12 rounded-full bg-amber-100 px-1.5 py-px text-center text-[11px] font-semibold tabular-nums text-amber-900">
                  {count(g.count)}
                </span>
                <span className="flex-1 min-w-0">
                  <span className="text-slate-800">{g.message}</span>
                  <span className="block text-[11px] text-slate-500">
                    {[lineSpan(g.warnings), g.elements.length > 0 && `${count(g.elements.length)} element${g.elements.length === 1 ? "" : "s"}`]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {group && (
        <div className="flex-1 min-h-0">
          <DataTable
            rows={group.warnings.map((w, i) => ({ ...w, i }))}
            rowKey={(w) => w.i}
            initialSort={null}
            columns={[
              { key: "line", header: "Line", value: (w) => w.line, render: (w) => (w.line > 0 ? w.line : "—"), align: "right" },
              {
                key: "element",
                header: "Element",
                value: (w) => w.element ?? "",
                render: (w) => (w.element ? <ElementLink element={w.element} from="power" onOpen={() => onShow(w.element!)} /> : <span className="text-slate-500">—</span>),
              },
            ]}
          />
        </div>
      )}
    </div>
  );
}
