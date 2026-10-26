// Actions that reach back into the open case from a generated drawing:
// bring in more buses, re-centre on a bus, change how many hops are drawn.
import { useCallback, useEffect, useState } from "react";
import { useReactFlow } from "@xyflow/react";
import { Crosshair, Minus, Plus, Network } from "lucide-react";
import { useDiagramStore } from "../../store/diagramStore";
import { useWorkspace } from "../../store/workspaceStore";
import type { BusData, OffPageData, SLDNode } from "../../store/types";
import { caseNetwork } from "../../lib/diagram/network";
import { busLabel, generateDiagram } from "../../lib/diagram/generate";
import { drawnBuses, expandDrawing, undrawnNeighbours } from "../../lib/diagram/expand";
import { elementLabel, parseElement } from "../../lib/elements";

/** Case bus number of a symbol, for buses and off-page markers' inside bus. */
function insideBus(node: SLDNode, nodes: SLDNode[], edges: { source: string; target: string }[]): number | null {
  if (node.type === "bus") {
    const ref = (node.data as BusData).caseRef;
    return ref?.kind === "bus" ? ref.id : null;
  }
  if (node.type === "offPage") {
    const wire = edges.find((e) => e.target === node.id || e.source === node.id);
    const other = wire && nodes.find((n) => n.id === (wire.source === node.id ? wire.target : wire.source));
    return other ? insideBus(other, nodes, edges) : null;
  }
  return null;
}

export function useCaseActions() {
  const { fitView, getZoom } = useReactFlow();
  const caseName = useDiagramStore((s) => s.metadata.caseName);
  const openName = useWorkspace((s) => s.caseSummary?.name);
  const linked = !!caseName && caseName === openName;
  const [busy, setBusy] = useState(false);

  const run = useCallback(async (work: () => Promise<void>) => {
    setBusy(true);
    try {
      await work();
    } catch (e) {
      window.alert((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, []);

  /** Add `add` (all undrawn neighbours when omitted) around case bus `anchor`. */
  const expand = useCallback(
    (anchor: number, add?: number[]) =>
      run(async () => {
        const net = await caseNetwork();
        const store = useDiagramStore.getState();
        const buses = add ?? undrawnNeighbours(net, store.nodes, anchor);
        const out = await expandDrawing(net, store.nodes, store.edges, anchor, buses);
        if (!out) return;
        store.checkpoint();
        store.setNodes(out.nodes);
        const anchorId = drawnBuses(out.nodes).get(anchor)?.id;
        const show = out.nodes.filter((n) => n.selected || n.id === anchorId).map((n) => ({ id: n.id }));
        // Wires go in once the buses have measured their new taps; then bring
        // the new part into view without zooming in past where you were.
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            useDiagramStore.getState().setEdges(out.edges);
            void fitView({ nodes: show, padding: 0.25, maxZoom: getZoom(), duration: 250 });
          })
        );
      }),
    [run, fitView, getZoom]
  );

  /** Redraw the drawing as `depth` hops around case bus `bus`, or around
   *  both ends of `element` when it is a branch. */
  const drawAround = useCallback(
    (bus: number, depth: number, element?: string) =>
      run(async () => {
        const net = await caseNetwork();
        const out = await generateDiagram(net, { kind: "around", bus, depth, element });
        useDiagramStore.getState().replaceWithGenerated(out.nodes, out.edges, caseName!, { bus, depth, element });
      }),
    [run, caseName]
  );

  return { linked, busy, expand, drawAround };
}

type Menu = { x: number; y: number; node: SLDNode } | null;

/** Right-click menu for buses and off-page markers on a drawing linked to the open case. */
export function NodeMenu({ menu, onClose }: { menu: Menu; onClose: () => void }) {
  const { linked, expand, drawAround } = useCaseActions();
  const depth = useDiagramStore((s) => s.metadata.around?.depth ?? 2);
  const [neighbours, setNeighbours] = useState<number | null>(null);
  const [names, setNames] = useState<Map<number, string>>(new Map());

  const node = menu?.node;
  const nodes = useDiagramStore.getState().nodes;
  const edges = useDiagramStore.getState().edges;
  const bus = node ? insideBus(node, nodes, edges) : null;
  // Far buses of an off-page marker: one, or several for a bundled marker.
  const offPage = node?.type === "offPage" ? (node.data as OffPageData) : null;
  const farBuses = offPage ? [...new Set((offPage.links ?? [offPage]).map((l) => l.bus))] : [];
  const far = farBuses.length === 1 ? farBuses[0] : null;

  useEffect(() => {
    setNeighbours(null);
    if (!linked || bus == null) return;
    let live = true;
    caseNetwork().then((net) => {
      if (!live) return;
      setNeighbours(undrawnNeighbours(net, useDiagramStore.getState().nodes, bus).length);
      setNames(new Map(net.buses.map((b) => [b.id, busLabel(b)])));
    });
    return () => {
      live = false;
    };
  }, [linked, bus, node?.id]);

  useEffect(() => {
    if (!menu) return;
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent && e.key !== "Escape") return;
      onClose();
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", close);
    window.addEventListener("wheel", close);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", close);
      window.removeEventListener("wheel", close);
    };
  }, [menu, onClose]);

  if (!menu || !linked || bus == null) return null;
  const item = (label: string, onClick: () => void, disabled = false, icon?: React.ReactNode) => (
    <button
      role="menuitem"
      disabled={disabled}
      // Act on pointer down: the window listener closes the menu on the same event.
      onPointerDown={(e) => {
        e.stopPropagation();
        if (disabled) return;
        onClose();
        onClick();
      }}
      className="w-full flex items-center gap-2 text-left px-3 py-1.5 text-xs text-slate-700 hover:bg-slate-100 disabled:opacity-40 disabled:hover:bg-transparent"
    >
      {icon}
      {label}
    </button>
  );
  const farName = far != null ? (names.get(far) ?? `Bus ${far}`) : "";
  return (
    <div
      role="menu"
      className="fixed z-50 min-w-52 py-1 rounded-md border border-slate-200 bg-white shadow-lg"
      style={{ left: menu.x, top: menu.y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {far != null && item(`Bring in ${farName}`, () => expand(bus, [far]), false, <Plus size={13} />)}
      {farBuses.length > 1 && (
        <>
          {item(`Bring in all ${farBuses.length} buses`, () => expand(bus, farBuses), false, <Plus size={13} />)}
          {farBuses.slice(0, 8).map((b) => (
            <div key={b}>{item(`Bring in ${names.get(b) ?? `Bus ${b}`}`, () => expand(bus, [b]), false, <span className="w-[13px]" />)}</div>
          ))}
          {farBuses.length > 8 && <div className="px-3 py-1 text-[11px] text-slate-400">and {farBuses.length - 8} more</div>}
          <div className="my-1 border-t border-slate-100" />
        </>
      )}
      {node?.type === "bus" &&
        item(
          neighbours === 0
            ? "All neighbours are drawn"
            : `Show neighbours${neighbours ? ` (${neighbours})` : ""}`,
          () => expand(bus),
          !neighbours,
          <Network size={13} />
        )}
      {item(
        `Draw ${depth} hop${depth === 1 ? "" : "s"} around ${far != null ? farName : (names.get(bus) ?? `Bus ${bus}`)}`,
        () => drawAround(far ?? bus, depth),
        false,
        <Crosshair size={13} />
      )}
    </div>
  );
}

/** Shows the studied bus of a local drawing and lets you change the hop count. */
export function LocalScopeBar() {
  const around = useDiagramStore((s) => s.metadata.around);
  const { linked, busy, drawAround } = useCaseActions();
  const focusName = useDiagramStore((s) => {
    const n = around ? drawnBuses(s.nodes).get(around.bus) : undefined;
    return n ? (n.data as BusData).label : undefined;
  });
  if (!around || !linked) return null;
  const step = (d: number) => drawAround(around.bus, Math.min(6, Math.max(1, around.depth + d)), around.element);
  // Drawn around a branch or a machine: name it rather than its bus.
  const kind = around.element ? parseElement(around.element)?.kind : "bus";
  const name = kind && kind !== "bus" && kind !== "load" ? elementLabel(around.element!) : (focusName ?? `Bus ${around.bus}`);
  const btn = "p-1 rounded text-slate-600 hover:bg-slate-100 disabled:opacity-30 disabled:hover:bg-transparent";
  return (
    <div className="absolute bottom-3 left-1/2 -translate-x-1/2 z-10 flex items-center gap-1 px-2 py-1 rounded-lg border border-slate-200 bg-white shadow-sm text-[11px] text-slate-600">
      <span className="px-1">
        Around <span className="font-semibold text-slate-800">{name}</span>
      </span>
      <span className="w-px h-4 bg-slate-200 mx-1" />
      <button className={btn} title="One hop fewer" aria-label="One hop fewer" disabled={busy || around.depth <= 1} onClick={() => step(-1)}>
        <Minus size={13} />
      </button>
      <span className="tabular-nums w-12 text-center" aria-label="Hops">
        {busy ? "…" : `${around.depth} hop${around.depth === 1 ? "" : "s"}`}
      </span>
      <button className={btn} title="One hop more" aria-label="One hop more" disabled={busy || around.depth >= 6} onClick={() => step(1)}>
        <Plus size={13} />
      </button>
    </div>
  );
}
