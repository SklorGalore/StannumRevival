// Switching the Diagram between its One-line and Network styles, keeping the
// bus drawn around, the hop count and the selected element.
import { useDiagramStore } from "../../../store/diagramStore";
import { useWorkspace } from "../../../store/workspaceStore";
import { useDrawingStyle, type Focus } from "../../../store/drawingStyleStore";
import { busElement, elementBus } from "../../../lib/elements";
import { caseNetwork } from "../../../lib/diagram/network";
import { generateDiagram } from "../../../lib/diagram/generate";
import { useLog } from "../../../store/logStore";
import type { CaseRef, SLDEdge, SLDNode } from "../../../store/types";

const refOf = (x: SLDNode | SLDEdge) => (x.data as { caseRef?: CaseRef } | undefined)?.caseRef;

/** The one-line symbols for an element: its own, or its bus's for a generator or load drawn on the bus. */
export function symbolsFor(element: string): { nodeIds: string[]; edgeIds: string[] } {
  const { nodes, edges } = useDiagramStore.getState();
  const find = (id: string) => {
    const nodeIds = nodes.filter((n) => refOf(n)?.element === id).map((n) => n.id);
    // A bus's load and shunt symbols share its id; the bus bar itself is the pick.
    const bars = nodeIds.filter((nid) => nodes.find((n) => n.id === nid)?.type === "bus");
    if (bars.length) return { nodeIds: bars, edgeIds: [] };
    return { nodeIds, edgeIds: edges.filter((e) => refOf(e)?.element === id).map((e) => e.id) };
  };
  const own = find(element);
  if (own.nodeIds.length || own.edgeIds.length) return own;
  const bus = elementBus(element);
  return bus !== null && element !== busElement(bus) ? find(busElement(bus)) : own;
}

/** The element of the selected one-line symbol, if one case-linked symbol or wire is selected. */
function oneLineElement(): string | null {
  const { nodes, edges, selectedNodeIds, selectedEdgeIds } = useDiagramStore.getState();
  const picked = [
    ...nodes.filter((n) => selectedNodeIds.includes(n.id)),
    ...edges.filter((e) => selectedEdgeIds.includes(e.id)),
  ]
    .map((x) => refOf(x)?.element)
    .filter((x): x is string => !!x);
  return new Set(picked).size === 1 ? picked[0] : null;
}

/** True when the one-line was generated from the case that is open now. */
function oneLineLinked(): boolean {
  const caseName = useDiagramStore.getState().metadata.caseName;
  return !!caseName && caseName === useWorkspace.getState().caseSummary?.name;
}

/** Show the Network style, drawn around the one-line's bus with its hop count. */
export function showNetwork() {
  const style = useDrawingStyle.getState();
  const picked = oneLineElement();
  if (picked) style.setElement(picked);
  // Nothing picked in the one-line: keep the network's selection unless the
  // one-line draws it, in which case it was deselected there.
  else if (style.element) {
    const drawn = symbolsFor(style.element);
    if (drawn.nodeIds.length || drawn.edgeIds.length) style.setElement(null);
  }
  const around = useDiagramStore.getState().metadata.around;
  if (around && oneLineLinked()) style.setFocus(around);
  style.setStyle("network");
}

/**
 * Show the One-line style. If the network's focus or hop count has moved
 * since the one-line was drawn, redraw it (undoably) around the same bus.
 * A hand-drawn diagram is never replaced.
 */
export async function showOneLine(): Promise<void> {
  const style = useDrawingStyle.getState();
  style.setStyle("oneline");
  const focus = style.focus;
  const diagram = useDiagramStore.getState();
  const around = diagram.metadata.around;
  const summary = useWorkspace.getState().caseSummary;
  const redraw =
    !!focus &&
    !!summary?.power.available &&
    (diagram.nodes.length === 0 || oneLineLinked()) &&
    (!around || around.bus !== focus.bus || around.depth !== focus.depth);
  if (redraw) await drawOneLine(focus!, summary!.name);
  const element = useDrawingStyle.getState().element;
  if (element) {
    const { nodeIds, edgeIds } = symbolsFor(element);
    useDiagramStore.getState().selectOnly(nodeIds, edgeIds);
    // Bring it into view (after a new drawing has been fitted to the screen).
    if (nodeIds.length || edgeIds.length) useDrawingStyle.setState((s) => ({ reveal: { n: (s.reveal?.n ?? 0) + 1, redrawn: redraw } }));
  }
}

/** Generate the one-line around a focus, keeping the style's busy flag for the switch. */
async function drawOneLine(focus: Focus, caseName: string) {
  const version = useWorkspace.getState().caseVersion;
  useDrawingStyle.setState({ busy: true, error: null });
  try {
    const net = await caseNetwork();
    if (!net.buses.some((b) => b.id === focus.bus && !b.star)) throw new Error(`Bus ${focus.bus} isn't in the case network.`);
    const out = await generateDiagram(net, { kind: "around", bus: focus.bus, depth: focus.depth });
    if (version !== useWorkspace.getState().caseVersion) return;
    useDiagramStore.getState().replaceWithGenerated(out.nodes, out.edges, caseName, focus);
  } catch (e) {
    const error = `Couldn't draw the one-line around bus ${focus.bus}: ${(e as Error).message}`;
    useLog.getState().add("error", error);
    useDrawingStyle.setState({ error });
  } finally {
    useDrawingStyle.setState({ busy: false });
  }
}
