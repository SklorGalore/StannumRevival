import type { SourceRecord } from "../../../lib/analysis/types";

/** The record an element was read from: its RAW fields and line. */
export function SourceFields({ source }: { source: SourceRecord }) {
  return (
    <details className="mt-2 text-[11px]">
      <summary className="cursor-pointer font-medium text-slate-600">
        {source.line > 0 ? `RAW fields · line ${source.line}` : "Model fields"}
      </summary>
      <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
        {Object.entries(source.fields).map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-slate-500">{k}</dt>
            <dd className="font-mono text-slate-800 truncate">{v || "—"}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}
