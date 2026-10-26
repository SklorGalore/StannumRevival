import { beforeEach, describe, expect, it } from "vitest";
import { useDiagramStore } from "../../../store/diagramStore";
import { useWorkspace } from "../../../store/workspaceStore";
import { useDrawingStyle } from "../../../store/drawingStyleStore";
import type { CaseSummary } from "../../../lib/analysis/types";
import type { SLDEdge, SLDNode } from "../../../store/types";
import { showNetwork, showOneLine, symbolsFor } from "./switchStyle";

const node = (id: string, type: string, element: string): SLDNode =>
  ({ id, type, position: { x: 0, y: 0 }, data: { caseRef: { kind: type, id: 0, element } } }) as unknown as SLDNode;
const wire = (id: string, element: string): SLDEdge =>
  ({ id, source: "b4", target: "b5", data: { caseRef: { kind: "branch", id: 0, element } } }) as unknown as SLDEdge;

// Buses 4 and 5 with a load on bus 4 (loads share their bus's id) and the line between them.
const drawing = {
  nodes: [node("b4", "bus", "bus:4"), node("l4", "load", "bus:4"), node("b5", "bus", "bus:5"), node("g5", "generator", "generator:5:1")],
  edges: [wire("w45", "line:4:5:1")],
};

describe("switching drawing styles", () => {
  beforeEach(() => {
    useWorkspace.setState({ caseSummary: { name: "five.raw", power: { available: true } } as unknown as CaseSummary, caseVersion: 1 });
    useDiagramStore.setState((s) => {
      s.nodes = drawing.nodes as typeof s.nodes;
      s.edges = drawing.edges as typeof s.edges;
      s.selectedNodeIds = [];
      s.selectedEdgeIds = [];
      s.metadata = { ...s.metadata, caseName: "five.raw", around: { bus: 4, depth: 1 } };
    });
    useDrawingStyle.setState({ style: "oneline", focus: null, element: null, busy: false, error: null, reveal: null });
  });

  it("picks a bus's bar rather than its load", () => {
    expect(symbolsFor("bus:4")).toEqual({ nodeIds: ["b4"], edgeIds: [] });
    expect(symbolsFor("line:4:5:1")).toEqual({ nodeIds: [], edgeIds: ["w45"] });
    expect(symbolsFor("generator:5:1")).toEqual({ nodeIds: ["g5"], edgeIds: [] });
    // A load without its own symbol is found on its bus.
    expect(symbolsFor("load:5:1")).toEqual({ nodeIds: ["b5"], edgeIds: [] });
  });

  it("takes the one-line's focus, hops and selection to the network", () => {
    useDiagramStore.setState({ selectedEdgeIds: ["w45"] });
    showNetwork();
    expect(useDrawingStyle.getState()).toMatchObject({ style: "network", focus: { bus: 4, depth: 1 }, element: "line:4:5:1" });
  });

  it("keeps a network selection the one-line doesn't draw, and drops one deselected there", () => {
    useDrawingStyle.setState({ element: "bus:9" });
    showNetwork();
    expect(useDrawingStyle.getState().element).toBe("bus:9");
    useDrawingStyle.setState({ style: "oneline", element: "bus:5" });
    showNetwork();
    expect(useDrawingStyle.getState().element).toBeNull();
  });

  it("leaves a hand drawing's focus alone", () => {
    useDiagramStore.setState((s) => {
      s.metadata = { ...s.metadata, caseName: "other.raw" };
    });
    useDrawingStyle.setState({ focus: { bus: 2, depth: 3 } });
    showNetwork();
    expect(useDrawingStyle.getState().focus).toEqual({ bus: 2, depth: 3 });
  });

  it("selects the network's pick in the one-line when it needn't be redrawn", async () => {
    useDrawingStyle.setState({ style: "network", focus: { bus: 4, depth: 1 }, element: "bus:5" });
    await showOneLine();
    expect(useDrawingStyle.getState().style).toBe("oneline");
    expect(useDiagramStore.getState().selectedNodeIds).toEqual(["b5"]);
    expect(useDrawingStyle.getState().reveal).toMatchObject({ redrawn: false });
  });

  it("clamps the hop count", () => {
    useDrawingStyle.getState().setFocus({ bus: 1, depth: 9 });
    expect(useDrawingStyle.getState().focus).toEqual({ bus: 1, depth: 6 });
    useDrawingStyle.getState().setFocus({ bus: 1, depth: 0 });
    expect(useDrawingStyle.getState().focus).toEqual({ bus: 1, depth: 1 });
  });
});
