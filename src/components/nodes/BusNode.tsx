import { useEffect, useRef } from "react";
import { Handle, Position, useReactFlow, useUpdateNodeInternals, type NodeProps } from "@xyflow/react";
import type { BusNode as BusNodeType } from "../../store/types";
import { useResults } from "../../lib/diagram/results";
import { useInk, SELECTED } from "../../lib/diagram/ink";
import { useDiagramStore } from "../../store/diagramStore";
import { busTaps } from "./shared/busTaps";

const BAR = 6;
const MIN_LENGTH = 40;

/** Grip on the right end of a selected bus: drag to lengthen or shorten it. */
function ResizeGrip({ id, length, minLength }: { id: string; length: number; minLength: number }) {
  const { getZoom } = useReactFlow();
  const updateInternals = useUpdateNodeInternals();
  const start = useRef<{ x: number; length: number } | null>(null);

  return (
    <div
      className="bus-grip nodrag nopan"
      title="Drag to resize the bus"
      style={{ left: length + 6 }}
      onPointerDown={(e) => {
        e.stopPropagation();
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        start.current = { x: e.clientX, length };
        useDiagramStore.getState().checkpoint();
      }}
      onPointerMove={(e) => {
        if (!start.current) return;
        const { gridSize } = useDiagramStore.getState();
        const raw = start.current.length + (e.clientX - start.current.x) / getZoom();
        const next = Math.max(minLength, Math.round(raw / gridSize) * gridSize);
        if (next === length) return;
        // Write straight to the store: the checkpoint above covers the drag.
        const { nodes, setNodes } = useDiagramStore.getState();
        setNodes(nodes.map((n) => (n.id === id ? ({ ...n, data: { ...n.data, length: next } } as typeof n) : n)));
        requestAnimationFrame(() => updateInternals(id));
      }}
      onPointerUp={() => {
        start.current = null;
      }}
    />
  );
}

export function BusNode({ id, data, selected }: NodeProps<BusNodeType>) {
  const width = data.length ?? 200;
  const ink = useInk();
  const color = selected ? SELECTED : ink.voltage(data.voltageKV);
  const results = useResults();
  const row = data.caseRef ? results?.buses.get(data.caseRef.id) : undefined;
  const taps = busTaps(data);
  // Re-measure when taps come or go (expand, undo, paste) so wires can attach.
  const updateInternals = useUpdateNodeInternals();
  const tapKey = taps.map((t) => `${t.id}${t.side}${t.x}`).join();
  useEffect(() => updateInternals(id), [tapKey, id, updateInternals]);
  // Fixed taps (from a generated layout or added by hand) must stay on the bar.
  const minLength = Math.max(MIN_LENGTH, ...(data.taps ?? []).map((t) => t.x + 20));

  const tapHandles = taps.map((t) => (
    <Handle
      key={t.id}
      type="source"
      id={t.id}
      position={t.side === "top" ? Position.Top : Position.Bottom}
      className="bus-tap"
      style={{ left: t.x }}
    />
  ));

  if (data.star) {
    // Star point of a three-winding transformer.
    return (
      <div className="node-wrapper relative" style={{ width, height: BAR }}>
        <svg width={width} height={BAR} overflow="visible" style={{ display: "block" }}>
          <circle cx={width / 2} cy={BAR / 2} r={4} fill={selected ? SELECTED : ink.line} />
        </svg>
        {tapHandles}
      </div>
    );
  }

  return (
    <div className="node-wrapper relative" style={{ width, height: BAR }}>
      <svg width={width} height={BAR} overflow="visible" style={{ display: "block" }}>
        {data.focus && (
          // The studied bus of a local drawing.
          <rect
            x={-6}
            y={-6}
            width={width + 12}
            height={BAR + 12}
            rx={4}
            fill={ink.mono ? "none" : "#fef3c7"}
            stroke={ink.mono ? ink.line : "#f59e0b"}
            strokeWidth={1}
            strokeDasharray={ink.mono ? "4 3" : undefined}
          >
            <title>Studied bus</title>
          </rect>
        )}
        <rect x={0} y={0} width={width} height={BAR} fill={color} rx={1} />
        <text x={-8} y={0} textAnchor="end" fontSize={11} fontWeight={600} fill={ink.line}>
          {data.label}
        </text>
        <text x={-8} y={13} textAnchor="end" fontSize={9.5} fill={ink.muted}>
          {data.caseRef ? `${data.caseRef.id} · ` : ""}
          {data.voltageKV} kV
        </text>
        {row && (
          <text
            x={width + 8}
            y={BAR / 2 + 3.5}
            fontSize={9.5}
            fill={row.violation > 0 ? ink.alert("#dc2626") : ink.result}
            fontWeight={row.violation > 0 ? 600 : 400}
          >
            {row.vm.toFixed(3)} pu {row.va.toFixed(1)}°
          </text>
        )}
      </svg>

      {tapHandles}
      <Handle type="source" position={Position.Left} id="bus-left" className="bus-tap" style={{ top: BAR / 2 }} />
      <Handle type="source" position={Position.Right} id="bus-right" className="bus-tap" style={{ top: BAR / 2 }} />
      {selected && <ResizeGrip id={id} length={width} minLength={minLength} />}
    </div>
  );
}
