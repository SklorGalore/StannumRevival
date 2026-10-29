import type { SLDDiagram } from "../../store/types";
import { DIAGRAM_SCHEMA_VERSION } from "./serialize";

/** The major part of a diagram schema version: "1.0" → 1. */
function major(version: string): number {
  return Number.parseInt(version.split(".")[0] ?? "", 10);
}

/**
 * Check a diagram read from a file (an .sld, or the one inside a study) and
 * fill in what older files leave out. A diagram from a newer major schema
 * version is refused; a newer minor version only adds fields, which are kept.
 */
export function checkDiagram(value: unknown): SLDDiagram {
  const d = value as Partial<SLDDiagram> | null;
  if (!d || typeof d !== "object" || !d.metadata || !Array.isArray(d.nodes) || !Array.isArray(d.edges)) {
    throw new Error("Invalid diagram file: missing required fields");
  }
  const version = typeof d.metadata.schemaVersion === "string" ? d.metadata.schemaVersion : DIAGRAM_SCHEMA_VERSION;
  const m = major(version);
  if (!Number.isFinite(m)) throw new Error(`Invalid diagram file: schema version "${version}" isn't a version number`);
  if (m > major(DIAGRAM_SCHEMA_VERSION)) {
    throw new Error(
      `This diagram was saved by a newer Stannum (diagram schema ${version}). This Stannum reads schema ${DIAGRAM_SCHEMA_VERSION}; update Stannum to open it.`
    );
  }
  const viewport = d.viewport && typeof d.viewport.zoom === "number" ? d.viewport : { x: 0, y: 0, zoom: 1 };
  return { ...d, metadata: { ...d.metadata }, viewport } as SLDDiagram;
}

export function deserialize(json: string): SLDDiagram {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    throw new Error("Invalid diagram file: not valid JSON");
  }
  return checkDiagram(parsed);
}
