import { Activity, ChevronDown, X } from "lucide-react";
import { latestResultName, useWorkspace } from "../store/workspaceStore";
import { useContingencies } from "../store/contingencyStore";
import { useStudy } from "../store/studyStore";
import { elementLabel } from "../lib/elements";
import { OverflowMenu, type MenuItem } from "../ui/OverflowMenu";

/**
 * Which result every view shows: the latest solve, or an outage drawn on the
 * diagram. Pick another from its menu; the cross goes back to the latest solve.
 */
export function ResultChip() {
  const latest = useWorkspace((s) => s.solveReport);
  const shown = useWorkspace((s) => s.shown);
  const showResult = useWorkspace((s) => s.showResult);
  const showLatest = useWorkspace((s) => s.showLatest);
  const edits = useStudy((s) => s.entries.length);
  const detail = useContingencies((s) => s.detail);
  const open = useContingencies((s) => s.open);
  if (!latest) return null;

  const latestName = latestResultName(edits);
  const latestLabel = latestName === "study" ? "Study solve" : "Base case";
  // The outage open in Contingencies, or the last trip, can be shown too.
  const outage =
    detail &&
    (open === null
      ? { name: `trip ${detail.row.element}`, label: "Tripped", element: detail.row.element }
      : { name: `contingency ${detail.row.rank}`, label: `Outage #${detail.row.rank}`, element: detail.row.element });
  const items: MenuItem[] = [
    { id: latestName, label: `${latestLabel} (latest solve)`, active: !shown, onClick: showLatest },
    ...(outage
      ? [
          {
            id: outage.name,
            label: `${outage.label}: ${elementLabel(outage.element)} out`,
            active: shown?.name === outage.name,
            onClick: () => void showResult(outage),
          },
        ]
      : []),
  ];
  const pending = shown && !shown.report && !shown.error;
  return (
    <div
      className={`shrink-0 flex items-center gap-0.5 rounded-full border pl-2 pr-0.5 text-[11px] ${
        shown ? "border-amber-300 bg-amber-50 text-amber-900" : "border-slate-300 text-slate-600"
      }`}
      data-testid="result-chip"
      data-result={shown?.name ?? latestName}
      title="The result the diagram, tables and inspector show"
    >
      <Activity size={12} className={shown ? "text-amber-700" : "text-slate-500"} />
      <span className="truncate max-w-[16rem] pl-1">
        {shown ? (
          <>
            <span className="font-semibold">{shown.label}</span> · {elementLabel(shown.element)} out
            {pending ? " · solving…" : shown.error ? " · failed" : ""}
          </>
        ) : (
          latestLabel
        )}
      </span>
      {items.length > 1 ? (
        <OverflowMenu label="Choose the result to show" icon={<ChevronDown size={12} />} className="!px-1 !py-0.5 rounded-full !text-inherit hover:!bg-black/5" items={items} />
      ) : (
        <span className="w-1" />
      )}
      {shown && (
        <button
          type="button"
          className="rounded-full p-0.5 text-amber-800 hover:bg-amber-100"
          onClick={showLatest}
          aria-label="Back to the latest solve"
          title="Back to the latest solve"
        >
          <X size={12} />
        </button>
      )}
    </div>
  );
}
