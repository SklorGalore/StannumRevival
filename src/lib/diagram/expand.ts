// Grow a generated drawing: bring more of the case in around a bus that is
// already drawn, without moving anything that is there.
import { nanoid } from "nanoid";
import type { CaseNetwork } from "../analysis/types";
import type { BusData, BusTap, ConductorData, OffPageData, SLDEdge, SLDNode, TransformerData } from "../../store/types";
import { defaultTransformerData } from "../../store/defaults";
import { busTaps } from "../../components/nodes/shared/busTaps";
import { symbolSize } from "./sizes";
import { gridAlong, isVertical } from "./bus";
import { BUS_BAR, DC_LINK, dcLinkData, layoutPiece, linkRef, linksOf, offPageData, type Link } from "./generate";

type Box = { x: number; y: number; w: number; h: number };

/** Room kept right of a symbol for its name and results. */
const LABEL_ROOM = 100;
const GAP = 20;
const STEP = 40;


function boxOf(n: SLDNode): Box {
  if (n.type === "bus") {
    const len = (n.data as BusData).length ?? 200;
    // A vertical bus has its name above and voltage below.
    if (isVertical(n.data as BusData))
      return { x: n.position.x - 40, y: n.position.y - 30, w: BUS_BAR + 80, h: len + 50 };
    // Bus names sit to the left, voltages to the right.
    return { x: n.position.x - 90, y: n.position.y - 12, w: len + 90 + 90, h: BUS_BAR + 24 };
  }
  const size = symbolSize(n);
  return { x: n.position.x, y: n.position.y, w: size.w + LABEL_ROOM, h: size.h };
}

const overlaps = (a: Box, b: Box) =>
  a.x < b.x + b.w + GAP && b.x < a.x + a.w + GAP && a.y < b.y + b.h + GAP && b.y < a.y + a.h + GAP;

/** Case bus number → its node on the drawing. */
export function drawnBuses(nodes: SLDNode[]): Map<number, SLDNode> {
  const out = new Map<number, SLDNode>();
  for (const n of nodes) {
    const ref = (n.data as BusData).caseRef;
    if (n.type === "bus" && ref?.kind === "bus") out.set(ref.id, n);
  }
  return out;
}

/** Buses next to `bus` in the case that aren't on the drawing yet. */
export function undrawnNeighbours(net: CaseNetwork, nodes: SLDNode[], bus: number): number[] {
  const drawn = drawnBuses(nodes);
  const out = new Set<number>();
  for (const br of linksOf(net)) {
    const other = br.from === bus ? br.to : br.to === bus ? br.from : null;
    if (other != null && !drawn.has(other)) out.add(other);
  }
  return [...out];
}

/**
 * Add `add` (buses next to `anchor`) to the drawing. The new buses are laid
 * out as a block hanging from the anchor bus, then moved down until they
 * clear what is already drawn. Off-page markers for branches that now have
 * both ends drawn are replaced by the branch itself.
 */
export async function expandDrawing(
  net: CaseNetwork,
  nodes: SLDNode[],
  edges: SLDEdge[],
  anchor: number,
  add: number[]
): Promise<{ nodes: SLDNode[]; edges: SLDEdge[]; added: number } | null> {
  const drawn = drawnBuses(nodes);
  const anchorNode = drawn.get(anchor);
  if (!anchorNode) return null;
  const star = new Set(net.buses.filter((b) => b.star).map((b) => b.id));
  const fresh = new Set(add.filter((b) => !drawn.has(b)));
  // A transformer's star point comes with the winding that reaches it.
  for (const br of net.branches)
    for (const [a, b] of [
      [br.from, br.to],
      [br.to, br.from],
    ])
      if (fresh.has(a) && star.has(b) && !drawn.has(b)) fresh.add(b);
  if (!fresh.size) return null;

  const drawnIds = new Set([...drawn.keys()].filter((b) => b !== anchor));
  const piece = await layoutPiece(net, { buses: fresh, anchor, root: anchor, byHop: true, drawn: drawnIds });

  // Line the anchor's stand-in up with the real anchor bus.
  const proxyId = `b${anchor}`;
  const proxy = piece.nodes.find((n) => n.id === proxyId)!;
  const dx = anchorNode.position.x - proxy.position.x;
  const dy = anchorNode.position.y - proxy.position.y;
  const block = piece.nodes.filter((n) => n.id !== proxyId);
  const existing = nodes.filter((n) => n.id !== anchorNode.id).map(boxOf);
  // Keep clear of laid-out wires and their results too.
  for (const e of edges) {
    const { route, labelAt } = (e.data ?? {}) as ConductorData;
    if (labelAt) existing.push({ x: labelAt.x, y: labelAt.y, w: labelAt.width, h: labelAt.height });
    route?.slice(1).forEach((p, i) => {
      const q = route[i];
      existing.push({ x: Math.min(p.x, q.x), y: Math.min(p.y, q.y), w: Math.abs(p.x - q.x), h: Math.abs(p.y - q.y) });
    });
  }
  // Push the block down until nothing overlaps; wires from the anchor stretch.
  let extra = 0;
  const clear = (shift: number) =>
    block.every((n) => {
      const b = boxOf({ ...n, position: { x: n.position.x + dx, y: n.position.y + dy + shift } } as SLDNode);
      return existing.every((e) => !overlaps(b, e));
    });
  while (!clear(extra) && extra < 200 * STEP) extra += STEP;

  const move = (p: { x: number; y: number }) => ({ x: p.x + dx, y: p.y + dy + extra });
  const newNodes = block.map((n) => ({ ...n, position: move(n.position), selected: true }) as SLDNode);

  // Taps the layout gave the stand-in move to the real anchor bus.
  const anchorData = anchorNode.data as BusData;
  const proxyTaps = (proxy.data as BusData).taps ?? [];
  const tapIds = new Map<string, string>();
  const addedTaps: BusTap[] = proxyTaps.map((t) => {
    const id = `x${nanoid(6)}`;
    tapIds.set(t.id, id);
    return { ...t, id };
  });

  const newEdges: SLDEdge[] = piece.edges.map((e) => {
    const data = { ...(e.data as ConductorData) };
    const fromAnchor = e.source === proxyId;
    const toAnchor = e.target === proxyId;
    if (data.route) {
      data.route = data.route.map((p, i) => {
        const end = (fromAnchor && i === 0) || (toAnchor && i === data.route!.length - 1);
        // The end on the anchor stays put; the rest moves with the block.
        return end ? { x: p.x + dx, y: p.y + dy } : move(p);
      });
    }
    if (data.labelAt) data.labelAt = { ...data.labelAt, ...move(data.labelAt) };
    return {
      ...e,
      source: fromAnchor ? anchorNode.id : e.source,
      target: toAnchor ? anchorNode.id : e.target,
      sourceHandle: fromAnchor ? tapIds.get(e.sourceHandle!) : e.sourceHandle,
      targetHandle: toAnchor ? tapIds.get(e.targetHandle!) : e.targetHandle,
      data,
    } as SLDEdge;
  });

  // Branches that now have both ends drawn lose their off-page markers.
  const nowDrawn = new Set([...drawn.keys(), ...fresh]);
  const joined = new Set(
    linksOf(net)
      .filter((br) => nowDrawn.has(br.from) && nowDrawn.has(br.to))
      .map((br) => `${br.dc ? "dc" : "br"}${br.dc?.index ?? br.index}`)
  );
  const markerKey = (ref: { kind: string; id: number } | undefined) =>
    ref?.kind === "branch" ? `br${ref.id}` : ref?.kind === "dcline" ? `dc${ref.id}` : "";
  const staleMarkers = new Set<string>();
  // A bundled marker keeps the branches still leaving the drawing.
  const trimmedMarkers = new Map<string, OffPageData>();
  for (const n of nodes) {
    if (n.type !== "offPage") continue;
    const data = n.data as OffPageData;
    const rest = (data.links ?? [data]).filter((l) => !joined.has(markerKey(l.caseRef)));
    if (!rest.length) staleMarkers.add(n.id);
    else if (data.links && rest.length < data.links.length)
      trimmedMarkers.set(n.id, { ...offPageData(rest, data.voltageKV), rotation: data.rotation, labelOffset: data.labelOffset });
  }

  const busNodes = new Map<number, SLDNode>(drawn);
  for (const n of newNodes) {
    const ref = (n.data as BusData).caseRef;
    if (n.type === "bus" && ref?.kind === "bus") busNodes.set(ref.id, n);
  }
  const extraTaps = new Map<string, BusTap[]>();
  extraTaps.set(anchorNode.id, addedTaps);
  const linkEdges: SLDEdge[] = [];
  const linkNodes: SLDNode[] = [];
  for (const br of piece.links) link(br);

  function tapOn(bus: SLDNode, toward: { x: number; y: number }): string {
    const x = gridAlong(bus, toward, 20);
    const side: BusTap["side"] = toward.y > bus.position.y ? "bottom" : "top";
    const id = `x${nanoid(6)}`;
    const list = extraTaps.get(bus.id) ?? [];
    list.push({ id, x, side });
    extraTaps.set(bus.id, list);
    return id;
  }

  // A branch from a new bus back to a bus drawn earlier: no layout route,
  // so it is drawn with a plain right-angle path.
  function link(br: Link) {
    const a = busNodes.get(br.from);
    const b = busNodes.get(br.to);
    if (!a || !b) return;
    const [upper, lower] = a.position.y <= b.position.y ? [a, b] : [b, a];
    const reversed = upper !== a || undefined;
    const kvOf = (n: SLDNode) => (n.data as BusData).voltageKV;
    const conductor = (kv: number, extra: Partial<ConductorData> = {}): ConductorData => ({
      voltageKV: kv,
      conductorType: kv >= 100 ? "transmission" : "distribution",
      outOfService: !br.online || undefined,
      ...extra,
    });
    if (br.dc) {
      const id = `dc${br.dc.index}`;
      const pos = {
        x: Math.round((upper.position.x + lower.position.x) / 2 / 20) * 20,
        y: Math.round(((upper.position.y + lower.position.y) / 2 - DC_LINK.h / 2) / 20) * 20,
      };
      const others = [...existing, ...newNodes.map(boxOf), ...linkNodes.map(boxOf)];
      const at = () => boxOf({ type: "dcLink", position: pos } as SLDNode);
      for (let i = 0; i < 50 && others.some((o) => overlaps(at(), o)); i++) pos.x += 40;
      const data = dcLinkData(br.dc, kvOf(upper), kvOf(lower), !!reversed);
      linkNodes.push({ id, type: "dcLink", position: pos, data, selected: true } as SLDNode);
      linkEdges.push(
        {
          id: `e-${id}-a`,
          type: "conductor",
          source: upper.id,
          sourceHandle: tapOn(upper, pos),
          target: id,
          targetHandle: "dc-top",
          data: conductor(kvOf(upper)),
        } as SLDEdge,
        {
          id: `e-${id}-b`,
          type: "conductor",
          source: id,
          sourceHandle: "dc-bottom",
          target: lower.id,
          targetHandle: tapOn(lower, { x: pos.x, y: pos.y + DC_LINK.h }),
          data: conductor(kvOf(lower)),
        } as SLDEdge
      );
    } else if (br.transformer) {
      const id = `t${br.index}`;
      const pos = {
        x: Math.round((upper.position.x + lower.position.x) / 2 / 20) * 20,
        y: Math.round(((upper.position.y + lower.position.y) / 2 - 50) / 20) * 20,
      };
      // Step aside until the symbol clears everything else on the drawing.
      const others = [...existing, ...newNodes.map(boxOf), ...linkNodes.map(boxOf)];
      const at = () => boxOf({ type: "transformer", position: pos } as SLDNode);
      for (let i = 0; i < 50 && others.some((o) => overlaps(at(), o)); i++) pos.x += 40;
      const data: TransformerData = {
        ...defaultTransformerData,
        label: `T ${(upper.data as BusData).caseRef?.id}-${(lower.data as BusData).caseRef?.id}`,
        ratingMVA: br.rate_a,
        primaryKV: kvOf(upper),
        secondaryKV: kvOf(lower),
        windingsUnknown: true,
        caseRef: linkRef(br),
        reversed,
        outOfService: !br.online || undefined,
      };
      linkNodes.push({ id, type: "transformer", position: pos, data, selected: true } as SLDNode);
      linkEdges.push(
        {
          id: `e-${id}-hv`,
          type: "conductor",
          source: upper.id,
          sourceHandle: tapOn(upper, pos),
          target: id,
          targetHandle: "xfmr-primary",
          data: conductor(kvOf(upper)),
        } as SLDEdge,
        {
          id: `e-${id}-lv`,
          type: "conductor",
          source: lower.id,
          sourceHandle: tapOn(lower, { x: pos.x, y: pos.y + 100 }),
          target: id,
          targetHandle: "xfmr-secondary",
          data: conductor(kvOf(lower)),
        } as SLDEdge
      );
    } else {
      linkEdges.push({
        id: `e-br${br.index}`,
        type: "conductor",
        source: upper.id,
        sourceHandle: tapOn(upper, lower.position),
        target: lower.id,
        targetHandle: tapOn(lower, upper.position),
        data: conductor(kvOf(upper), { caseRef: linkRef(br), reversed }),
      } as SLDEdge);
    }
  }

  const withTaps = (n: SLDNode): SLDNode => {
    const more = extraTaps.get(n.id);
    if (!more?.length) return n;
    const data = n.data as BusData;
    const taps = [...busTaps(data), ...more];
    // Lengthen the bus if a new tap landed past its end.
    const length = Math.max(data.length ?? 200, Math.ceil((Math.max(...taps.map((t) => t.x)) + 20) / 20) * 20);
    return { ...n, data: { ...data, taps, length } } as SLDNode;
  };

  const keptNodes = nodes
    .filter((n) => !staleMarkers.has(n.id))
    .map((n) => {
      const trimmed = trimmedMarkers.get(n.id);
      return withTaps({ ...n, ...(trimmed ? { data: trimmed } : {}), selected: false } as SLDNode);
    });
  const keptEdges = edges
    .filter((e) => !staleMarkers.has(e.source) && !staleMarkers.has(e.target))
    .map((e) => {
      // A bundle down to one branch: its wire carries that branch's flow again.
      const trimmed = trimmedMarkers.get(e.target) ?? trimmedMarkers.get(e.source);
      if (!trimmed) return { ...e, selected: false };
      const { caseRef, reversed, outOfService } = trimmed;
      return { ...e, selected: false, data: { ...(e.data as ConductorData), caseRef, reversed, outOfService } };
    });
  return {
    nodes: [...keptNodes, ...newNodes.map(withTaps), ...linkNodes],
    edges: [...keptEdges, ...newEdges, ...linkEdges],
    added: [...fresh].filter((b) => !star.has(b)).length,
  };
}
