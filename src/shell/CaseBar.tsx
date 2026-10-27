import { Columns2, FileText, FlaskConical, FolderOpen, Search, X } from "lucide-react";
import { useStudy } from "../store/studyStore";
import { SPLIT_PARTNER, useWorkspace } from "../store/workspaceStore";
import { Button } from "../ui/primitives";
import { count } from "../lib/analysis/format";
import { SelectionChip } from "./SelectionChip";
import { ResultChip } from "./ResultChip";
import { useWidth } from "../ui/useWidth";
import { WindowControls, dragRegion } from "./WindowControls";
import { StudyFileButton } from "./StudyFileControls";
import { useStudyFile } from "../store/studyFileStore";

const SHORTCUT = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘K" : "Ctrl K";

/** The shared open case, shown above every analysis module. On Windows it is also the title bar. */
export function CaseBar() {
  const summary = useWorkspace((s) => s.caseSummary);
  const opening = useWorkspace((s) => s.opening);
  const openError = useWorkspace((s) => s.openError);
  const openFile = useStudyFile((s) => s.openFile);
  const studyOpening = useStudyFile((s) => s.busy === "opening");
  const closeCase = useWorkspace((s) => s.closeCase);
  const setPaletteOpen = useWorkspace((s) => s.setPaletteOpen);
  const module = useWorkspace((s) => s.module);
  const split = useWorkspace((s) => s.split);
  const setSplit = useWorkspace((s) => s.setSplit);
  const edits = useStudy((s) => s.entries.length);
  const showStudy = useStudy((s) => s.show);

  const counts = summary?.power.info?.counts;
  const topo = summary?.topology.info;

  // In a narrow window the buttons drop their labels and the selection's
  // "Show in" links fold into a menu, rather than running into each other.
  const [ref, width] = useWidth<HTMLDivElement>(1440);
  const narrow = width < 1100;
  return (
    <div ref={ref} {...dragRegion} className="h-12 shrink-0 flex items-center gap-3 px-4 border-b border-slate-200 bg-white whitespace-nowrap">
      <FileText size={16} className="text-slate-400 shrink-0" />
      {summary ? (
        <div className="min-w-[10rem] flex-[1_1_0] flex items-baseline gap-2" data-case-summary>
          <span className="min-w-0 max-w-full shrink-0 text-sm font-semibold text-slate-800 truncate" title={summary.path}>
            {summary.name}
          </span>
          <span className="min-w-0 text-[11px] text-slate-500 truncate">
            {summary.format === "matpower" ? "MATPOWER" : `PSS/E RAW${topo && !topo.derived ? ` v${topo.revision}` : ""}`}
            {counts &&
              ` · ${count(counts.buses)} buses · ${count(counts.generators)} generators · ${count(
                counts.branches
              )} branches · ${counts.base_mva} MVA base`}
            {!counts && topo && ` · ${count(topo.buses)} buses · ${count(topo.equipment)} equipment`}
          </span>
        </div>
      ) : (
        <span className="text-sm text-slate-500">
          {studyOpening ? "Opening study…" : opening ? "Opening case…" : "No case open. Open a PSS/E RAW or MATPOWER file or a Stannum study, or drop one on the window."}
        </span>
      )}
      {summary && edits > 0 && (
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
      {summary && <ResultChip />}
      <SelectionChip />
      {openError && (
        <span role="alert" className="text-[11px] text-red-700 truncate" title={openError}>
          {openError}
        </span>
      )}
      <div className="ml-auto flex items-center gap-1.5 shrink-0">
        <StudyFileButton narrow={narrow} />
        <button
          type="button"
          onClick={() => setSplit(split ? null : SPLIT_PARTNER[module])}
          aria-pressed={!!split}
          className={`inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs ${
            split ? "border-teal-300 bg-teal-50 text-teal-800 hover:bg-teal-100" : "border-slate-300 text-slate-500 hover:bg-slate-50 hover:text-slate-700"
          }`}
          title={split ? "Back to one module at a time" : "Show two modules side by side, with linked selection"}
        >
          <Columns2 size={13} /> {!narrow && "Side by side"}
        </button>
        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          className="inline-flex items-center gap-2 rounded-md border border-slate-300 px-2.5 py-1.5 text-xs text-slate-500 hover:bg-slate-50 hover:text-slate-700"
          title="Find a bus, branch or generator, or run a command"
        >
          <Search size={13} />
          {!narrow && <span>{summary ? "Find in case" : "Commands"}</span>}
          <kbd className="rounded border border-slate-200 px-1 text-[10px] text-slate-400">{SHORTCUT}</kbd>
        </button>
        <Button
          variant="primary"
          onClick={() => void openFile()}
          disabled={opening || studyOpening}
          aria-label={summary ? "Open another…" : "Open case…"}
          title={summary ? "Open another case or study" : "Open a case or a study"}
        >
          <FolderOpen size={14} /> {(!narrow || !summary) && (summary ? "Open another…" : "Open case…")}
        </Button>
        {summary && (
          <Button variant="ghost" onClick={() => closeCase()} title="Close case" aria-label="Close case">
            <X size={14} />
          </Button>
        )}
      </div>
      <WindowControls />
    </div>
  );
}
