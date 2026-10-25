import { ReactFlowProvider } from "@xyflow/react";
import { Toolbar } from "../../components/layout/Toolbar";
import { StatusBar } from "../../components/layout/StatusBar";
import { ComponentPalette } from "../../components/palette/ComponentPalette";
import { DiagramCanvas } from "../../components/canvas/DiagramCanvas";
import { PropertiesPanel } from "../../components/properties/PropertiesPanel";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import { useState } from "react";
import { Wand2, X } from "lucide-react";
import { GenerateDialog } from "../../components/dialogs/GenerateDialog";
import { useWorkspace } from "../../store/workspaceStore";
import { elementLabel } from "../../lib/elements";
import { generateCentre, useSharedSelection } from "./useSharedSelection";
import { useDarkCanvas } from "../../lib/diagram/ink";

/** Shown when the shared selection has no symbol in this drawing. */
function NotDrawn({ element }: { element: string }) {
  const canGenerate = useWorkspace((s) => !!s.caseSummary?.power.available);
  const [open, setOpen] = useState(false);
  const [hidden, setHidden] = useState<string | null>(null);
  const bus = generateCentre(element);
  if (hidden === element) return null;
  return (
    <>
      <div
        className="absolute left-1/2 top-3 -translate-x-1/2 z-10 flex items-center gap-2 rounded-md border border-slate-200 bg-white/95 px-3 py-1.5 text-xs text-slate-700 shadow-sm whitespace-nowrap"
        data-testid="not-drawn"
      >
        <span>
          <strong>{elementLabel(element)}</strong> isn't in this drawing.
        </span>
        {canGenerate && bus !== null && (
          <button className="inline-flex items-center gap-1 font-medium text-teal-700 hover:underline" onClick={() => setOpen(true)}>
            <Wand2 size={12} /> Generate around bus {bus}
          </button>
        )}
        <button className="text-slate-400 hover:text-slate-700" onClick={() => setHidden(element)} aria-label="Dismiss">
          <X size={12} />
        </button>
      </div>
      {open && bus !== null && <GenerateDialog around={bus} onClose={() => setOpen(false)} />}
    </>
  );
}

function Inner({ active }: { active: boolean }) {
  // Shortcuts (Delete, Ctrl+S) belong to the diagram only while it is visible.
  useKeyboardShortcuts(active);
  const missing = useSharedSelection();
  // Print style and exports are black on white whatever the app theme.
  const dark = useDarkCanvas();
  return (
    <div className="h-full flex flex-col bg-slate-50 overflow-hidden">
      <Toolbar />
      <div className="flex flex-1 min-h-0">
        <aside className="w-56 shrink-0 border-r border-slate-200 bg-white overflow-y-auto">
          <ComponentPalette />
        </aside>
        <main className={`flex-1 min-w-0 relative ${dark ? "" : "force-light"}`}>
          <DiagramCanvas />
          {missing && <NotDrawn element={missing} />}
        </main>
        <aside className="w-72 shrink-0 border-l border-slate-200 bg-white overflow-y-auto">
          <PropertiesPanel />
        </aside>
      </div>
      <StatusBar />
    </div>
  );
}

/** The original StannumSLD single-line diagram editor. */
export function DiagramModule({ active }: { active: boolean }) {
  return (
    <ReactFlowProvider>
      <Inner active={active} />
    </ReactFlowProvider>
  );
}
