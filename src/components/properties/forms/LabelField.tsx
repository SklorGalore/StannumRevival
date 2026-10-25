import type { LabelOffset } from "../../../store/types";
import { Field } from "./FormField";

/** Where the label sits: dragged by hand, or the default spot. */
export function LabelField({ offset, onReset }: { offset?: LabelOffset; onReset: () => void }) {
  return (
    <Field label="Label">
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-50 disabled:opacity-40 disabled:hover:bg-transparent"
          disabled={!offset}
          onClick={onReset}
        >
          Reset label position
        </button>
        <span className="text-[11px] text-slate-500">{offset ? "Moved by hand" : "Drag the text to move it"}</span>
      </div>
    </Field>
  );
}
