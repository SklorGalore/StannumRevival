import { ReactFlowProvider, useReactFlow } from "@xyflow/react";
import { Toolbar } from "../../components/layout/Toolbar";
import { StatusBar } from "../../components/layout/StatusBar";
import { ComponentPalette } from "../../components/palette/ComponentPalette";
import { DiagramCanvas } from "../../components/canvas/DiagramCanvas";
import { PropertiesPanel, selectedCaseElement } from "../../components/properties/PropertiesPanel";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import { useEffect, useRef, useState } from "react";
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
import { useDrawingStyle } from "../../store/drawingStyleStore";
import { NetworkCanvas, NetworkInspector, NetworkToolbar } from "./network/NetworkView";
import { MapCanvas, MapInspector, MapToolbar } from "./map/MapView";
import { inspectorClass } from "../../ui/ElementInspector";
import { useWidth } from "../../ui/useWidth";
import { useLayers } from "../../store/layerStore";
import { useLayerSync } from "./layers/useLayerSync";
import { LayerLegend, LayersPanel } from "./layers/LayersPanel";
import { TablesDrawer } from "./tables/TablesDrawer";

/**
 * Keep the style's shared state in step with the rest of the app: a new case
 * starts afresh, and selections and "Draw around" requests from other modules
 * become the Diagram's selection in either style.
 */
function useDrawingStyleSync() {
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const topology = useWorkspace((s) => !!s.caseSummary?.topology.available);
  const shared = useWorkspace((s) => s.selection);
  const request = useWorkspace((s) => s.drawRequest);
  useEffect(() => {
    useDrawingStyle.getState().reset();
  }, [caseVersion]);
  useEffect(() => {
    if (!topology && useDrawingStyle.getState().style === "network") useDrawingStyle.getState().setStyle("oneline");
  }, [topology]);
  const hadShared = useRef(false);
  useEffect(() => {
    if (shared && shared.from !== "diagram") useDrawingStyle.getState().setElement(shared.element);
    // Clearing the shared selection (Escape) clears the Diagram's too.
    else if (!shared && hadShared.current) useDrawingStyle.getState().setElement(null);
    hadShared.current = !!shared;
  }, [shared]);
  useEffect(() => {
    if (request) useDrawingStyle.getState().setElement(request.element);
  }, [request]);
}

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
      <button className="text-slate-500 hover:text-slate-700" onClick={() => setHidden(element)} aria-label="Dismiss">
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

/** After switching to One-line, bring the kept selection into view. */
function useRevealSelection() {
  const reveal = useDrawingStyle((s) => s.reveal);
  const { fitView, getZoom } = useReactFlow();
  useEffect(() => {
    if (!reveal) return;
    // A new drawing is first fitted to the screen, so wait for that.
    const timer = window.setTimeout(
      () => {
        const { nodes, edges, selectedNodeIds, selectedEdgeIds } = useDiagramStore.getState();
        const ids = new Set(selectedNodeIds);
        for (const e of edges)
          if (selectedEdgeIds.includes(e.id)) {
            ids.add(e.source);
            ids.add(e.target);
          }
        const show = nodes.filter((n) => ids.has(n.id)).map((n) => ({ id: n.id }));
        if (show.length) void fitView({ nodes: show, padding: 0.5, maxZoom: Math.max(getZoom(), 0.8), duration: 250 });
      },
      reveal.redrawn ? 500 : 0
    );
    return () => window.clearTimeout(timer);
  }, [reveal, fitView, getZoom]);
}

function Inner({ active }: { active: boolean }) {
  const style = useDrawingStyle((s) => s.style);
  const map = style === "map";
  // Network and Map both replace the one-line's canvas, toolbar and inspector.
  const network = style === "network" || map;
  // The network and the map are mounted the first time they are shown, then
  // kept, so switching back and forth doesn't lay them out again.
  const [networkShown, setNetworkShown] = useState(false);
  const [mapShown, setMapShown] = useState(false);
  useEffect(() => {
    if (style === "network") setNetworkShown(true);
    if (map) setMapShown(true);
  }, [style, map]);
  const styleBusy = useDrawingStyle((s) => s.busy);
  const styleError = useDrawingStyle((s) => s.error);
  useDrawingStyleSync();
  useRevealSelection();
  // Shortcuts (Delete, Ctrl+S) belong to the one-line only while it is visible.
  useKeyboardShortcuts(active && !network);
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
  // Layers (topology findings, filters and colours) are drawn on every
  // style; the one-line needs them only when it is drawn from the case.
  useLayerSync(active && (network || generated));
  const layersOpen = useLayers((s) => s.panel);
  const legends = useLayers((s) => s.legends);
  const tablesOpen = useLayers((s) => s.tables);
  const showLegend = network || generated;
  // The inspector only takes room while there is one thing to inspect: a
  // single symbol, a single wire, or a selected element this drawing lacks.
  const hasSelection = useDiagramStore(
    (s) =>
      s.selectedNodeIds.length === 1 ||
      (s.selectedNodeIds.length === 0 && s.selectedEdgeIds.length === 1) ||
      selectedCaseElement(s) !== null
  );
  // In a narrow pane (side by side) the inspector floats over the canvas
  // instead of taking a third of the width.
  const [rowRef, rowWidth] = useWidth<HTMLDivElement>(1200);
  const floating = rowWidth < 900;
  return (
    <div className="h-full flex flex-col bg-slate-50 overflow-hidden">
      {map ? <MapToolbar /> : network ? <NetworkToolbar /> : <Toolbar />}
      <div ref={rowRef} className="relative flex flex-1 min-h-0">
        {layersOpen && floating && <LayersPanel floating />}
        {layersOpen && !floating ? (
          <LayersPanel />
        ) : network ? null : paletteOpen ? (
          <aside aria-label="Components" className="relative w-56 shrink-0 border-r border-slate-200 bg-white overflow-y-auto" data-testid="components-palette">
            <button
              className="absolute right-1.5 top-1.5 z-10 p-1 rounded text-slate-500 hover:bg-slate-100 hover:text-slate-700"
              onClick={() => setPalette(false)}
              title="Hide components"
              aria-label="Hide components"
            >
              <PanelLeftClose size={15} />
            </button>
            <ComponentPalette />
          </aside>
        ) : (
          <aside aria-label="Components" className="w-10 shrink-0 border-r border-slate-200 bg-white flex flex-col items-center py-2">
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
        <div className={`@container flex-1 min-w-0 relative ${dark ? "" : "force-light"}`}>
          <DiagramCanvas />
          {!drawing.busy && !network && <DrawPrompt />}
          {missing && !drawing.busy && !blank && !network && <NotDrawn element={missing} />}
          {(networkShown || style === "network") && (
            <div className={style === "network" ? "absolute inset-0 z-20" : "hidden"}>
              <NetworkCanvas visible={active && style === "network"} />
            </div>
          )}
          {(mapShown || map) && (
            <div className={map ? "absolute inset-0 z-20" : "hidden"}>
              <MapCanvas visible={active && map} />
            </div>
          )}
          {showLegend && <LayerLegend legends={legends} />}
          {(styleBusy || styleError) && (
            <div
              className={`absolute left-1/2 top-3 -translate-x-1/2 z-30 flex items-center gap-2 rounded-md border px-3 py-1.5 text-xs shadow-sm whitespace-nowrap ${
                styleError ? "border-red-200 bg-red-50 text-red-800" : "border-slate-200 bg-white/95 text-slate-700"
              }`}
              data-testid="style-switch-status"
            >
              {styleError ?? "Drawing the one-line…"}
              {styleError && (
                <button className="opacity-60 hover:opacity-100" onClick={() => useDrawingStyle.setState({ error: null })} aria-label="Dismiss">
                  <X size={12} />
                </button>
              )}
            </div>
          )}
          {(drawing.busy || drawing.error) && (
            <div
              className={`absolute left-1/2 top-3 -translate-x-1/2 z-30 flex items-center gap-2 rounded-md border px-3 py-1.5 text-xs shadow-sm whitespace-nowrap ${
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
        </div>
        {style === "network" && <NetworkInspector floating={floating} />}
        {map && <MapInspector floating={floating} />}
        {!network && (hasSelection || missing) && (
          <aside aria-label="Inspector" className={inspectorClass(floating)} data-testid="inspector">
            <PropertiesPanel missing={hasSelection ? null : missing} />
          </aside>
        )}
      </div>
      {tablesOpen && <TablesDrawer />}
      {!network && <StatusBar />}
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
