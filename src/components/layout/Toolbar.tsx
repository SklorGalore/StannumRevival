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
import { StyleSwitch } from "../../modules/diagram/network/StyleSwitch";
import { OverflowMenu, type MenuItem } from "../../ui/OverflowMenu";
import { LayersButton } from "../../modules/diagram/layers/LayersPanel";
import { TablesButton } from "../../modules/diagram/tables/TablesDrawer";
import { useWidth } from "../../ui/useWidth";

interface ToolbarButtonProps {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  active?: boolean;
  danger?: boolean;
  disabled?: boolean;
  title?: string;
  /** Show the text beside the icon (the toolbar hides it when narrow). */
  showLabel?: boolean;
}

function ToolbarButton({
  icon,
  label,
  onClick,
  active,
  danger,
  disabled,
  title,
  showLabel = true,
}: ToolbarButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      aria-label={label}
      className={`shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium whitespace-nowrap
                  transition-colors select-none
                  ${danger
          ? "text-red-600 hover:bg-red-50"
          : active
            ? "bg-teal-50 text-teal-800 ring-1 ring-inset ring-teal-200"
            : "text-slate-600 hover:bg-slate-100"
        }
                  ${disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}`}
    >
      {icon}
      {showLabel && <span>{label}</span>}
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
      className={`shrink-0 p-1.5 rounded text-slate-600 transition-colors ${disabled ? "opacity-30 cursor-not-allowed" : "hover:bg-slate-100 cursor-pointer"}`}
    >
      {icon}
    </button>
  );
}

function Divider() {
  return <div className="shrink-0 w-px h-5 bg-slate-200 mx-1" />;
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
  const currentFilePath = useDiagramStore((s) => s.currentFilePath);
  const gridVisible = useDiagramStore((s) => s.gridVisible);
  const snapToGrid = useDiagramStore((s) => s.snapToGrid);
  const selectedNodeIds = useDiagramStore((s) => s.selectedNodeIds);
  const selectedEdgeIds = useDiagramStore((s) => s.selectedEdgeIds);

  const newDiagram = useDiagramStore((s) => s.newDiagram);
  const deleteSelected = useDiagramStore((s) => s.deleteSelected);
  const toggleGrid = useDiagramStore((s) => s.toggleGrid);
  const toggleSnap = useDiagramStore((s) => s.toggleSnap);
  // A locked drawing can't be changed, undo and redo included.
  const locked = useDiagramStore((s) => s.locked);
  const canUndo = useDiagramStore((s) => s.past.length > 0) && !locked;
  const canRedo = useDiagramStore((s) => s.future.length > 0) && !locked;
  const undo = useDiagramStore((s) => s.undo);
  const redo = useDiagramStore((s) => s.redo);
  const mono = useDiagramStore((s) => s.colorMode === "mono");
  const toggleColorMode = useDiagramStore((s) => s.toggleColorMode);

  const hasSelection = selectedNodeIds.length > 0 || selectedEdgeIds.length > 0;
  const [ref, width] = useWidth<HTMLDivElement>(1440);

  async function handleNew() {
    if (isDirty) {
      const ok = window.confirm(
        "You have unsaved changes. Create a new diagram anyway?"
      );
      if (!ok) return;
    }
    newDiagram();
  }

  // The toolbar follows its pane's width, not the window's: side by side,
  // labels go first, then the file and view buttons move into "More".
  const labels = width >= 900;
  const roomy = width >= 760;
  const fileItems: MenuItem[] = [
    { id: "new", label: "New", icon: <FilePlus size={14} />, onClick: handleNew },
    { id: "open", label: "Open…", icon: <FolderOpen size={14} />, onClick: openDiagram },
    { id: "save", label: "Save", icon: <Save size={14} />, onClick: saveDiagram },
    { id: "saveas", label: "Save as…", icon: <Save size={14} />, onClick: saveAsDialog },
    { id: "export", label: "Export…", icon: <Download size={14} />, onClick: () => setExportOpen(true) },
  ];
  const viewItems: MenuItem[] = [
    { id: "grid", label: "Grid", icon: <Grid size={14} />, onClick: toggleGrid, active: gridVisible },
    { id: "snap", label: "Snap", icon: <Magnet size={14} />, onClick: toggleSnap, active: snapToGrid },
    { id: "print", label: "Print style", icon: <Printer size={14} />, onClick: toggleColorMode, active: mono, title: "Black and white, for printing and export" },
  ];

  return (
    <div
      ref={ref}
      className="h-12 flex items-center px-3 gap-1 border-b border-slate-200 bg-white select-none overflow-hidden whitespace-nowrap shrink-0"
      data-testid="diagram-toolbar"
    >
      {width >= 820 && (
        <span className="shrink-0 mr-3 flex items-baseline gap-1.5" data-testid="diagram-name">
          <span className="font-semibold text-slate-700 text-sm">Diagram</span>
          <span className="max-w-40 truncate text-[11px] text-slate-500" title={currentFilePath ?? "Not saved yet"}>
            {currentFilePath ? currentFilePath.split(/[\\/]/).pop() : "Untitled"}
            {isDirty && " *"}
          </span>
        </span>
      )}
      <StyleSwitch compact={!roomy} />
      <LayersButton showLabel={labels} />
      <TablesButton showLabel={labels} />

      <IconButton icon={<Undo2 size={14} />} title="Undo (Ctrl+Z)" onClick={undo} disabled={!canUndo} />
      <IconButton icon={<Redo2 size={14} />} title="Redo (Ctrl+Shift+Z)" onClick={redo} disabled={!canRedo} />

      <Divider />

      <ToolbarButton
        icon={<Wand2 size={14} />}
        label="From case"
        showLabel={width >= 520}
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
            showLabel={width >= 520}
            onClick={toggleResults}
            active={showResults}
            title="Show power flow results on the diagram"
          />
        ) : (
          <ToolbarButton
            icon={<Play size={14} />}
            label={solving ? "Solving…" : "Solve"}
            showLabel={width >= 520}
            onClick={solve}
            disabled={solving}
            title="Run AC power flow and show results on the diagram"
          />
        ))}

      {caseName && !linked && (
        <span className="min-w-0 truncate text-[11px] text-amber-700" title={`Drawn from ${caseName}`}>
          Drawn from {caseName}; open it to see results
        </span>
      )}
      {/* File and view actions sit in two menus; Delete appears with a selection. */}
      <div className="ml-auto flex items-center gap-1">
        {hasSelection && !locked && (
          <ToolbarButton icon={<Trash2 size={14} />} label="Delete" showLabel={false} onClick={deleteSelected} danger title="Delete from the drawing (Del)" />
        )}
        {roomy ? (
          <>
            <OverflowMenu label="Drawing" showLabel items={fileItems} />
            <OverflowMenu label="Canvas" showLabel items={viewItems} />
          </>
        ) : (
          <OverflowMenu label="More diagram actions" items={[...fileItems, ...viewItems]} />
        )}
      </div>

      {exportOpen && <ExportDialog onClose={() => setExportOpen(false)} />}
      {generateOpen && <GenerateDialog onClose={() => setGenerateOpen(false)} />}
    </div>
  );
}
