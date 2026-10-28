import { useState } from "react";
import { api } from "../../../lib/analysis/api";
import type { SourceRecord, SourceRef } from "../../../lib/analysis/types";

/**
 * The record an element was read from: its RAW fields and line. The fields
 * are fetched when the section is first opened, since the topology snapshot
 * leaves them out to stay small on large cases.
 */
export function SourceFields({ element, source }: { element: string; source: SourceRef }) {
  const [record, setRecord] = useState<{ element: string; fields: SourceRecord | null } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const shown = record?.element === element ? record : null;
  function load(open: boolean) {
    if (!open || shown) return;
    setError(null);
    api.topologySource(element).then(
      (fields) => setRecord({ element, fields }),
      (e) => setError((e as Error).message)
    );
  }
  return (
    <details key={element} className="mt-2 text-[11px]" onToggle={(e) => load((e.currentTarget as HTMLDetailsElement).open)}>
      <summary className="cursor-pointer font-medium text-slate-600">
        {source.line > 0 ? `RAW fields · line ${source.line}` : "Model fields"}
      </summary>
      {error ? (
        <p className="mt-1 text-red-700">{error}</p>
      ) : !shown ? (
        <p className="mt-1 text-slate-500">Reading…</p>
      ) : (
        <dl data-testid="source-fields" className="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
          {Object.entries(shown.fields?.fields ?? {}).map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-slate-500">{k}</dt>
              <dd className="font-mono text-slate-800 truncate">{v || "—"}</dd>
            </div>
          ))}
        </dl>
      )}
    </details>
  );
}
