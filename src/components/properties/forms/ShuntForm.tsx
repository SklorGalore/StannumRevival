import { useDiagramStore } from "../../../store/diagramStore";
import type { ShuntNode } from "../../../store/types";
import { Field, Input, SectionTitle } from "./FormField";

export function ShuntForm({ node }: { node: ShuntNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Shunt</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
      </Field>
      <Field label="Reactive power at 1 pu (Mvar, + capacitor, − reactor)">
        <Input
          type="number"
          value={d.nominalMVAR}
          onChange={(e) => update({ nominalMVAR: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Active power at 1 pu (MW)">
        <Input
          type="number"
          value={d.nominalMW}
          onChange={(e) => update({ nominalMW: parseFloat(e.target.value) || 0 })}
        />
      </Field>
    </div>
  );
}
