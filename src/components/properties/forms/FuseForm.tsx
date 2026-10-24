import { useDiagramStore } from "../../../store/diagramStore";
import type { FuseNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function FuseForm({ node }: { node: FuseNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Fuse</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
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
      <Field label="Current Rating (A)">
        <Input
          type="number"
          value={d.ratingA}
          min={0}
          onChange={(e) => update({ ratingA: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Type">
        <Select
          value={d.type}
          onChange={(e) => update({ type: e.target.value as typeof d.type })}
        >
          <option value="current-limiting">Current Limiting</option>
          <option value="expulsion">Expulsion</option>
        </Select>
      </Field>
      <Field label="Status">
        <Select
          value={d.blown ? "blown" : "intact"}
          onChange={(e) => update({ blown: e.target.value === "blown" })}
        >
          <option value="intact">Intact</option>
          <option value="blown">Blown</option>
        </Select>
      </Field>
    </div>
  );
}
