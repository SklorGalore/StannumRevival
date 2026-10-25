import { useEffect, useRef } from "react";
import { Handle, Position, useReactFlow, useUpdateNodeInternals, type NodeProps } from "@xyflow/react";
import type { BusNode as BusNodeType } from "../../store/types";
import { useResults } from "../../lib/diagram/results";
import { useInk, SELECTED } from "../../lib/diagram/ink";
import { useDiagramStore } from "../../store/diagramStore";
import { busTaps } from "./shared/busTaps";
import { handleStyle } from "./shared/SymbolFrame";
import { Leader, useBBox, useLabelDrag } from "./shared/labels";
import { BAR, isVertical, tapFacing } from "../../lib/diagram/bus";

const POSITION = { top: Position.Top, right: Position.Right, bottom: Position.Bottom, left: Position.Left };
const MIN_LENGTH = 40;

/** Grip on the far end of a selected bus (right, or bottom when vertical):
 *  drag to lengthen or shorten it. */
function ResizeGrip({ id, length, minLength, vertical }: { id: string; length: number; minLength: number; vertical: boolean }) {
  const { getZoom } = useReactFlow();
  const updateInternals = useUpdateNodeInternals();
  const start = useRef<{ x: number; length: number } | null>(null);

  return (
    <div
      className={`bus-grip nodrag nopan${vertical ? " vertical" : ""}`}
      title="Drag to resize the bus"
      style={vertical ? { top: length + 6 } : { left: length + 6 }}
      onPointerDown={(e) => {
        e.stopPropagation();
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        start.current = { x: vertical ? e.clientY : e.clientX, length };
        useDiagramStore.getState().checkpoint();
      }}
      onPointerMove={(e) => {
        if (!start.current) return;
        const { gridSize } = useDiagramStore.getState();
        const raw = start.current.length + ((vertical ? e.clientY : e.clientX) - start.current.x) / getZoom();
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
  const length = data.length ?? 200;
  const vertical = isVertical(data);
  const ink = useInk();
  const color = selected ? SELECTED : ink.voltage(data.voltageKV);
  const results = useResults();
  const row = data.caseRef ? results?.buses.get(data.caseRef.id) : undefined;
  const taps = busTaps(data);
  // Re-measure when taps come or go (expand, undo, paste) or the bus turns,
  // so wires can attach.
  const updateInternals = useUpdateNodeInternals();
  const tapKey = `${vertical}` + taps.map((t) => `${t.id}${t.side}${t.x}`).join();
  useEffect(() => updateInternals(id), [tapKey, id, updateInternals]);
  // Fixed taps (from a generated layout or added by hand) must stay on the bar.
  const minLength = Math.max(MIN_LENGTH, ...(data.taps ?? []).map((t) => t.x + 20));
  const width = vertical ? BAR : length;
  const off = data.labelOffset ?? { x: 0, y: 0 };
  const drag = useLabelDrag(data.labelOffset, (next) => useDiagramStore.getState().updateNodeData(id, { labelOffset: next }));
  const [nameRef, nameBox] = useBBox<SVGGElement>([off.x, off.y, vertical, data.label, data.voltageKV]);
  const height = vertical ? length : BAR;

  const tapHandles = taps.map((t) => {
    const facing = tapFacing(vertical, t.side);
    return (
      <Handle
        key={t.id}
        type="source"
        id={t.id}
        position={POSITION[facing]}
        className="bus-tap"
        style={vertical ? handleStyle(facing === "right" ? BAR : 0, t.x, facing) : { left: t.x }}
      />
    );
  });

  if (data.star) {
    // Star point of a three-winding transformer.
    return (
      <div className="node-wrapper relative" style={{ width: length, height: BAR }}>
        <svg width={length} height={BAR} overflow="visible" style={{ display: "block" }}>
          <circle cx={length / 2} cy={BAR / 2} r={4} fill={selected ? SELECTED : ink.line} />
        </svg>
        {tapHandles}
      </div>
    );
  }

  const name = (
    <g ref={nameRef} transform={`translate(${off.x},${off.y})`} {...drag}>
      <title>Drag to move the label</title>
      <text
        x={vertical ? BAR / 2 : -8}
        y={vertical ? -20 : 0}
        textAnchor={vertical ? "middle" : "end"}
        fontSize={11}
        fontWeight={600}
        fill={ink.line}
      >
        {data.label}
      </text>
      <text
        x={vertical ? BAR / 2 : -8}
        y={vertical ? -8 : 13}
        textAnchor={vertical ? "middle" : "end"}
        fontSize={9.5}
        fill={ink.muted}
      >
        {data.caseRef ? `${data.caseRef.id} · ` : ""}
        {data.voltageKV} kV
      </text>
    </g>
  );

  return (
    <div className="node-wrapper relative" style={{ width, height }}>
      <svg width={width} height={height} overflow="visible" style={{ display: "block" }}>
        {data.focus && (
          // The studied bus of a local drawing.
          <rect
            x={-6}
            y={-6}
            width={width + 12}
            height={height + 12}
            rx={4}
            fill={ink.mono ? "none" : "#fef3c7"}
            stroke={ink.mono ? ink.line : "#f59e0b"}
            strokeWidth={1}
            strokeDasharray={ink.mono ? "4 3" : undefined}
          >
            <title>Studied bus</title>
          </rect>
        )}
        <rect x={0} y={0} width={width} height={height} fill={color} rx={1} />
        {data.labelOffset && nameBox && (
          <Leader
            from={{ x: 0, y: 0, w: width, h: height }}
            to={{ x: nameBox.x + off.x, y: nameBox.y + off.y, w: nameBox.w, h: nameBox.h }}
            color={ink.muted}
          />
        )}
        {name}
        {row && (
          <text
            x={vertical ? BAR / 2 : width + 8}
            y={vertical ? height + 16 : BAR / 2 + 3.5}
            textAnchor={vertical ? "middle" : undefined}
            fontSize={9.5}
            fill={row.violation > 0 ? ink.alert("#dc2626") : ink.result}
            fontWeight={row.violation > 0 ? 600 : 400}
          >
            {row.vm.toFixed(3)} pu {row.va.toFixed(1)}°
          </text>
        )}
      </svg>

      {tapHandles}
      {/* End handles: left and right, or top and bottom of a vertical bus. */}
      {vertical ? (
        <>
          <Handle type="source" position={Position.Top} id="bus-left" className="bus-tap" style={{ left: BAR / 2 }} />
          <Handle type="source" position={Position.Bottom} id="bus-right" className="bus-tap" style={{ left: BAR / 2 }} />
        </>
      ) : (
        <>
          <Handle type="source" position={Position.Left} id="bus-left" className="bus-tap" style={{ top: BAR / 2 }} />
          <Handle type="source" position={Position.Right} id="bus-right" className="bus-tap" style={{ top: BAR / 2 }} />
        </>
      )}
      {selected && <ResizeGrip id={id} length={length} minLength={minLength} vertical={vertical} />}
    </div>
  );
}
