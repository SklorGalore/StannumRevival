import { ReactFlowProvider } from "@xyflow/react";
import { Toolbar } from "../../components/layout/Toolbar";
import { StatusBar } from "../../components/layout/StatusBar";
import { ComponentPalette } from "../../components/palette/ComponentPalette";
import { DiagramCanvas } from "../../components/canvas/DiagramCanvas";
import { PropertiesPanel } from "../../components/properties/PropertiesPanel";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";

function Inner({ active }: { active: boolean }) {
  // Shortcuts (Delete, Ctrl+S) belong to the diagram only while it is visible.
  useKeyboardShortcuts(active);
  return (
    <div className="h-full flex flex-col bg-slate-50 overflow-hidden">
      <Toolbar />
      <div className="flex flex-1 min-h-0">
        <aside className="w-56 shrink-0 border-r border-slate-200 bg-white overflow-y-auto">
          <ComponentPalette />
        </aside>
        <main className="flex-1 min-w-0 relative">
          <DiagramCanvas />
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
