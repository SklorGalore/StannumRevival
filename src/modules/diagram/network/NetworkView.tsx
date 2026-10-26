// The Diagram's Network style: the topology graph (Cytoscape) drawn around a
// bus, sharing the focus bus, hop count and selection with the One-line style.
import { useEffect, useMemo, useRef } from "react";
import { Maximize2, Minus, Plus, Spline } from "lucide-react";
import { useWorkspace } from "../../../store/workspaceStore";
import { useDiagramStore } from "../../../store/diagramStore";
import { MAX_HOPS, useDrawingStyle } from "../../../store/drawingStyleStore";
import { useTopology } from "../../../lib/analysis/topologyStore";
import type { TopologySnapshot } from "../../../lib/analysis/types";
import { busElement } from "../../../lib/elements";
import { count } from "../../../lib/analysis/format";
import { centreBus } from "../../../ui/DrawAround";
import { NetworkGraph } from "../../topology/NetworkGraph";
import { ElementInspector, InspectorSection, inspectorClass } from "../../../ui/ElementInspector";
import { DISPLAY_LIMIT, focusedGraph, islandGraph, locateElement, neighborhood, nodeForBus, type GraphSelection } from "../../topology/viewModel";
import { useLayers } from "../../../store/layerStore";
import { filterGraph } from "../../topology/fields/field-view";
import { isComplete } from "../../topology/fields/storage";
import { showOneLine } from "./switchStyle";
import { useWidth } from "../../../ui/useWidth";
import { NARROW_LEFT } from "../../../components/canvas/CaseActions";
import { StyleSwitch } from "./StyleSwitch";
import { SourceFields } from "./SourceFields";
import { LayersButton } from "../layers/LayersPanel";
import { TablesButton } from "../tables/TablesDrawer";

export { useSnapshot } from "./useSnapshot";
import { useSnapshot } from "./useSnapshot";

const NOTHING: GraphSelection = { bus: null, equipment: null };

/** Draw the one-line around a bus with the network's hop count, and switch to it. */
function drawOneLineAround(bus: number) {
  const { focus, setFocus } = useDrawingStyle.getState();
  setFocus({ bus, depth: focus?.depth ?? 2 });
  void showOneLine();
}

/** The bus to draw around for the selected element. */
function selectionBus(element: string | null, snapshot: TopologySnapshot | null): number | null {
  if (!element) return null;
  if (snapshot) {
    const found = locateElement(element, snapshot);
    if (found) return found.selection.bus ?? found.bus;
  }
  return centreBus(element);
}

/** The Diagram toolbar while the Network style is shown. */
export function NetworkToolbar() {
  const focus = useDrawingStyle((s) => s.focus);
  const element = useDrawingStyle((s) => s.element);
  const busy = useDrawingStyle((s) => s.busy);
  const requestFit = useDrawingStyle((s) => s.requestFit);
  const canDraw = useWorkspace((s) => !!s.caseSummary?.power.available);
  const snapshot = useTopology((s) => s.snapshot);
  const bus = selectionBus(element, snapshot) ?? focus?.bus ?? null;
  const [ref, width] = useWidth<HTMLDivElement>(1440);
  return (
    <div ref={ref} className="h-12 flex items-center px-3 gap-1 border-b border-slate-200 bg-white select-none overflow-hidden whitespace-nowrap shrink-0">
      {width >= 600 && <span className="shrink-0 font-semibold text-slate-700 text-sm mr-3">Diagram</span>}
      <StyleSwitch compact={width < 520} />
      <LayersButton showLabel={width >= 640} />
      <TablesButton showLabel={width >= 640} />
      <div className="w-px h-5 bg-slate-200 mx-1" />
      <button
        className="shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium text-slate-600 hover:bg-slate-100"
        onClick={requestFit}
        title="Fit the network to the view"
      >
        <Maximize2 size={14} />
        {width >= 520 && <span>Fit</span>}
      </button>
      <button
        className="shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium text-teal-700 hover:bg-teal-50 disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={!canDraw || bus === null || busy}
        onClick={() => bus !== null && drawOneLineAround(bus)}
        title={
          !canDraw
            ? "This case can't be read for power flow, so it can't be drawn as a one-line"
            : bus !== null
              ? `Draw ${focus?.depth ?? 2} hops around bus ${bus} as a one-line`
              : "Pick a bus first"
        }
      >
        <Spline size={14} />
        <span>{busy ? "Drawing…" : "Draw as one-line"}</span>
      </button>
    </div>
  );
}

/** The network graph around the focus bus, with its hop bar. */
export function NetworkCanvas({ visible }: { visible: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const { snapshot, error } = useSnapshot(visible);
  const focus = useDrawingStyle((s) => s.focus);
  const setFocus = useDrawingStyle((s) => s.setFocus);
  const element = useDrawingStyle((s) => s.element);
  const setElement = useDrawingStyle((s) => s.setElement);
  const fit = useDrawingStyle((s) => s.fit);
  const [ref, width] = useWidth<HTMLDivElement>(1200);
  const around = useDiagramStore((s) => s.metadata.around);
  const linked = useDiagramStore((s) => !!s.metadata.caseName && s.metadata.caseName === summary?.name);

  // A one-line drawn around a bus (a "Draw around" request, its hop bar)
  // moves the network with it.
  useEffect(() => {
    if (around && linked) setFocus(around);
  }, [around, linked, setFocus]);

  // Start around the selection, or the best-connected bus.
  useEffect(() => {
    if (!snapshot || focus) return;
    const bus = selectionBus(element, snapshot) ?? snapshot.analysis.degrees[0]?.bus;
    if (bus !== undefined && bus !== null) setFocus({ bus, depth: 2 });
  }, [snapshot, focus, element, setFocus]);

  const view = useDrawingStyle((s) => s.view);
  const pathResult = useLayers((s) => s.path?.result ?? null);
  const marks = useLayers((s) => s.marks);
  const fieldsOn = useLayers((s) => s.on.fields);
  const rules = useLayers((s) => s.rules);
  const complete = useMemo(() => rules.filter(isComplete), [rules]);
  const scoped = useMemo(() => {
    if (!snapshot) return null;
    if (view.kind === "island") return islandGraph(snapshot, view.island);
    if (view.kind === "path") return pathResult ? focusedGraph(snapshot.graph, new Set(pathResult.node_ids)) : null;
    return focus ? neighborhood(snapshot.graph, focus.bus, focus.depth) : null;
  }, [snapshot, focus, view, pathResult]);
  // Field filters hide what doesn't match (the one-line fades it instead).
  const graph = useMemo(
    () => (scoped && snapshot && fieldsOn && complete.length ? filterGraph(snapshot, scoped, complete) : scoped),
    [scoped, snapshot, fieldsOn, complete]
  );
  const located = useMemo(() => (snapshot && element ? locateElement(element, snapshot) : null), [snapshot, element]);
  const selection = located?.selection ?? NOTHING;

  // A new selection made elsewhere that isn't in view re-centres the network
  // on it. (The selection the network opened with, and changing the hop
  // count, never move the focus.)
  const graphRef = useRef(graph);
  graphRef.current = graph;
  const followed = useRef(element);
  useEffect(() => {
    const shownGraph = graphRef.current;
    if (!shownGraph || !located || followed.current === element) return;
    followed.current = element;
    if (located.bus === null) return;
    const shown = located.selection.equipment
      ? shownGraph.edges.some((e) => e.equipment_id === located.selection.equipment)
      : located.selection.bus !== null && !!nodeForBus(shownGraph, located.selection.bus);
    if (!shown) setFocus({ bus: located.bus, depth: useDrawingStyle.getState().focus?.depth ?? 2 });
  }, [element, located, setFocus]);

  function pick(next: GraphSelection) {
    const picked = next.equipment ?? (next.bus !== null ? busElement(next.bus) : null);
    setElement(picked);
    if (picked) useWorkspace.getState().select(picked, "diagram");
  }

  let message: string | null = null;
  if (!summary) message = "Open a case to draw its network.";
  else if (!summary.topology.available) message = `This case has no topology to draw. ${summary.topology.error ?? ""}`;
  else if (error) message = `Couldn't build the network: ${error}`;
  else if (!snapshot || !graph) message = "Building the network…";
  else if (graph.nodes.length > DISPLAY_LIMIT)
    message =
      view.kind === "island"
        ? `Island ${view.island} has ${count(graph.nodes.length)} buses, more than the ${DISPLAY_LIMIT} the network view draws. Pick a bus in it instead.`
        : `${count(graph.nodes.length)} buses are within ${focus!.depth} hops, more than the ${DISPLAY_LIMIT} the network view draws. Use fewer hops.`;
  else if (!graph.nodes.length)
    message = scoped?.nodes.length ? "Nothing here matches the filters." : `Bus ${focus?.bus} isn't in service, so it has no network to draw.`;

  return (
    <div ref={ref} className="@container absolute inset-0 bg-white" data-testid="network-view">
      {message ? (
        <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-slate-500">{message}</div>
      ) : (
        <NetworkGraph
          graph={graph!}
          snapshot={snapshot!}
          marks={marks}
          selection={selection}
          onInspect={pick}
          visible={visible}
          fitSignal={fit}
          focus={focus?.bus ?? null}
          onRecentre={(bus) => setFocus({ bus, depth: focus?.depth ?? 2 })}
        />
      )}
      {snapshot && view.kind !== "around" && <ScopeBar buses={graph?.nodes.filter((n) => n.bus !== null).length ?? 0} />}
      {snapshot && focus && view.kind === "around" && (
        <HopBar snapshot={snapshot} buses={graph?.nodes.filter((n) => n.bus !== null).length ?? 0} hint={width >= 760} />
      )}
    </div>
  );
}

/** Showing a whole island or a path: say so, with the way back. */
function ScopeBar({ buses }: { buses: number }) {
  const view = useDrawingStyle((s) => s.view);
  const focus = useDrawingStyle((s) => s.focus);
  const showAround = useDrawingStyle((s) => s.showAround);
  const path = useLayers((s) => s.path);
  const label =
    view.kind === "island" ? `Island ${view.island}` : path?.result ? `Path ${path.from} → ${path.to}, ${path.result.hops} hops` : "Path";
  return (
    <div
      className={`${NARROW_LEFT} bottom-3 z-10 flex items-center gap-1 px-2 py-1 rounded-lg border border-slate-200 bg-white shadow-sm text-[11px] text-slate-600 whitespace-nowrap`}
      data-testid="network-scope"
    >
      <span className="px-1 font-semibold text-slate-800">{label}</span>
      <span className="px-1 tabular-nums">{count(buses)} buses</span>
      {focus && (
        <>
          <span className="w-px h-4 bg-slate-200 mx-1" />
          <button className="px-1.5 py-0.5 rounded text-teal-700 hover:bg-teal-50" onClick={showAround}>
            Back to bus {focus.bus}
          </button>
        </>
      )}
    </div>
  );
}

/** Which bus the network is drawn around, and how many hops. */
function HopBar({ snapshot, buses, hint }: { snapshot: TopologySnapshot; buses: number; hint: boolean }) {
  const focus = useDrawingStyle((s) => s.focus)!;
  const setFocus = useDrawingStyle((s) => s.setFocus);
  const name = snapshot.case.buses.find((b) => b.number === focus.bus)?.name.trim();
  const step = (d: number) => setFocus({ bus: focus.bus, depth: focus.depth + d });
  const btn = "p-1 rounded text-slate-600 hover:bg-slate-100 disabled:opacity-30 disabled:hover:bg-transparent";
  return (
    <div
      className={`${NARROW_LEFT} bottom-3 z-10 flex items-center gap-1 px-2 py-1 rounded-lg border border-slate-200 bg-white shadow-sm text-[11px] text-slate-600 whitespace-nowrap`}
      data-testid="network-hops"
    >
      <span className="px-1">
        Around <span className="font-semibold text-slate-800">Bus {focus.bus}</span>
        {name && hint && <span className="text-slate-500"> {name}</span>}
      </span>
      <span className="w-px h-4 bg-slate-200 mx-1" />
      <button className={btn} title="One hop fewer" aria-label="One hop fewer" disabled={focus.depth <= 1} onClick={() => step(-1)}>
        <Minus size={13} />
      </button>
      <span className="tabular-nums w-12 text-center" aria-label="Hops">
        {focus.depth} hop{focus.depth === 1 ? "" : "s"}
      </span>
      <button className={btn} title="One hop more" aria-label="One hop more" disabled={focus.depth >= MAX_HOPS} onClick={() => step(1)}>
        <Plus size={13} />
      </button>
      <span className="w-px h-4 bg-slate-200 mx-1" />
      <span className="px-1 tabular-nums">{count(buses)} buses</span>
      {hint && <span className="px-1 text-slate-400">Double-click a bus to centre on it</span>}
    </div>
  );
}

/**
 * The shared element inspector for what is selected in the network, with a
 * Network section: its topology facts and the actions that move between
 * styles. It takes the same column as the one-line's inspector.
 */
export function NetworkInspector({ floating = false }: { floating?: boolean }) {
  const element = useDrawingStyle((s) => s.element);
  const setElement = useDrawingStyle((s) => s.setElement);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const snapshot = useTopology((s) => (s.version === caseVersion ? s.snapshot : null));
  if (!element || !snapshot || !locateElement(element, snapshot)) return null;
  const pick = (next: string) => {
    setElement(next);
    useWorkspace.getState().select(next, "diagram");
  };
  return (
    <aside className={inspectorClass(floating)} data-testid="network-selection">
      <ElementInspector element={element} from="diagram" onPick={pick} onClose={() => setElement(null)}>
        <NetworkSection element={element} snapshot={snapshot} />
      </ElementInspector>
    </aside>
  );
}

/** Islands, bridges and cut buses for the selection, and Centre here / Draw as one-line. */
function NetworkSection({ element, snapshot }: { element: string; snapshot: TopologySnapshot }) {
  const focus = useDrawingStyle((s) => s.focus);
  const setFocus = useDrawingStyle((s) => s.setFocus);
  const busy = useDrawingStyle((s) => s.busy);
  const canDraw = useWorkspace((s) => !!s.caseSummary?.power.available);
  const found = locateElement(element, snapshot)!;
  const { analysis: a, graph, case: c } = snapshot;
  const bus = found.selection.bus ?? found.bus;
  let facts: React.ReactNode;
  if (found.selection.equipment) {
    const id = found.selection.equipment;
    const e = c.equipment.find((x) => x.id === id);
    const inGraph = graph.edges.some((g) => g.equipment_id === id);
    const absorbed = graph.absorbed_equipment_ids.includes(id);
    const bridge = a.bridges.some((b) => b.equipment_id === id);
    facts = (
      <>
        <p className="mb-1.5">
          <span className={`rounded px-1.5 py-px text-[10px] font-semibold ${inGraph ? "bg-emerald-100 text-emerald-800" : "bg-slate-100 text-slate-600"}`}>
            {inGraph ? "In the active network" : absorbed ? "Inside a merged node" : "Left out of the network"}
          </span>
        </p>
        {bridge ? (
          <p className="rounded bg-amber-50 px-2 py-1 text-amber-900">Bridge: taking it out splits its island.</p>
        ) : inGraph ? (
          <p className="text-slate-500">Not a bridge: its island stays connected without it.</p>
        ) : null}
        {e && e.active_terminals.some((t) => !t) && (
          <p className="mt-2 text-slate-600">
            Open at {e.buses.filter((_, i) => e.active_terminals[i] === false).map((b) => `bus ${b}`).join(", ")}.
          </p>
        )}
        {e && <SourceFields source={e.source} />}
      </>
    );
  } else {
    const node = found.selection.bus !== null ? nodeForBus(snapshot.graph, found.selection.bus) : undefined;
    const degree = a.degrees.find((d) => d.bus === (node?.bus ?? found.selection.bus));
    const record = c.buses.find((b) => b.number === (node?.bus ?? found.selection.bus));
    const cut = found.selection.bus !== null && a.articulation_buses.includes(node?.bus ?? found.selection.bus);
    facts = (
      <>
        {node && node.member_buses.length > 1 && (
          <p className="mb-1.5 text-slate-600">Merged node of {node.member_buses.length} buses: {node.member_buses.join(", ")}</p>
        )}
        <dl className="grid grid-cols-2 gap-1.5">
          <div><dt className="text-slate-500">Island</dt><dd className="font-medium">{degree?.island ?? "—"}</dd></div>
          <div><dt className="text-slate-500">Connections</dt><dd className="font-medium">{degree?.degree ?? "—"}</dd></div>
        </dl>
        {cut && <p className="mt-2 rounded bg-amber-50 px-2 py-1 text-amber-900">Cut bus: losing it splits its island.</p>}
        {record && <SourceFields source={record.source} />}
      </>
    );
  }
  return (
    <InspectorSection id="network" title="Network">
      {facts}
      {bus !== null && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {found.selection.bus !== null && focus?.bus !== bus && (
            <button className="rounded-md border border-slate-300 px-2 py-1 font-medium text-slate-700 hover:bg-slate-50" onClick={() => setFocus({ bus, depth: focus?.depth ?? 2 })}>
              Centre here
            </button>
          )}
          {canDraw && (
            <button
              className="inline-flex items-center gap-1 rounded-md bg-teal-700 px-2 py-1 font-medium text-white hover:bg-teal-800 disabled:opacity-40"
              disabled={busy}
              onClick={() => drawOneLineAround(bus)}
              title={`Draw ${focus?.depth ?? 2} hops around bus ${bus} as a one-line`}
            >
              <Spline size={12} /> Draw as one-line
            </button>
          )}
        </div>
      )}
    </InspectorSection>
  );
}
