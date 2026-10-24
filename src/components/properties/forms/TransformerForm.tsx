import { useDiagramStore } from "../../../store/diagramStore";
import type { TransformerNode } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function TransformerForm({ node }: { node: TransformerNode }) {
  const updateNodeData = useDiagramStore((s) => s.updateNodeData);
  const d = node.data;

  function update(partial: Partial<typeof d>) {
    updateNodeData(node.id, partial);
  }

  return (
    <div>
      <SectionTitle>Transformer</SectionTitle>
      <Field label="Label">
        <Input value={d.label} onChange={(e) => update({ label: e.target.value })} />
      </Field>
      <Field label="Rating (MVA)">
        <Input
          type="number"
          value={d.ratingMVA}
          min={0}
          onChange={(e) => update({ ratingMVA: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Primary kV">
        <Input
          type="number"
          value={d.primaryKV}
          min={0}
          step={0.1}
          onChange={(e) => update({ primaryKV: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Secondary kV">
        <Input
          type="number"
          value={d.secondaryKV}
          min={0}
          step={0.1}
          onChange={(e) => update({ secondaryKV: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Connection">
        <Select
          value={d.connection}
          onChange={(e) => update({ connection: e.target.value as typeof d.connection })}
        >
          <option value="WyeWye">Wye-Wye</option>
          <option value="WyeDelta">Wye-Delta</option>
          <option value="DeltaWye">Delta-Wye</option>
          <option value="DeltaDelta">Delta-Delta</option>
        </Select>
      </Field>
      <Field label="Impedance (%)">
        <Input
          type="number"
          value={d.impedancePct}
          min={0}
          max={100}
          step={0.1}
          onChange={(e) => update({ impedancePct: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Primary Grounding">
        <Select
          value={d.groundingPrimary}
          onChange={(e) => update({ groundingPrimary: e.target.value as typeof d.groundingPrimary })}
        >
          <option value="solidly">Solidly</option>
          <option value="resistance">Resistance</option>
          <option value="none">None</option>
        </Select>
      </Field>
      <Field label="Secondary Grounding">
        <Select
          value={d.groundingSecondary}
          onChange={(e) => update({ groundingSecondary: e.target.value as typeof d.groundingSecondary })}
        >
          <option value="solidly">Solidly</option>
          <option value="resistance">Resistance</option>
          <option value="none">None</option>
        </Select>
      </Field>
    </div>
  );
}
