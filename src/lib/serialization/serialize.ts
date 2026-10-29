import type { DiagramStore, SLDDiagram } from "../../store/types";

/** The diagram schema version this Stannum writes and the newest it reads.
 *  The minor part goes up when fields are added that older versions can
 *  ignore; the major part when older versions couldn't read the file. */
export const DIAGRAM_SCHEMA_VERSION = "1.0";

export function serialize(store: Pick<DiagramStore, "nodes" | "edges" | "viewport" | "metadata">): string {
  const diagram: SLDDiagram = {
    metadata: {
      ...store.metadata,
      modifiedAt: new Date().toISOString(),
      // A diagram opened from an older file is saved as the current version.
      schemaVersion: DIAGRAM_SCHEMA_VERSION,
    },
    nodes: store.nodes,
    edges: store.edges,
    viewport: store.viewport,
  };
  return JSON.stringify(diagram, null, 2);
}
