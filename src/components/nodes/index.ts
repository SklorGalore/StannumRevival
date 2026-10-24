import { BusNode } from "./BusNode";
import { GeneratorNode } from "./GeneratorNode";
import { LoadNode } from "./LoadNode";
import { TransformerNode } from "./TransformerNode";
import { CircuitBreakerNode } from "./CircuitBreakerNode";
import { DisconnectorNode } from "./DisconnectorNode";
import { FuseNode } from "./FuseNode";

// Module-level constant — NEVER move inside a component or this breaks React Flow
export const nodeTypes = {
  bus: BusNode,
  generator: GeneratorNode,
  load: LoadNode,
  transformer: TransformerNode,
  circuitBreaker: CircuitBreakerNode,
  disconnector: DisconnectorNode,
  fuse: FuseNode,
} as const;

export {
  BusNode,
  GeneratorNode,
  LoadNode,
  TransformerNode,
  CircuitBreakerNode,
  DisconnectorNode,
  FuseNode,
};
