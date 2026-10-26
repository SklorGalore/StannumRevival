import { useEffect, useRef, useState } from "react";
import { useReactFlow } from "@xyflow/react";
import { useWorkspace } from "../../store/workspaceStore";
import { useDiagramStore } from "../../store/diagramStore";
import { busElement, elementBus } from "../../lib/elements";
import type { CaseRef, OffPageData, SLDEdge, SLDNode } from "../../store/types";

const refOf = (x: SLDNode | SLDEdge | undefined) => (x?.data as { caseRef?: CaseRef } | undefined)?.caseRef;

/** Publish a clicked case-linked symbol as the shared selection. */
export function publishDiagramSelection(item: SLDNode | SLDEdge) {
  const element = refOf(item)?.element;
  if (element) useWorkspace.getState().select(element, "diagram");
}

/**
 * Follow a selection made in another module: select the matching symbol and
 * centre it. Returns the element when this drawing has no symbol for it.
 */
export function useSharedSelection(): string | null {
  const shared = useWorkspace((s) => s.selection);
  // "Draw around this" from another module: select the element once its drawing is in.
  const request = useWorkspace((s) => s.drawRequest);
  // A drawing opened or generated may now hold the selected element.
  const fitRequest = useDiagramStore((s) => s.fitRequest);
  const lastShared = useRef(shared);
  const lastRequest = useRef(request);
  const source = useRef<"shared" | "request">("shared");
  const { getZoom, setCenter } = useReactFlow();
  const [missing, setMissing] = useState<string | null>(null);

  useEffect(() => {
    // True when the selection changed; false when only the drawing did.
    const moved = lastShared.current !== shared;
    const requested = lastRequest.current !== request;
    lastShared.current = shared;
    lastRequest.current = request;
    if (moved) source.current = "shared";
    if (requested) {
      source.current = "request";
      // The new drawing is still being laid out; wait for it to arrive.
      setMissing(null);
      return;
    }
    setMissing(null);
    const { nodes, edges, selectOnly } = useDiagramStore.getState();
    const target =
      source.current === "request" ? (request?.element ?? null) : shared && shared.from !== "diagram" ? shared.element : null;
    if (!target) return;
    const byElement = (id: string) => ({
      // A bundled off-page marker stands for each of its branches.
      nodes: nodes.filter(
        (n) => refOf(n)?.element === id || (n.data as OffPageData).links?.some((l) => l.caseRef?.element === id)
      ),
      edges: edges.filter((e) => refOf(e)?.element === id),
    });
    let hit = byElement(target);
    // Loads are drawn on their bus; fall back to the bus for generators too.
    const bus = elementBus(target);
    if (!hit.nodes.length && !hit.edges.length && bus !== null) hit = byElement(busElement(bus));
    if (!hit.nodes.length && !hit.edges.length) {
      selectOnly([], []);
      if (source.current === "shared") setMissing(target);
      return;
    }
    const nodeIds = hit.nodes.map((n) => n.id);
    const edgeIds = hit.edges.map((e) => e.id);
    selectOnly(nodeIds, edgeIds);

    // Centre on the symbol, or on the middle of a wire, keeping the zoom.
    const centre = (n: SLDNode) => ({
      x: n.position.x + (n.measured?.width ?? n.width ?? 0) / 2,
      y: n.position.y + (n.measured?.height ?? n.height ?? 0) / 2,
    });
    let point: { x: number; y: number } | null = null;
    if (hit.nodes[0]) point = centre(hit.nodes[0]);
    else {
      const route = (hit.edges[0].data as { route?: { x: number; y: number }[] } | undefined)?.route;
      if (route && route.length > 1) point = halfway(route);
      else {
        const ends = [hit.edges[0].source, hit.edges[0].target]
          .map((id) => nodes.find((n) => n.id === id))
          .filter((n): n is SLDNode => !!n)
          .map(centre);
        if (ends.length) point = halfway(ends);
      }
    }
    // A new drawing is first fitted to the screen, so centre after that.
    if (!point) return;
    const { x, y } = point;
    const timer = window.setTimeout(() => void setCenter(x, y, { zoom: Math.max(getZoom(), 0.8), duration: 250 }), moved ? 0 : 400);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shared, request, fitRequest, getZoom, setCenter]);

  return missing;
}

/** The point half way along a polyline, so a routed wire is centred on the wire itself. */
function halfway(points: { x: number; y: number }[]): { x: number; y: number } {
  const lengths = points.slice(1).map((p, i) => Math.hypot(p.x - points[i].x, p.y - points[i].y));
  let left = lengths.reduce((a, b) => a + b, 0) / 2;
  for (let i = 0; i < lengths.length; i++) {
    if (left <= lengths[i] && lengths[i] > 0) {
      const t = left / lengths[i];
      return { x: points[i].x + t * (points[i + 1].x - points[i].x), y: points[i].y + t * (points[i + 1].y - points[i].y) };
    }
    left -= lengths[i];
  }
  return points[points.length - 1];
}
