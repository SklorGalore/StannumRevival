import { useDiagramStore } from "../../../store/diagramStore";
import type { SLDEdge } from "../../../store/types";
import { Field, Input, Select, SectionTitle } from "./FormField";

export function EdgeForm({ edge }: { edge: SLDEdge }) {
  const updateEdgeData = useDiagramStore((s) => s.updateEdgeData);
  const d = edge.data ?? { voltageKV: 0, conductorType: "distribution" as const };

  function update(partial: Partial<typeof d>) {
    updateEdgeData(edge.id, partial);
  }

  return (
    <div>
      <SectionTitle>Conductor</SectionTitle>
      <Field label="Label">
        <Input
          value={d.label ?? ""}
          placeholder="Optional"
          onChange={(e) => update({ label: e.target.value })}
        />
      </Field>
      <Field label="Voltage (kV)">
        <Input
          type="number"
          value={d.voltageKV}
          min={0}
          step={0.1}
          onChange={(e) => update({ voltageKV: parseFloat(e.target.value) || 0 })}
        />
      </Field>
      <Field label="Type">
        <Select
          value={d.conductorType}
          onChange={(e) =>
            update({ conductorType: e.target.value as typeof d.conductorType })
          }
        >
          <option value="transmission">Transmission</option>
          <option value="distribution">Distribution</option>
          <option value="buswork">Buswork</option>
        </Select>
      </Field>
      <Field label="Resistance R (pu)">
        <Input
          type="number"
          value={d.impedanceR ?? ""}
          placeholder="Optional"
          step={0.001}
          onChange={(e) =>
            update({ impedanceR: parseFloat(e.target.value) || undefined })
          }
        />
      </Field>
      <Field label="Reactance X (pu)">
        <Input
          type="number"
          value={d.impedanceX ?? ""}
          placeholder="Optional"
          step={0.001}
          onChange={(e) =>
            update({ impedanceX: parseFloat(e.target.value) || undefined })
          }
        />
      </Field>
    </div>
  );
}
