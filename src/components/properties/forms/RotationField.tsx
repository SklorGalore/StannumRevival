import { useDiagramStore } from "../../../store/diagramStore";
import type { Rotation, SLDNode } from "../../../store/types";
import { Field } from "./FormField";

const BUTTON =
  "px-2 py-1 text-xs border border-slate-200 rounded bg-white text-slate-700 hover:bg-slate-50 " +
  "focus:outline-none focus:ring-1 focus:ring-teal-500";

/** Turn the selected symbol a quarter either way (also R and Shift+R). */
export function RotationField({ node }: { node: SLDNode }) {
  const rotateSelection = useDiagramStore((s) => s.rotateSelection);
  const rotation = ((node.data as { rotation?: Rotation }).rotation ?? 0) as Rotation;
  return (
    <Field label="Rotation">
      <div className="flex items-center gap-1.5">
        <button type="button" className={BUTTON} title="Turn anticlockwise (Shift+R)" aria-label="Turn anticlockwise" onClick={() => rotateSelection(-90)}>
          ⟲
        </button>
        <button type="button" className={BUTTON} title="Turn clockwise (R)" aria-label="Turn clockwise" onClick={() => rotateSelection(90)}>
          ⟳
        </button>
        <span className="ml-1 text-xs tabular-nums text-slate-600" data-testid="rotation-angle">
          {rotation}°
        </span>
      </div>
    </Field>
  );
}
