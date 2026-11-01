import type { ReactNode } from "react";
import { FlaskConical } from "lucide-react";
import { useWorkspace } from "../store/workspaceStore";
import { useStudy } from "../store/studyStore";
import { ResultChip } from "./ResultChip";
import { SelectionChip } from "./SelectionChip";

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div role="group" aria-label={label} className="shrink-0 flex items-center gap-2">
      <span aria-hidden="true" className="shrink-0 text-[10px] font-semibold uppercase tracking-wide text-slate-500">
        {label}
      </span>
      {children}
    </div>
  );
}

/**
 * Under the toolbar while a case is open: the result every view shows and
 * the element selected across views. Both carry over when you switch module.
 */
export function ContextBar() {
  const summary = useWorkspace((s) => s.caseSummary);
  const solved = useWorkspace((s) => !!s.solveReport);
  const selected = useWorkspace((s) => !!s.selection);
  const linked = useWorkspace((s) => s.linked);
  const edits = useStudy((s) => s.entries.length);
  const showStudy = useStudy((s) => s.show);
  if (!summary) return null;
  return (
    <div
      role="region"
      aria-label="Shown result and selection"
      data-testid="context-bar"
      data-chip-bar
      className="h-9 shrink-0 flex items-center gap-5 px-3 border-b border-slate-200 bg-slate-50 whitespace-nowrap overflow-hidden"
    >
      <Group label="Showing">
        {solved ? (
          <ResultChip />
        ) : (
          <span className="text-[11px] text-slate-500" data-testid="result-none">
            {summary.power.available ? "Not solved yet" : "No power flow data in this case"}
          </span>
        )}
      </Group>
      <Group label="Selected">
        {!selected && (
          <span className="text-[11px] text-slate-500" data-testid="selection-none">
            {linked ? "Nothing selected" : "Each module keeps its own selection"}
          </span>
        )}
        <SelectionChip />
      </Group>
      {/* Free room: the selection chip unfolds its links when this can spare it. */}
      <div className="flex-1 min-w-0" data-bar-spare />
      {edits > 0 && (
        <button
          type="button"
          onClick={showStudy}
          className="shrink-0 inline-flex items-center gap-1 rounded-full border border-amber-300 bg-amber-50 px-2 py-0.5 text-[11px] font-medium text-amber-900 hover:bg-amber-100"
          title="Power flow is solving this case with study edits. Click to see them."
          data-testid="study-badge"
        >
          <FlaskConical size={12} /> {edits} edit{edits === 1 ? "" : "s"}
        </button>
      )}
    </div>
  );
}
