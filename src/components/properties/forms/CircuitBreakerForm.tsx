import { useDiagramStore } from "../../../store/diagramStore";
import type { CircuitBreakerNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function CircuitBreakerForm({ node }: { node: CircuitBreakerNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Circuit Breaker</SectionTitle>
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
          <option value="fault">Fault</option>
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
      <Field label="Interrupting Capacity (kA)">
        <Input
          type="number"
          value={d.ratingKA}
          min={0}
          step={0.1}
          onChange={(e) => update({ ratingKA: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Normally Open">
        <Select
          value={d.normallyOpen ? "yes" : "no"}
          onChange={(e) => update({ normallyOpen: e.target.value === "yes" })}
        >
          <option value="no">No (NC)</option>
          <option value="yes">Yes (NO)</option>
        </Select>
      </Field>
    </div>
  );
}
