import { useDiagramStore } from "../../../store/diagramStore";
import type { LoadNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function LoadForm({ node }: { node: LoadNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Load</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
      </Field>
      <Field label="Load Type">
        <Select
          value={d.loadType}
          onChange={(e) => update({ loadType: e.target.value as typeof d.loadType })}
        >
          <option value="static">Static</option>
          <option value="motor">Motor</option>
          <option value="composite">Composite</option>
        </Select>
      </Field>
      <Field label="Active Power (MW)">
        <Input
          type="number"
          value={d.nominalMW}
          min={0}
          onChange={(e) => update({ nominalMW: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Reactive Power (MVAR)">
        <Input
          type="number"
          value={d.nominalMVAR}
          onChange={(e) => update({ nominalMVAR: parseFloat(e.target.value) || 0 })}
        />
      </Field>
    </div>
  );
}
