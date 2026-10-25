import { useDiagramStore } from "../../store/diagramStore";
import { BusForm } from "./forms/BusForm";
import { GeneratorForm } from "./forms/GeneratorForm";
import { ShuntForm } from "./forms/ShuntForm";
import { LoadForm } from "./forms/LoadForm";
import { TransformerForm } from "./forms/TransformerForm";
import { CircuitBreakerForm } from "./forms/CircuitBreakerForm";
import { DisconnectorForm } from "./forms/DisconnectorForm";
import { FuseForm } from "./forms/FuseForm";
import { EdgeForm } from "./forms/EdgeForm";
import { RotationField } from "./forms/RotationField";
import { LabelField } from "./forms/LabelField";
import type { SLDNode, SLDEdge, CaseRef, LabelOffset } from "../../store/types";
import { elementLabel } from "../../lib/elements";
import { StudyActions } from "../../ui/StudyActions";

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
        <CaseElement caseRef={(selectedNode ?? selectedEdge)?.data?.caseRef as CaseRef | undefined} />
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
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const label = (
    <LabelField
      offset={(node.data as { labelOffset?: LabelOffset }).labelOffset}
      onReset={() => updateNodeData(node.id, { labelOffset: undefined })}
    />
  );
  if (node.type === "bus")
    return (
      <>
        <BusForm node={node} />
        {label}
      </>
    );
  return (
    <>
      <SymbolForm node={node} />
      <RotationField node={node} />
      {label}
    </>
  );
}

function SymbolForm({ node }: { node: SLDNode }) {
  switch (node.type) {
    case "bus":
      return <BusForm node={node} />;
    case "generator":
      return <GeneratorForm node={node} />;
    case "load":
      return <LoadForm node={node} />;
    case "shunt":
      return <ShuntForm node={node} />;
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

/** Which record of the open case the selected symbol stands for. */
function CaseElement({ caseRef }: { caseRef?: CaseRef }) {
  if (!caseRef?.element) return null;
  return (
    <div className="mb-3 rounded-md border border-slate-200 bg-slate-50 px-2 py-1.5" title="Shared id used by every module">
      <p className="text-[10px] font-semibold uppercase tracking-wider text-slate-400">In the open case</p>
      <p className="text-xs font-medium text-slate-700">{elementLabel(caseRef.element)}</p>
      <p className="font-mono text-[10px] text-slate-500 break-all">{caseRef.element}</p>
      <div className="mt-1 -ml-1.5">
        <StudyActions element={caseRef.element} />
      </div>
    </div>
  );
}
