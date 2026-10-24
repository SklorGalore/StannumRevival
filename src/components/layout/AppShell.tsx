import { ReactFlowProvider } from "@xyflow/react";
import { Toolbar } from "./Toolbar";
import { StatusBar } from "./StatusBar";
import { ComponentPalette } from "../palette/ComponentPalette";
import { DiagramCanvas } from "../canvas/DiagramCanvas";
import { PropertiesPanel } from "../properties/PropertiesPanel";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";

function Inner() {
  useKeyboardShortcuts();

  return (
    <div className="h-screen w-screen flex flex-col bg-slate-50 overflow-hidden">
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

export function AppShell() {
  return (
    <ReactFlowProvider>
      <Inner />
    </ReactFlowProvider>
  );
}
