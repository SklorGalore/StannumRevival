import { publishDiagramSelection } from "../../modules/diagram/useSharedSelection";
import { useDarkCanvas } from "../../lib/diagram/ink";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  BackgroundVariant,
  ConnectionMode,
  SelectionMode,
  ConnectionLineType,
  useUpdateNodeInternals,
  useNodesInitialized,
  type Connection,
  type FinalConnectionState,
  type NodeChange,
  type EdgeChange,
  type Edge,
  type OnNodesChange,
  type OnEdgesChange,
  applyNodeChanges,
  applyEdgeChanges,
  addEdge,
  useReactFlow,
} from "@xyflow/react";
import { nanoid } from "nanoid";
import { nodeTypes } from "../nodes";
import { edgeTypes } from "../edges";
import { useDiagramStore } from "../../store/diagramStore";
import { defaultsByType, defaultConductorData } from "../../store/defaults";
import type { SLDNode, SLDEdge, SLDNodeType, BusData, BusTap, OffPageData } from "../../store/types";
import { busTaps } from "../nodes/shared/busTaps";
import {
  BAR,
  across,
  along,
  isVertical,
  outward,
  placeTerminal,
  sideForTerminal,
  sideToward,
  tapPoint,
  terminalOf,
  type Side,
} from "../../lib/diagram/bus";
import { symbolSize } from "../../lib/diagram/sizes";
import { AlignBar } from "./AlignBar";
import { VoltageLegend } from "./VoltageLegend";
import { LocalScopeBar, NodeMenu, useCaseActions } from "./CaseActions";
import { warmLayout } from "../../lib/diagram/generate";
import { useWireRouting } from "./useWireRouting";

/** Changes that don't alter the drawing itself. */
const VIEW_ONLY = new Set(["select", "dimensions"]);
const changesDrawing = (changes: (NodeChange | EdgeChange)[]) => changes.some((c) => !VIEW_ONLY.has(c.type));

/** How far above or below a bus bar a wire may be released and still land on it. */
const BUS_REACH = 14;

/** The bus bar under a point in diagram coordinates, if any. */
function busAt(nodes: SLDNode[], p: { x: number; y: number }): SLDNode | null {
  let best: { node: SLDNode; d: number } | null = null;
  for (const n of nodes) {
    if (n.type !== "bus") continue;
    const data = n.data as BusData;
    if (data.star) continue;
    const at = along(n, p);
    if (at < -4 || at > (data.length ?? 200) + 4) continue;
    const d = Math.abs(across(n, p));
    if (d <= BUS_REACH && (!best || d < best.d)) best = { node: n, d };
  }
  return best?.node ?? null;
}

/** Terminal of each symbol that can sit straight on a bus bar. */
const ON_BUS: Partial<Record<string, string>> = { generator: "gen-top", load: "load-top", shunt: "shunt-top" };

/** Bus whose bar a symbol's terminal at `p`, facing `facing`, touches: from
 *  just past the bar to `reach` out on the symbol's side. Also says which
 *  side of the bar the symbol hangs from. */
function barUnder(
  nodes: SLDNode[],
  p: { x: number; y: number },
  facing: Side,
  reach: number
): { bus: SLDNode; side: BusTap["side"] } | null {
  for (const n of nodes) {
    if (n.type !== "bus") continue;
    const data = n.data as BusData;
    if (data.star) continue;
    const side = sideForTerminal(isVertical(data), facing);
    if (!side) continue;
    const at = along(n, p);
    if (at < 0 || at > (data.length ?? 200)) continue;
    // How far out the terminal is on the symbol's side of the bar.
    const out = outward(n, p, side);
    if (out >= -BAR / 2 - 8 && out <= BAR / 2 + reach) return { bus: n, side };
  }
  return null;
}

/** Centre of a node's box. */
function centreOf(n: SLDNode) {
  const { w, h } = n.type === "bus" ? { w: 0, h: 0 } : symbolSize(n);
  return { x: n.position.x + w / 2, y: n.position.y + h / 2 };
}

function pointerPoint(e: MouseEvent | TouchEvent) {
  const t = "changedTouches" in e ? e.changedTouches[0] : (e as MouseEvent);
  return { x: t.clientX, y: t.clientY };
}

export function DiagramCanvas() {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition, fitView } = useReactFlow();
  const updateNodeInternals = useUpdateNodeInternals();
  const checkpoint = useDiagramStore((s) => s.checkpoint);

  const nodes = useDiagramStore((s) => s.nodes);
  const edges = useDiagramStore((s) => s.edges);
  const gridVisible = useDiagramStore((s) => s.gridVisible);
  const snapToGrid = useDiagramStore((s) => s.snapToGrid);
  const gridSize = useDiagramStore((s) => s.gridSize);

  const setNodes = useDiagramStore((s) => s.setNodes);
  const setEdges = useDiagramStore((s) => s.setEdges);
  const setSelection = useDiagramStore((s) => s.setSelection);
  const addNode = useDiagramStore((s) => s.addNode);
  const setViewport = useDiagramStore((s) => s.setViewport);
  const fitRequest = useDiagramStore((s) => s.fitRequest);
  const dark = useDarkCanvas();
  const [menu, setMenu] = useState<{ x: number; y: number; node: SLDNode } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const { linked, expand } = useCaseActions();

  // The first layout of a session pays to start the layout worker; do it now.
  useEffect(() => {
    const id = window.setTimeout(warmLayout, 300);
    return () => window.clearTimeout(id);
  }, []);
  // Wires take right-angle paths around symbols once they're drawn or moved.
  useWireRouting(wrapperRef);

  // Fit when a drawing is opened or generated, not when the first symbol is
  // dropped (React Flow's `fitView` prop zoomed a lone symbol to 400%). The
  // fit waits until every new symbol has been measured.
  const nodesReady = useNodesInitialized();
  const fitted = useRef(0);
  useEffect(() => {
    if (!fitRequest || fitted.current === fitRequest || !nodesReady) return;
    fitted.current = fitRequest;
    const id = requestAnimationFrame(() => {
      void fitView({ padding: 0.08, maxZoom: 1.25, duration: 200 });
    });
    return () => cancelAnimationFrame(id);
  }, [fitRequest, fitView, nodesReady]);

  const onNodesChange: OnNodesChange = useCallback(
    (changes) => {
      setNodes(applyNodeChanges(changes, nodes as never[]) as unknown as SLDNode[], changesDrawing(changes));
    },
    [nodes, setNodes]
  );

  const onEdgesChange: OnEdgesChange = useCallback(
    (changes) => {
      setEdges(applyEdgeChanges(changes, edges as never[]) as unknown as SLDEdge[], changesDrawing(changes));
    },
    [edges, setEdges]
  );

  /** Wire a symbol's terminal to a bus at `x` along the bar, on the side
   *  facing the symbol, adding a tap there if the bus has none. */
  const wireToBus = useCallback(
    (bus: SLDNode, x: number, side: BusTap["side"], node: string, handle: string | null) => {
      const { nodes: all, edges: wires } = useDiagramStore.getState();
      const data = bus.data as BusData;
      const taps = busTaps(data);
      let tap = taps.find((t) => t.side === side && Math.abs(t.x - x) < 1);
      if (!tap) {
        tap = { id: `bus-tap-${nanoid(6)}`, x, side };
        const next = [...taps, tap];
        useDiagramStore.getState().setNodes(
          all.map((n) => (n.id === bus.id ? ({ ...n, data: { ...n.data, taps: next } } as SLDNode) : n))
        );
      }
      const edge = {
        id: nanoid(),
        type: "conductor",
        // Bus taps are source handles. React Flow only looks a wire's
        // source up among source handles, so the bus end goes first.
        source: bus.id,
        sourceHandle: tap.id,
        target: node,
        targetHandle: handle,
        data: { ...defaultConductorData, voltageKV: data.voltageKV },
      } as unknown as SLDEdge;
      // The new tap must be measured before a wire can attach to it.
      requestAnimationFrame(() => {
        updateNodeInternals(bus.id);
        requestAnimationFrame(() => useDiagramStore.getState().setEdges([...wires, edge]));
      });
    },
    [updateNodeInternals]
  );

  /** A generator, load or shunt whose terminal touches a bus bar snaps onto
   *  it and is wired there. Returns false when it isn't on a bus. */
  const attachToBus = useCallback(
    (nodeId: string, reach: number) => {
      const { nodes: all, edges: wires, gridSize } = useDiagramStore.getState();
      const node = all.find((n) => n.id === nodeId);
      const handle = node && ON_BUS[node.type ?? ""];
      if (!node || !handle) return false;
      if (wires.some((e) => (e.target === nodeId && e.targetHandle === handle) || (e.source === nodeId && e.sourceHandle === handle)))
        return false;
      const terminal = terminalOf(node);
      const hit = barUnder(all, terminal.p, terminal.side, reach);
      if (!hit) return false;
      const { bus, side } = hit;
      const len = (bus.data as BusData).length ?? 200;
      const x = Math.min(len, Math.max(0, Math.round(along(bus, terminal.p) / gridSize) * gridSize));
      // Hang the symbol from the side of the bar its terminal faces.
      const { w, h } = symbolSize(node);
      const position = placeTerminal(tapPoint(bus, x, side), terminal.side, w, h);
      useDiagramStore.getState().setNodes(all.map((n) => (n.id === nodeId ? ({ ...n, position } as SLDNode) : n)));
      wireToBus(useDiagramStore.getState().nodes.find((n) => n.id === bus.id)!, x, side, nodeId, handle);
      return true;
    },
    [wireToBus]
  );

  const onConnect = useCallback(
    (params: Connection) => {
      checkpoint();
      // A wire from below that snapped to a tap on top of the bus (or the
      // reverse) would loop through the bar: use the facing side instead.
      for (const [end, other, otherHandle] of [
        ["source", params.target, params.targetHandle],
        ["target", params.source, params.sourceHandle],
      ] as const) {
        const bus = nodes.find((n) => n.id === params[end]);
        const otherNode = nodes.find((n) => n.id === other);
        if (bus?.type !== "bus" || !otherNode || (bus.data as BusData).star) continue;
        const tap = busTaps(bus.data as BusData).find((t) => t.id === params[`${end}Handle`]);
        if (!tap) continue;
        const facing = sideToward(bus, centreOf(otherNode));
        if (tap.side !== facing) return wireToBus(bus, tap.x, facing, other, otherHandle ?? null);
      }
      const newEdge: SLDEdge = {
        ...params,
        id: nanoid(),
        type: "conductor",
        data: { ...defaultConductorData },
      } as unknown as SLDEdge;
      setEdges(addEdge(newEdge as Edge, edges as Edge[]) as unknown as SLDEdge[]);
    },
    [nodes, edges, setEdges, checkpoint, wireToBus]
  );

  // Highlight the bus a wire would land on while it's being drawn.
  const dropTarget = useRef<string | null>(null);
  const markDropTarget = useCallback((id: string | null) => {
    if (dropTarget.current === id) return;
    const el = (nid: string) => wrapperRef.current?.querySelector(`.react-flow__node[data-id="${CSS.escape(nid)}"]`);
    if (dropTarget.current) el(dropTarget.current)?.classList.remove("bus-drop-target");
    if (id) el(id)?.classList.add("bus-drop-target");
    dropTarget.current = id;
  }, []);

  const onConnectStart = useCallback(
    (_: unknown, { nodeId }: { nodeId: string | null }) => {
      const fromBus = useDiagramStore.getState().nodes.find((n) => n.id === nodeId)?.type === "bus";
      if (fromBus) return;
      const move = (e: PointerEvent) => {
        // Over a terminal, React Flow connects to that instead.
        const overHandle = (e.target as Element | null)?.closest?.(".react-flow__handle");
        const bus = overHandle ? null : busAt(useDiagramStore.getState().nodes, screenToFlowPosition({ x: e.clientX, y: e.clientY }, { snapToGrid: false }));
        markDropTarget(bus?.id ?? null);
      };
      const stop = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", stop);
        // Leave the class until onConnectEnd has read it.
        requestAnimationFrame(() => markDropTarget(null));
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", stop);
    },
    [screenToFlowPosition, markDropTarget]
  );

  // Released on a bus bar rather than a terminal: add a tap there and connect to it.
  const onConnectEnd = useCallback(
    (event: MouseEvent | TouchEvent, state: FinalConnectionState) => {
      if (state.isValid || !state.fromNode || !state.fromHandle) return;
      const { nodes: all, gridSize } = useDiagramStore.getState();
      const from = all.find((n) => n.id === state.fromNode!.id);
      if (!from || from.type === "bus") return;
      const p = screenToFlowPosition(pointerPoint(event), { snapToGrid: false });
      const bus = busAt(all, p);
      if (!bus) return;
      const len = (bus.data as BusData).length ?? 200;
      const x = Math.min(len, Math.max(0, Math.round(along(bus, p) / gridSize) * gridSize));
      // The tap goes on the side facing the symbol, so the wire never crosses the bar.
      const side = sideToward(bus, centreOf(from));
      checkpoint();
      wireToBus(bus, x, side, from.id, state.fromHandle.id ?? null);
    },
    [screenToFlowPosition, checkpoint, wireToBus]
  );

  const isValidConnection = useCallback(
    (connection: Edge | Connection): boolean => {
      const { source, target } = connection;
      if (source === target) return false;
      const sourceNode = nodes.find((n) => n.id === source);
      const targetNode = nodes.find((n) => n.id === target);
      if (!sourceNode || !targetNode) return false;
      // Bus-to-bus direct connections forbidden
      if (sourceNode.type === "bus" && targetNode.type === "bus") return false;
      return true;
    },
    [nodes]
  );

  const onDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();
      const type = event.dataTransfer.getData(
        "application/sld-node-type"
      ) as SLDNodeType;
      const dataJson = event.dataTransfer.getData("application/sld-node-data");
      if (!type) return;

      const position = screenToFlowPosition({
        x: event.clientX,
        y: event.clientY,
      });

      const snapped = snapToGrid
        ? {
            x: Math.round(position.x / gridSize) * gridSize,
            y: Math.round(position.y / gridSize) * gridSize,
          }
        : position;

      const defaults = defaultsByType[type] ?? {};
      const data = dataJson ? JSON.parse(dataJson) : defaults;

      const newNode: SLDNode = {
        id: nanoid(),
        type,
        position: snapped,
        data,
      } as SLDNode;

      addNode(newNode);
      // Dropped onto a bus bar: it goes straight on the bus. The pointer
      // stands for the terminal; symbols come from the palette upright.
      // On a vertical bus it is turned to face the bar.
      if (ON_BUS[type]) {
        const all = useDiagramStore.getState().nodes;
        const hit =
          barUnder(all, position, "top", 30) ??
          barUnder(all, position, "left", 30) ??
          barUnder(all, position, "right", 30);
        if (hit) {
          const vertical = isVertical(hit.bus.data as BusData);
          const rotation = vertical ? (hit.side === "top" ? 270 : 90) : 0;
          // Put the terminal near the bar so the attach below finds it.
          const p = tapPoint(hit.bus, along(hit.bus, position), hit.side);
          const facing = vertical ? (rotation === 270 ? "left" : "right") : "top";
          const { w, h } = symbolSize({ ...newNode, data: { ...newNode.data, rotation } } as SLDNode);
          const onBar = placeTerminal(p, facing, w, h);
          const { setNodes: put } = useDiagramStore.getState();
          put(
            all.map((n) =>
              n.id === newNode.id ? ({ ...n, position: onBar, data: { ...n.data, rotation } } as SLDNode) : n
            )
          );
          attachToBus(newNode.id, 30);
        }
      }
    },
    [screenToFlowPosition, snapToGrid, gridSize, addNode, attachToBus]
  );

  const onDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);

  return (
    <div ref={wrapperRef} className="w-full h-full relative">
      <AlignBar />
      <LocalScopeBar />
      <NodeMenu menu={menu} onClose={closeMenu} />
      <ReactFlow
        nodes={nodes as never[]}
        edges={edges as never[]}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onConnectStart={onConnectStart}
        onConnectEnd={onConnectEnd}
        onNodeDragStart={() => checkpoint()}
        onNodeDragStop={(_, node) => attachToBus(node.id, 20)}
        onNodeContextMenu={(e, node) => {
          if (!linked) return;
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY, node: node as unknown as SLDNode });
        }}
        onNodeDoubleClick={(_, node) => {
          // Double-clicking an off-page marker brings its far bus into the drawing.
          if (!linked || node.type !== "offPage") return;
          const wire = edges.find((e) => e.source === node.id || e.target === node.id);
          const busNode = wire && nodes.find((n) => n.id === (wire.source === node.id ? wire.target : wire.source));
          const ref = (busNode?.data as BusData | undefined)?.caseRef;
          // A bundled marker brings in every bus it leads to.
          const data = node.data as OffPageData;
          if (ref?.kind === "bus") void expand(ref.id, [...new Set((data.links ?? [data]).map((l) => l.bus))]);
        }}
        onSelectionDragStart={() => checkpoint()}
        isValidConnection={isValidConnection}
        onDrop={onDrop}
        onDragOver={onDragOver}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        defaultEdgeOptions={{
          type: "conductor",
          data: { ...defaultConductorData },
        }}
        snapToGrid={snapToGrid}
        snapGrid={[gridSize, gridSize]}
        onSelectionChange={({ nodes: sn, edges: se }) => {
          setSelection(
            sn.map((n) => n.id),
            se.map((e) => e.id)
          );
        }}
        // Only a click publishes: onSelectionChange also fires when another
        // module's selection is applied here, or when the nodes change.
        onNodeClick={(_, n) => publishDiagramSelection(n as unknown as SLDNode)}
        onEdgeClick={(_, e) => publishDiagramSelection(e as unknown as SLDEdge)}
        onMoveEnd={(_, vp) => setViewport(vp)}
        // Any terminal can join any other: a generator's terminal and a bus
        // tap are both "sources", which strict mode refused to connect.
        connectionMode={ConnectionMode.Loose}
        connectionLineType={ConnectionLineType.Step}
        // Snap to a terminal only when close, so a wire released between
        // taps lands on the bus there instead.
        connectionRadius={10}
        proOptions={{ hideAttribution: true }}
        minZoom={0.05}
        maxZoom={4}
        // Deleting, nudging and the other edit keys go through the store
        // (useKeyboardShortcuts) so each one can be undone.
        deleteKeyCode={null}
        disableKeyboardA11y
        // Drag on empty canvas draws a selection box; pan with the middle or
        // right button, or hold Space.
        selectionOnDrag
        selectionMode={SelectionMode.Partial}
        panOnDrag={[1, 2]}
        panActivationKeyCode="Space"
        multiSelectionKeyCode={["Shift", "Meta", "Control"]}
      >
        {gridVisible && (
          <Background
            variant={BackgroundVariant.Dots}
            gap={gridSize}
            size={1}
            color={dark ? "#334155" : "#cbd5e1"}
          />
        )}
        <Controls />
        <VoltageLegend />
        <MiniMap
          pannable
          zoomable
          nodeColor={(n) => (n.type === "bus" ? (dark ? "#cbd5e1" : "#334155") : dark ? "#64748b" : "#94a3b8")}
          maskColor={dark ? "rgba(2, 6, 23, 0.55)" : "rgba(241, 245, 249, 0.7)"}
          style={{ width: 160, height: 110, border: `1px solid ${dark ? "#334155" : "#e2e8f0"}`, borderRadius: 6 }}
        />
      </ReactFlow>
    </div>
  );
}
