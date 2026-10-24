import { useDiagramStore } from "../../../store/diagramStore";
import type { BusNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function BusForm({ node }: { node: BusNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Bus</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
      </Field>
      <Field label="Nominal Voltage (kV)">
        <Input
          type="number"
          value={d.voltageKV}
          min={0}
          step={0.1}
          onChange={(e) => update({ voltageKV: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Orientation">
        <Select
          value={d.orientation}
          onChange={(e) => update({ orientation: e.target.value as typeof d.orientation })}
        >
          <option value="horizontal">Horizontal</option>
          <option value="vertical">Vertical</option>
        </Select>
      </Field>
      <Field label="Length (px)">
        <Input
          type="number"
          value={d.length}
          min={60}
          max={600}
          step={20}
          onChange={(e) => update({ length: parseInt(e.target.value) || 200 })}
        />
      </Field>
    </div>
  );
}
