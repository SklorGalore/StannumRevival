import { BusNode } from "./BusNode";
import { GeneratorNode } from "./GeneratorNode";
import { LoadNode } from "./LoadNode";
import { ShuntNode } from "./ShuntNode";
import { TransformerNode } from "./TransformerNode";
import { CircuitBreakerNode } from "./CircuitBreakerNode";
import { DisconnectorNode } from "./DisconnectorNode";
import { FuseNode } from "./FuseNode";
import { OffPageNode } from "./OffPageNode";

// Module-level constant — NEVER move inside a component or this breaks React Flow
export const nodeTypes = {
  bus: BusNode,
  generator: GeneratorNode,
  load: LoadNode,
  shunt: ShuntNode,
  transformer: TransformerNode,
  circuitBreaker: CircuitBreakerNode,
  disconnector: DisconnectorNode,
  fuse: FuseNode,
  offPage: OffPageNode,
} as const;

export {
  BusNode,
  GeneratorNode,
  LoadNode,
  ShuntNode,
  TransformerNode,
  CircuitBreakerNode,
  DisconnectorNode,
  FuseNode,
  OffPageNode,
};
