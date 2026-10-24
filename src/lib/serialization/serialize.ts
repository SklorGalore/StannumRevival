import type { DiagramStore, SLDDiagram } from "../../store/types";

export function serialize(store: Pick<DiagramStore, "nodes" | "edges" | "viewport" | "metadata">): string {
  const diagram: SLDDiagram = {
    metadata: {
      ...store.metadata,
      modifiedAt: new Date().toISOString(),
    },
    nodes: store.nodes,
    edges: store.edges,
    viewport: store.viewport,
  };
  return JSON.stringify(diagram, null, 2);
}
