import { Lock } from "lucide-react";
import { useDiagramStore } from "../../store/diagramStore";
import { BusForm } from "./forms/BusForm";
import { GeneratorForm } from "./forms/GeneratorForm";
import { ShuntForm } from "./forms/ShuntForm";
import { LoadForm } from "./forms/LoadForm";
import { TransformerForm } from "./forms/TransformerForm";
import { Transformer3Form } from "./forms/Transformer3Form";
import { CircuitBreakerForm } from "./forms/CircuitBreakerForm";
import { DisconnectorForm } from "./forms/DisconnectorForm";
import { FuseForm } from "./forms/FuseForm";
import { EdgeForm } from "./forms/EdgeForm";
import { RotationField } from "./forms/RotationField";
import { LabelField } from "./forms/LabelField";
import type { SLDNode, SLDEdge, CaseRef, LabelOffset } from "../../store/types";
import { ElementInspector } from "../../ui/ElementInspector";

type Selection = { nodes: SLDNode[]; edges: SLDEdge[]; selectedNodeIds: string[]; selectedEdgeIds: string[] };

/**
 * The case element the selection stands for, when every selected symbol and
 * wire stands for the same one (a bus and the load drawn on it, say).
 */
export function selectedCaseElement(s: Selection): string | null {
  const items = [
    ...s.nodes.filter((n) => s.selectedNodeIds.includes(n.id)),
    ...s.edges.filter((e) => s.selectedEdgeIds.includes(e.id)),
  ];
  const ids = new Set(items.map((x) => (x.data?.caseRef as CaseRef | undefined)?.element ?? null));
  if (items.length === 0 || ids.size !== 1) return null;
  return [...ids][0];
}

/** The diagram's inspector. `missing` is a selected case element with no symbol in this drawing. */
export function PropertiesPanel({ missing = null }: { missing?: string | null }) {
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

  const shared = useDiagramStore(selectedCaseElement);
  // Several symbols of one element (a MATPOWER bus's bar, load and shunt all
  // stand for the bus): show the drawing fields of the one of its own kind.
  const own =
    !selectedNode && !selectedEdge && shared
      ? nodes.filter((n) => selectedNodeIds.includes(n.id) && n.type === shared.split(":")[0])
      : [];
  const primary = selectedNode ?? (own.length === 1 ? own[0] : null);
  const item = primary ?? selectedEdge;
  const element = shared ?? (item || selectedNodeIds.length || selectedEdgeIds.length ? null : missing);
  const locked = useDiagramStore((s) => s.locked);
  const fields = primary ? <NodeForm node={primary} /> : selectedEdge ? <EdgeForm edge={selectedEdge} /> : null;
  // A locked drawing is inspected, not edited: its drawing fields are read
  // only. The case element's own actions (Trip and compare) stay live.
  const form = fields && locked ? <LockedFields>{fields}</LockedFields> : fields;

  // A symbol drawn from the case is inspected as the case element; its
  // drawing fields fold into a section of their own.
  if (element) {
    return (
      <div className="h-full overflow-y-auto">
        <ElementInspector element={element} from="diagram" drawing={form ?? undefined} />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-slate-200">
        <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
          Properties
        </p>
      </div>
      <div className="flex-1 overflow-y-auto p-3">
        {form ?? (
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
    case "transformer3":
      return <Transformer3Form node={node} />;
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

function LockedFields({ children }: { children: React.ReactNode }) {
  const setLocked = useDiagramStore((s) => s.setLocked);
  return (
    <div data-testid="locked-fields">
      <p className="mb-2 flex items-center gap-1.5 rounded border border-slate-200 bg-slate-50 px-2 py-1 text-[11px] text-slate-600">
        <Lock size={11} className="shrink-0" />
        <span className="flex-1">The drawing is locked.</span>
        <button type="button" className="font-medium text-teal-700 hover:underline" onClick={() => setLocked(false)}>
          Unlock
        </button>
      </p>
      <fieldset disabled className="min-w-0 opacity-70">
        {children}
      </fieldset>
    </div>
  );
}
