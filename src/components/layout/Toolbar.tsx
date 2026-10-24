import {
  FilePlus,
  FolderOpen,
  Save,
  Download,
  Grid,
  Magnet,
  Trash2,
  Wand2,
  Activity,
  Play,
  Undo2,
  Redo2,
  Printer,
} from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, DEFAULT_SOLVE_SETTINGS } from "../../lib/analysis/api";
import { GenerateDialog } from "../dialogs/GenerateDialog";
import { useDiagramStore } from "../../store/diagramStore";
import { openDiagram, saveDiagram, saveAsDialog } from "../../lib/tauri/fileOps";
import { useState } from "react";
import { ExportDialog } from "../dialogs/ExportDialog";

interface ToolbarButtonProps {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  active?: boolean;
  danger?: boolean;
  disabled?: boolean;
  title?: string;
  /** Show the label only on wide screens. */
  compact?: boolean;
}

function ToolbarButton({
  icon,
  label,
  onClick,
  active,
  danger,
  disabled,
  title,
  compact,
}: ToolbarButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      aria-label={label}
      className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium
                  transition-colors select-none
                  ${danger
          ? "text-red-600 hover:bg-red-50"
          : active
            ? "bg-blue-100 text-blue-700"
            : "text-slate-600 hover:bg-slate-100"
        }
                  ${disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}`}
    >
      {icon}
      <span className={compact ? "hidden 2xl:inline" : "hidden sm:inline"}>{label}</span>
    </button>
  );
}

/** Icon-only toolbar button. */
function IconButton({ icon, title, onClick, disabled }: { icon: React.ReactNode; title: string; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
      className={`p-1.5 rounded text-slate-600 transition-colors ${disabled ? "opacity-30 cursor-not-allowed" : "hover:bg-slate-100 cursor-pointer"}`}
    >
      {icon}
    </button>
  );
}

function Divider() {
  return <div className="w-px h-5 bg-slate-200 mx-1" />;
}

export function Toolbar() {
  const [exportOpen, setExportOpen] = useState(false);
  const [generateOpen, setGenerateOpen] = useState(false);
  const [solving, setSolving] = useState(false);

  const caseSummary = useWorkspace((s) => s.caseSummary);
  const solveReport = useWorkspace((s) => s.solveReport);
  const setSolveReport = useWorkspace((s) => s.setSolveReport);
  const caseName = useDiagramStore((s) => s.metadata.caseName);
  const showResults = useDiagramStore((s) => s.showResults);
  const toggleResults = useDiagramStore((s) => s.toggleResults);
  const canGenerate = !!caseSummary?.power.available;
  const linked = !!caseName && caseName === caseSummary?.name;

  async function solve() {
    const version = useWorkspace.getState().caseVersion;
    setSolving(true);
    try {
      const report = await api.solve(DEFAULT_SOLVE_SETTINGS);
      if (version === useWorkspace.getState().caseVersion) setSolveReport(report);
    } catch (e) {
      window.alert(`Power flow failed: ${(e as Error).message}`);
    } finally {
      setSolving(false);
    }
  }

  const isDirty = useDiagramStore((s) => s.isDirty);
  const gridVisible = useDiagramStore((s) => s.gridVisible);
  const snapToGrid = useDiagramStore((s) => s.snapToGrid);
  const selectedNodeIds = useDiagramStore((s) => s.selectedNodeIds);
  const selectedEdgeIds = useDiagramStore((s) => s.selectedEdgeIds);

  const newDiagram = useDiagramStore((s) => s.newDiagram);
  const deleteSelected = useDiagramStore((s) => s.deleteSelected);
  const toggleGrid = useDiagramStore((s) => s.toggleGrid);
  const toggleSnap = useDiagramStore((s) => s.toggleSnap);
  const canUndo = useDiagramStore((s) => s.past.length > 0);
  const canRedo = useDiagramStore((s) => s.future.length > 0);
  const undo = useDiagramStore((s) => s.undo);
  const redo = useDiagramStore((s) => s.redo);
  const mono = useDiagramStore((s) => s.colorMode === "mono");
  const toggleColorMode = useDiagramStore((s) => s.toggleColorMode);

  const hasSelection = selectedNodeIds.length > 0 || selectedEdgeIds.length > 0;

  async function handleNew() {
    if (isDirty) {
      const ok = window.confirm(
        "You have unsaved changes. Create a new diagram anyway?"
      );
      if (!ok) return;
    }
    newDiagram();
  }

  return (
    <div className="h-12 flex items-center px-3 gap-1 border-b border-slate-200 bg-white select-none overflow-x-auto">
      {/* App name */}
      <span className="font-semibold text-slate-700 text-sm mr-3">Diagram</span>

      <ToolbarButton
        icon={<FilePlus size={14} />}
        label="New"
        compact
        onClick={handleNew}
      />
      <ToolbarButton
        icon={<FolderOpen size={14} />}
        label="Open"
        compact
        onClick={openDiagram}
      />
      <ToolbarButton
        icon={<Save size={14} />}
        label="Save"
        compact
        onClick={saveDiagram}
      />
      <ToolbarButton
        icon={<Save size={14} />}
        label="Save As"
        compact
        onClick={saveAsDialog}
      />

      <Divider />

      <IconButton icon={<Undo2 size={14} />} title="Undo (Ctrl+Z)" onClick={undo} disabled={!canUndo} />
      <IconButton icon={<Redo2 size={14} />} title="Redo (Ctrl+Shift+Z)" onClick={redo} disabled={!canRedo} />

      <Divider />

      <ToolbarButton
        icon={<Download size={14} />}
        label="Export"
        compact
        onClick={() => setExportOpen(true)}
      />

      <Divider />

      <ToolbarButton
        icon={<Wand2 size={14} />}
        label="From case"
        onClick={() => setGenerateOpen(true)}
        disabled={!canGenerate}
        title={
          canGenerate
            ? "Generate a one-line from the open case"
            : caseSummary
              ? "This case can't be read for power flow, so it can't be drawn"
              : "Open a case first"
        }
      />
      {linked &&
        (solveReport ? (
          <ToolbarButton
            icon={<Activity size={14} />}
            label="Results"
            onClick={toggleResults}
            active={showResults}
            title="Show power flow results on the diagram"
          />
        ) : (
          <ToolbarButton
            icon={<Play size={14} />}
            label={solving ? "Solving…" : "Solve"}
            onClick={solve}
            disabled={solving}
            title="Run AC power flow and show results on the diagram"
          />
        ))}

      <Divider />

      <ToolbarButton
        icon={<Grid size={14} />}
        label="Grid"
        onClick={toggleGrid}
        active={gridVisible}
      />
      <ToolbarButton
        icon={<Magnet size={14} />}
        label="Snap"
        onClick={toggleSnap}
        active={snapToGrid}
      />
      <ToolbarButton
        icon={<Printer size={14} />}
        label="Print style"
        onClick={toggleColorMode}
        active={mono}
        title="Black and white, for printing and export"
      />

      <Divider />

      <ToolbarButton
        icon={<Trash2 size={14} />}
        label="Delete"
        onClick={deleteSelected}
        danger
        disabled={!hasSelection}
      />

      {exportOpen && <ExportDialog onClose={() => setExportOpen(false)} />}
      {generateOpen && <GenerateDialog onClose={() => setGenerateOpen(false)} />}
    </div>
  );
}
