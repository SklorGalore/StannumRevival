import type { SLDDiagram } from "../../store/types";

export function deserialize(json: string): SLDDiagram {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    throw new Error("Invalid diagram file: not valid JSON");
  }

  const d = parsed as SLDDiagram;
  if (!d.metadata || !Array.isArray(d.nodes) || !Array.isArray(d.edges)) {
    throw new Error("Invalid diagram file: missing required fields");
  }

  return d;
}
