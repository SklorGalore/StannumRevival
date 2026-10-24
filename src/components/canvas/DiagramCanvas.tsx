import { useCallback, useEffect, useRef } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  BackgroundVariant,
  ConnectionMode,
  type Connection,
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
import type { SLDNode, SLDEdge, SLDNodeType } from "../../store/types";

export function DiagramCanvas() {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition, fitView } = useReactFlow();

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

  // Fit when a drawing is opened or generated, not when the first symbol is
  // dropped (React Flow's `fitView` prop zoomed a lone symbol to 400%).
  useEffect(() => {
    if (!fitRequest) return;
    // Wait a frame so new nodes are measured before fitting.
    const id = requestAnimationFrame(() => {
      void fitView({ padding: 0.08, maxZoom: 1.25, duration: 200 });
    });
    return () => cancelAnimationFrame(id);
  }, [fitRequest, fitView]);

  const onNodesChange: OnNodesChange = useCallback(
    (changes) => {
      setNodes(applyNodeChanges(changes, nodes as never[]) as unknown as SLDNode[]);
    },
    [nodes, setNodes]
  );

  const onEdgesChange: OnEdgesChange = useCallback(
    (changes) => {
      setEdges(applyEdgeChanges(changes, edges as never[]) as unknown as SLDEdge[]);
    },
    [edges, setEdges]
  );

  const onConnect = useCallback(
    (params: Connection) => {
      const newEdge: SLDEdge = {
        ...params,
        id: nanoid(),
        type: "conductor",
        data: { ...defaultConductorData },
      } as unknown as SLDEdge;
      setEdges(addEdge(newEdge as Edge, edges as Edge[]) as unknown as SLDEdge[]);
    },
    [edges, setEdges]
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
    },
    [screenToFlowPosition, snapToGrid, gridSize, addNode]
  );

  const onDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);

  return (
    <div ref={wrapperRef} className="w-full h-full">
      <ReactFlow
        nodes={nodes as never[]}
        edges={edges as never[]}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
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
        onMoveEnd={(_, vp) => setViewport(vp)}
        // Any terminal can join any other: a generator's terminal and a bus
        // tap are both "sources", which strict mode refused to connect.
        connectionMode={ConnectionMode.Loose}
        proOptions={{ hideAttribution: true }}
        minZoom={0.05}
        maxZoom={4}
        deleteKeyCode={["Backspace", "Delete"]}
        multiSelectionKeyCode="Shift"
        selectionKeyCode="Shift"
      >
        {gridVisible && (
          <Background
            variant={BackgroundVariant.Dots}
            gap={gridSize}
            size={1}
            color="#cbd5e1"
          />
        )}
        <Controls />
        <MiniMap
          pannable
          zoomable
          nodeColor={(n) => (n.type === "bus" ? "#334155" : "#94a3b8")}
          maskColor="rgba(241, 245, 249, 0.7)"
          style={{ width: 160, height: 110, border: "1px solid #e2e8f0", borderRadius: 6 }}
        />
      </ReactFlow>
    </div>
  );
}
