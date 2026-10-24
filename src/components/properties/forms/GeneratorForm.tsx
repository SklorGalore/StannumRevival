import { useDiagramStore } from "../../../store/diagramStore";
import type { GeneratorNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function GeneratorForm({ node }: { node: GeneratorNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Generator</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
      </Field>
      <Field label="Nominal Rating (MW)">
        <Input
          type="number"
          value={d.nominalMW}
          min={0}
          onChange={(e) => update({ nominalMW: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Terminal Voltage (kV)">
        <Input
          type="number"
          value={d.voltageKV}
          min={0}
          step={0.1}
          onChange={(e) => update({ voltageKV: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Power Factor">
        <Input
          type="number"
          value={d.powerFactor}
          min={0}
          max={1}
          step={0.01}
          onChange={(e) => update({ powerFactor: parseFloat(e.target.value) || 0.85 })}
        />
      </Field>
      <Field label="Grounding">
        <Select
          value={d.groundingType}
          onChange={(e) => update({ groundingType: e.target.value as typeof d.groundingType })}
        >
          <option value="solidly">Solidly Grounded</option>
          <option value="resistance">Resistance Grounded</option>
          <option value="reactance">Reactance Grounded</option>
          <option value="ungrounded">Ungrounded</option>
        </Select>
      </Field>
    </div>
  );
}
