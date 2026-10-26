import { beforeEach, describe, expect, it } from "vitest";
import { useDiagramStore } from "./diagramStore";
import type { SLDEdge, SLDNode } from "./types";

const bus = (id: string, x: number, selected = false) =>
  ({ id, type: "bus", position: { x, y: 0 }, selected, data: { label: id, voltageKV: 138, length: 200 } }) as unknown as SLDNode;
const wire = { id: "w", type: "conductor", source: "a", target: "b", data: { label: "w" } } as unknown as SLDEdge;

describe("a locked drawing", () => {
  beforeEach(() => {
    const s = useDiagramStore.getState();
    s.newDiagram();
    s.setLocked(false);
    s.setNodes([bus("a", 0, true), bus("b", 300, true)]);
    s.setEdges([wire]);
    s.checkpoint();
    s.setLocked(true);
  });

  it("can still be selected", () => {
    const s = useDiagramStore.getState();
    s.selectOnly(["b"], []);
    s.setSelection(["b"], []);
    const now = useDiagramStore.getState();
    expect(now.nodes.find((n) => n.id === "b")?.selected).toBe(true);
    expect(now.nodes.find((n) => n.id === "a")?.selected).toBe(false);
    expect(now.selectedNodeIds).toEqual(["b"]);
    s.selectAll();
    expect(useDiagramStore.getState().nodes.every((n) => n.selected)).toBe(true);
  });

  it("refuses moves, edits, deletes, pastes and undo", () => {
    const before = useDiagramStore.getState();
    const { nodes, edges } = before;
    before.nudge(20, 0);
    before.rotateSelection(90);
    before.align("top");
    before.updateNodeData("a", { label: "renamed" });
    before.updateEdgeData("w", { label: "renamed" });
    before.addNode(bus("c", 600));
    before.copySelection();
    before.paste();
    before.duplicateSelection();
    before.deleteSelected();
    before.undo();
    const after = useDiagramStore.getState();
    expect(after.nodes).toBe(nodes);
    expect(after.edges).toBe(edges);
    expect(after.past.length).toBe(before.past.length);
  });

  it("takes edits again once unlocked", () => {
    const s = useDiagramStore.getState();
    s.setLocked(false);
    s.updateNodeData("a", { label: "renamed" });
    s.nudge(20, 0);
    const after = useDiagramStore.getState();
    expect((after.nodes[0].data as { label: string }).label).toBe("renamed");
    expect(after.nodes[0].position.x).toBe(20);
  });
});
