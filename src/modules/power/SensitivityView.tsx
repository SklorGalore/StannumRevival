import { useEffect, useMemo, useState } from "react";
import { api } from "../../lib/analysis/api";
import type { ResponseTable, SensitivitySummary } from "../../lib/analysis/types";
import { count, fixed, sig } from "../../lib/analysis/format";
import { ErrorBanner, Field, SearchBox, inputClass } from "../../ui/primitives";
import { DataTable } from "../../ui/DataTable";

type Order = "magnitude" | "value" | "input";

/** Inspect how every state and branch flow responds to one bus injection. */
export function SensitivityView({ summary }: { summary: SensitivitySummary }) {
  const ac = summary.model === "ac";
  const [busText, setBusText] = useState(() => String(summary.injections[0]?.bus ?? ""));
  const [kind, setKind] = useState<"p" | "q">("p");
  const [quantity, setQuantity] = useState(summary.quantities[0]?.key ?? "angle");
  const [engineering, setEngineering] = useState(true);
  const [order, setOrder] = useState<Order>("magnitude");
  const [filter, setFilter] = useState("");
  const [table, setTable] = useState<ResponseTable | null>(null);
  const [error, setError] = useState<string | null>(null);

  const busNumber = Number(busText.split(/\s/)[0]);
  const column = summary.injections.find((c) => c.bus === busNumber && c.kind === kind);
  const busesWithInjection = useMemo(() => {
    const seen = new Map<number, string>();
    for (const c of summary.injections) if (!seen.has(c.bus)) seen.set(c.bus, c.bus_name);
    return [...seen];
  }, [summary]);

  useEffect(() => {
    if (!column) {
      setTable(null);
      return;
    }
    let cancelled = false;
    setError(null);
    api
      .responses(column.column, quantity, engineering)
      .then((t) => !cancelled && setTable(t))
      .catch((e) => !cancelled && setError((e as Error).message));
    return () => {
      cancelled = true;
    };
  }, [column?.column, quantity, engineering]);

  const rows = useMemo(() => {
    if (!table) return [];
    const r = [...table.rows];
    if (order === "magnitude") r.sort((a, b) => Math.abs(b.value) - Math.abs(a.value) || a.index - b.index);
    else if (order === "value") r.sort((a, b) => b.value - a.value || a.index - b.index);
    return r;
  }, [table, order]);

  const injectionMissing =
    busText.trim() !== "" && !column
      ? kind === "q" && !ac
        ? "DC sensitivities have active-power injections only."
        : `Bus ${busText.split(/\s/)[0]} has no ${kind === "p" ? "active" : "reactive"} injection column. ${summary.column_explanation}`
      : null;

  return (
    <div className="h-full flex flex-col">
      <div className="flex flex-wrap items-end gap-3 px-3 py-2.5 border-b border-slate-200 bg-white">
        <Field label="Injection bus">
          <input
            className={`${inputClass} w-40`}
            list="sensitivity-buses"
            value={busText}
            onChange={(e) => setBusText(e.target.value)}
            placeholder="Bus number"
          />
          <datalist id="sensitivity-buses">
            {busesWithInjection.slice(0, 2000).map(([bus, name]) => (
              <option key={bus} value={String(bus)}>
                {name}
              </option>
            ))}
          </datalist>
        </Field>
        <Field label="Injection">
          <select className={inputClass} value={kind} onChange={(e) => setKind(e.target.value as "p" | "q")}>
            <option value="p">Active (P)</option>
            <option value="q" disabled={!ac}>
              Reactive (Q)
            </option>
          </select>
        </Field>
        <Field label="Response">
          <select className={inputClass} value={quantity} onChange={(e) => setQuantity(e.target.value)}>
            {summary.quantities.map((q) => (
              <option key={q.key} value={q.key}>
                {q.title}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Units">
          <select
            className={inputClass}
            value={engineering ? "eng" : "raw"}
            onChange={(e) => setEngineering(e.target.value === "eng")}
          >
            <option value="eng">Engineering</option>
            <option value="raw">Raw (p.u., rad)</option>
          </select>
        </Field>
        <Field label="Order">
          <select className={inputClass} value={order} onChange={(e) => setOrder(e.target.value as Order)}>
            <option value="magnitude">Largest |response|</option>
            <option value="value">Signed response</option>
            <option value="input">Input order</option>
          </select>
        </Field>
        <SearchBox value={filter} onChange={setFilter} placeholder="Filter responses…" />
        <span className="ml-auto text-[11px] text-slate-500 self-center">
          {summary.status} · {count(summary.injections.length)} injection columns · {fixed(summary.elapsed_ms, 0)} ms
        </span>
      </div>
      {(error || injectionMissing) && (
        <div className="p-3 pb-0">
          <ErrorBanner>{error ?? injectionMissing}</ErrorBanner>
        </div>
      )}
      <details className="px-3 py-2 text-[11px] text-slate-600 bg-white border-b border-slate-200">
        <summary className="cursor-pointer font-medium text-slate-700">How to read these results</summary>
        <p className="mt-1">{summary.explanation}</p>
        <p className="mt-1">{summary.column_explanation}</p>
      </details>
      <div className="flex-1 min-h-0 bg-white">
        {table && (
          <DataTable
            key={`${column?.column}-${quantity}-${order}`}
            rows={rows}
            rowKey={(r) => r.index}
            search={filter}
            rowClassName={(r) => (r.offline ? "text-slate-400" : "")}
            columns={[
              { key: "label", header: table.title, value: (r) => r.label + (r.offline ? " [offline]" : "") },
              {
                key: "value",
                header: `Response (${table.units})`,
                value: (r) => r.value,
                render: (r) => <span title={String(r.value)}>{sig(r.value)}</span>,
                align: "right",
              },
            ]}
          />
        )}
      </div>
    </div>
  );
}
