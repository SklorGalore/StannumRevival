import { useDiagramStore } from "../../../store/diagramStore";
import type { DisconnectorNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function DisconnectorForm({ node }: { node: DisconnectorNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Disconnector</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
      </Field>
      <Field label="State">
        <Select
          value={d.state}
          onChange={(e) => update({ state: e.target.value as typeof d.state })}
        >
          <option value="closed">Closed</option>
          <option value="open">Open</option>
        </Select>
      </Field>
      <Field label="Rating (kV)">
        <Input
          type="number"
          value={d.ratingKV}
          min={0}
          step={0.1}
          onChange={(e) => update({ ratingKV: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Type">
        <Select
          value={d.type}
          onChange={(e) => update({ type: e.target.value as typeof d.type })}
        >
          <option value="gang-operated">Gang Operated</option>
          <option value="single-pole">Single Pole</option>
        </Select>
      </Field>
    </div>
  );
}
