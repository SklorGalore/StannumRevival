import { useDiagramStore } from "../../../store/diagramStore";
import type { Transformer3Leg, Transformer3Node } from "../../../store/types";
import { Field, Input, SectionTitle } from "./FormField";

export function Transformer3Form({ node }: { node: Transformer3Node }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;
  const legs = [...(d.legs ?? [])].sort((a, b) => a.winding - b.winding);
  const setLeg = (winding: number, partial: Partial<Transformer3Leg>) =>
    updateNodeData(node.id, { legs: (d.legs ?? []).map((l) => (l.winding === winding ? { ...l, ...partial } : l)) });

  return (
    <div>
      <SectionTitle>Three-winding transformer</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => updateNodeData(node.id, { label: e.target.value })} />
      </Field>
      {legs.map((l) => (
        <div key={l.winding} className="grid grid-cols-2 gap-2">
          <Field label={`W${l.winding} kV`}>
            <Input
              type="number"
              value={l.kv}
              min={0}
              step={0.1}
              onChange={(e) => setLeg(l.winding, { kv: parseFloat(e.target.value) || 0 })}
            />
          </Field>
          <Field label={`W${l.winding} MVA`}>
            <Input
              type="number"
              value={l.ratingMVA}
              min={0}
              onChange={(e) => setLeg(l.winding, { ratingMVA: parseFloat(e.target.value) || 0 })}
            />
          </Field>
        </div>
      ))}
    </div>
  );
}
