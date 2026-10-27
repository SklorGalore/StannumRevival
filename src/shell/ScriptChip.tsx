import { Plug } from "lucide-react";
import { useAttach } from "../store/attachStore";

/** Says when a Python script is driving the window (stannum.attach()). */
export function ScriptChip() {
  const clients = useAttach((s) => s.clients);
  if (clients === 0) return null;
  return (
    <span
      className="shrink-0 inline-flex items-center gap-1 rounded-full border border-teal-300 bg-teal-50 px-2 py-0.5 text-[11px] font-medium text-teal-800"
      title="A Python script is driving this window with stannum.attach(). What it changes shows here as it happens."
      data-testid="script-chip"
    >
      <Plug size={12} /> {clients === 1 ? "Script attached" : `${clients} scripts attached`}
    </span>
  );
}
