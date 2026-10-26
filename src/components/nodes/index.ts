import { BusNode } from "./BusNode";
import { GeneratorNode } from "./GeneratorNode";
import { LoadNode } from "./LoadNode";
import { ShuntNode } from "./ShuntNode";
import { TransformerNode } from "./TransformerNode";
import { Transformer3Node } from "./Transformer3Node";
import { CircuitBreakerNode } from "./CircuitBreakerNode";
import { DisconnectorNode } from "./DisconnectorNode";
import { FuseNode } from "./FuseNode";
import { OffPageNode } from "./OffPageNode";
import { DcLinkNode } from "./DcLinkNode";

// Module-level constant — NEVER move inside a component or this breaks React Flow
export const nodeTypes = {
  bus: BusNode,
  generator: GeneratorNode,
  load: LoadNode,
  shunt: ShuntNode,
  transformer: TransformerNode,
  transformer3: Transformer3Node,
  circuitBreaker: CircuitBreakerNode,
  disconnector: DisconnectorNode,
  fuse: FuseNode,
  offPage: OffPageNode,
  dcLink: DcLinkNode,
} as const;

export {
  BusNode,
  GeneratorNode,
  LoadNode,
  ShuntNode,
  TransformerNode,
  Transformer3Node,
  CircuitBreakerNode,
  DisconnectorNode,
  FuseNode,
  OffPageNode,
  DcLinkNode,
};
