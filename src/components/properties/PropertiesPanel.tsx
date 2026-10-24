import { useDiagramStore } from "../../store/diagramStore";
import { BusForm } from "./forms/BusForm";
import { GeneratorForm } from "./forms/GeneratorForm";
import { LoadForm } from "./forms/LoadForm";
import { TransformerForm } from "./forms/TransformerForm";
import { CircuitBreakerForm } from "./forms/CircuitBreakerForm";
import { DisconnectorForm } from "./forms/DisconnectorForm";
import { FuseForm } from "./forms/FuseForm";
import { EdgeForm } from "./forms/EdgeForm";
import type { SLDNode, SLDEdge } from "../../store/types";

export function PropertiesPanel() {
  const nodes = useDiagramStore((s) => s.nodes);
  const edges = useDiagramStore((s) => s.edges);
  const selectedNodeIds = useDiagramStore((s) => s.selectedNodeIds);
  const selectedEdgeIds = useDiagramStore((s) => s.selectedEdgeIds);

  const selectedNode =
    selectedNodeIds.length === 1
      ? nodes.find((n) => n.id === selectedNodeIds[0])
      : null;

  const selectedEdge =
    selectedEdgeIds.length === 1 && !selectedNode
      ? edges.find((e) => e.id === selectedEdgeIds[0])
      : null;

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-slate-200">
        <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
          Properties
        </p>
      </div>
      <div className="flex-1 overflow-y-auto p-3">
        {selectedNode ? (
          <NodeForm node={selectedNode} />
        ) : selectedEdge ? (
          <EdgeForm edge={selectedEdge} />
        ) : (
          <p className="text-xs text-slate-400 text-center mt-8">
            Select a component to edit its properties
          </p>
        )}
      </div>
    </div>
  );
}

function NodeForm({ node }: { node: SLDNode }) {
  switch (node.type) {
    case "bus":
      return <BusForm node={node} />;
    case "generator":
      return <GeneratorForm node={node} />;
    case "load":
      return <LoadForm node={node} />;
    case "transformer":
      return <TransformerForm node={node} />;
    case "circuitBreaker":
      return <CircuitBreakerForm node={node} />;
    case "disconnector":
      return <DisconnectorForm node={node} />;
    case "fuse":
      return <FuseForm node={node} />;
    default:
      return null;
  }
}
