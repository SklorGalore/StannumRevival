import { ConductorEdge } from "./ConductorEdge";

// Module-level constant — NEVER move inside a component
export const edgeTypes = {
  conductor: ConductorEdge,
} as const;

export { ConductorEdge };
