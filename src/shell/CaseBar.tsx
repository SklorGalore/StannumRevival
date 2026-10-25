import { FileText, FolderOpen, Search, X } from "lucide-react";
import { useWorkspace } from "../store/workspaceStore";
import { Button } from "../ui/primitives";
import { count } from "../lib/analysis/format";
import { SelectionChip } from "./SelectionChip";

const SHORTCUT = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘K" : "Ctrl K";

/** The shared open case, shown above every analysis module. */
export function CaseBar() {
  const summary = useWorkspace((s) => s.caseSummary);
  const opening = useWorkspace((s) => s.opening);
  const openError = useWorkspace((s) => s.openError);
  const openCase = useWorkspace((s) => s.openCase);
  const closeCase = useWorkspace((s) => s.closeCase);
  const setPaletteOpen = useWorkspace((s) => s.setPaletteOpen);

  const counts = summary?.power.info?.counts;
  const topo = summary?.topology.info;

  return (
    <div className="h-12 shrink-0 flex items-center gap-3 px-4 border-b border-slate-200 bg-white">
      <FileText size={16} className="text-slate-400 shrink-0" />
      {summary ? (
        <div className="min-w-0 flex items-baseline gap-2">
          <span className="text-sm font-semibold text-slate-800 truncate" title={summary.path}>
            {summary.name}
          </span>
          <span className="text-[11px] text-slate-500 truncate">
            {summary.format === "matpower" ? "MATPOWER" : `PSS/E RAW${topo ? ` v${topo.revision}` : ""}`}
            {counts &&
              ` · ${count(counts.buses)} buses · ${count(counts.generators)} generators · ${count(
                counts.branches
              )} branches · ${counts.base_mva} MVA base`}
            {!counts && topo && ` · ${count(topo.buses)} buses · ${count(topo.equipment)} equipment`}
          </span>
        </div>
      ) : (
        <span className="text-sm text-slate-500">
          {opening ? "Opening case…" : "No case open. Open a PSS/E RAW or MATPOWER file, or drop one on the window."}
        </span>
      )}
      <SelectionChip />
      {openError && (
        <span role="alert" className="text-[11px] text-red-700 truncate" title={openError}>
          {openError}
        </span>
      )}
      <div className="ml-auto flex items-center gap-1.5 shrink-0">
        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          className="inline-flex items-center gap-2 rounded-md border border-slate-300 px-2.5 py-1.5 text-xs text-slate-500 hover:bg-slate-50 hover:text-slate-700"
          title="Find a bus, branch or generator, or run a command"
        >
          <Search size={13} />
          <span>{summary ? "Find in case" : "Commands"}</span>
          <kbd className="rounded border border-slate-200 px-1 text-[10px] text-slate-400">{SHORTCUT}</kbd>
        </button>
        <Button variant="primary" onClick={() => openCase()} disabled={opening}>
          <FolderOpen size={14} /> {summary ? "Open another…" : "Open case…"}
        </Button>
        {summary && (
          <Button variant="ghost" onClick={() => closeCase()} title="Close case" aria-label="Close case">
            <X size={14} />
          </Button>
        )}
      </div>
    </div>
  );
}
