import { ReactFlowProvider } from "@xyflow/react";
import { Toolbar } from "../../components/layout/Toolbar";
import { StatusBar } from "../../components/layout/StatusBar";
import { ComponentPalette } from "../../components/palette/ComponentPalette";
import { DiagramCanvas } from "../../components/canvas/DiagramCanvas";
import { PropertiesPanel } from "../../components/properties/PropertiesPanel";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import { useEffect, useState } from "react";
import { PanelLeftClose, PanelLeftOpen, X } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { useDiagramStore } from "../../store/diagramStore";
import { elementLabel } from "../../lib/elements";
import { caseNetwork } from "../../lib/diagram/network";
import { generateDiagram } from "../../lib/diagram/generate";
import { centreBus, DrawAroundButton } from "../../ui/DrawAround";
import { useSharedSelection } from "./useSharedSelection";
import { DrawPrompt } from "./DrawPrompt";
import { useDarkCanvas } from "../../lib/diagram/ink";

/** Shown when the shared selection has no symbol in this drawing. */
function NotDrawn({ element }: { element: string }) {
  const [hidden, setHidden] = useState<string | null>(null);
  if (hidden === element) return null;
  return (
    <div
      className="absolute left-1/2 top-3 -translate-x-1/2 z-10 flex items-center gap-2 rounded-md border border-slate-200 bg-white/95 px-3 py-1.5 text-xs text-slate-700 shadow-sm whitespace-nowrap"
      data-testid="not-drawn"
    >
      <span>
        <strong>{elementLabel(element)}</strong> isn't in this drawing.
      </span>
      <DrawAroundButton element={element} from="diagram" label={`Draw around bus ${centreBus(element)}`} />
      <button className="text-slate-400 hover:text-slate-700" onClick={() => setHidden(element)} aria-label="Dismiss">
        <X size={12} />
      </button>
    </div>
  );
}

/**
 * Carry out "Draw around this" requests from other modules: replace the
 * drawing (undoably) with the element's neighbourhood, keeping the hop count
 * of the current local drawing.
 */
function useDrawRequest(): { busy: boolean; error: string | null; dismiss: () => void } {
  const request = useWorkspace((s) => s.drawRequest);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!request) return;
    const summary = useWorkspace.getState().caseSummary;
    const bus = centreBus(request.element);
    if (!summary?.power.available || bus === null) return;
    let live = true;
    setBusy(true);
    setError(null);
    const depth = useDiagramStore.getState().metadata.around?.depth ?? 2;
    caseNetwork()
      .then((net) => {
        if (!net.buses.some((b) => b.id === bus && !b.star)) throw new Error(`Bus ${bus} isn't in the case network.`);
        // A branch is drawn from both its ends, and the element is marked.
        return generateDiagram(net, { kind: "around", bus, depth, element: request.element });
      })
      .then((out) => {
        if (live)
          useDiagramStore.getState().replaceWithGenerated(out.nodes, out.edges, summary.name, { bus, depth, element: request.element });
      })
      .catch((e: Error) => live && setError(`Couldn't draw around ${elementLabel(request.element)}: ${e.message}`))
      .finally(() => live && setBusy(false));
    return () => {
      live = false;
    };
  }, [request]);
  return { busy, error, dismiss: () => setError(null) };
}

function Inner({ active }: { active: boolean }) {
  // Shortcuts (Delete, Ctrl+S) belong to the diagram only while it is visible.
  useKeyboardShortcuts(active);
  const missing = useSharedSelection();
  const drawing = useDrawRequest();
  // An empty canvas offers to draw instead (DrawPrompt).
  const blank = useDiagramStore((s) => s.nodes.length === 0);
  // Print style and exports are black on white whatever the app theme.
  const dark = useDarkCanvas();
  // A drawing generated from the open case is studied, not built by hand:
  // keep the components palette folded away unless asked for.
  const openName = useWorkspace((s) => s.caseSummary?.name);
  const generated = useDiagramStore((s) => !!s.metadata.caseName && s.metadata.caseName === openName);
  const [paletteChoice, setPaletteChoice] = useState<{ generated: boolean; open: boolean } | null>(null);
  const paletteOpen = paletteChoice && paletteChoice.generated === generated ? paletteChoice.open : !generated;
  const setPalette = (open: boolean) => setPaletteChoice({ generated, open });
  // The inspector only takes room while there is one thing to edit: a single
  // symbol, or a single wire.
  const hasSelection = useDiagramStore(
    (s) => s.selectedNodeIds.length === 1 || (s.selectedNodeIds.length === 0 && s.selectedEdgeIds.length === 1)
  );
  return (
    <div className="h-full flex flex-col bg-slate-50 overflow-hidden">
      <Toolbar />
      <div className="flex flex-1 min-h-0">
        {paletteOpen ? (
          <aside className="relative w-56 shrink-0 border-r border-slate-200 bg-white overflow-y-auto" data-testid="components-palette">
            <button
              className="absolute right-1.5 top-1.5 z-10 p-1 rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700"
              onClick={() => setPalette(false)}
              title="Hide components"
              aria-label="Hide components"
            >
              <PanelLeftClose size={15} />
            </button>
            <ComponentPalette />
          </aside>
        ) : (
          <aside className="w-10 shrink-0 border-r border-slate-200 bg-white flex flex-col items-center py-2">
            <button
              className="p-1.5 rounded text-slate-500 hover:bg-slate-100 hover:text-slate-800"
              onClick={() => setPalette(true)}
              title="Show components to draw by hand"
              aria-label="Show components"
            >
              <PanelLeftOpen size={16} />
            </button>
          </aside>
        )}
        <main className={`flex-1 min-w-0 relative ${dark ? "" : "force-light"}`}>
          <DiagramCanvas />
          {!drawing.busy && <DrawPrompt />}
          {missing && !drawing.busy && !blank && <NotDrawn element={missing} />}
          {(drawing.busy || drawing.error) && (
            <div
              className={`absolute left-1/2 top-3 -translate-x-1/2 z-10 flex items-center gap-2 rounded-md border px-3 py-1.5 text-xs shadow-sm whitespace-nowrap ${
                drawing.error ? "border-red-200 bg-red-50 text-red-800" : "border-slate-200 bg-white/95 text-slate-700"
              }`}
              data-testid="draw-request"
            >
              {drawing.error ?? "Drawing…"}
              {drawing.error && (
                <button className="opacity-60 hover:opacity-100" onClick={drawing.dismiss} aria-label="Dismiss">
                  <X size={12} />
                </button>
              )}
            </div>
          )}
        </main>
        {hasSelection && (
          <aside className="w-72 shrink-0 border-l border-slate-200 bg-white overflow-y-auto" data-testid="inspector">
            <PropertiesPanel />
          </aside>
        )}
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
