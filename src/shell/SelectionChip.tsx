import { useEffect } from "react";
import { Crosshair, Link2, Link2Off, X } from "lucide-react";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import { elementLabel } from "../lib/elements";

const TARGETS: { id: ModuleId; label: string }[] = [
  { id: "diagram", label: "Diagram" },
  { id: "power", label: "Power flow" },
  { id: "topology", label: "Topology" },
  { id: "diff", label: "Case diff" },
];

/** The shared selection, with buttons to see it in another module. Esc clears it. */
export function SelectionChip() {
  const selection = useWorkspace((s) => s.selection);
  const module = useWorkspace((s) => s.module);
  const summary = useWorkspace((s) => s.caseSummary);
  const setModule = useWorkspace((s) => s.setModule);
  const select = useWorkspace((s) => s.select);
  const linked = useWorkspace((s) => s.linked);
  const setLinked = useWorkspace((s) => s.setLinked);

  useEffect(() => {
    if (!selection) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName))
      )
        return;
      if (document.querySelector("[role=dialog]")) return;
      select(null, module);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selection, module, select]);

  if (!summary) return null;
  const toggle = (
    <button
      className={`shrink-0 inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium ${
        linked
          ? "border-teal-300 bg-teal-50 text-teal-800 hover:bg-teal-100"
          : "border-slate-300 text-slate-600 hover:bg-slate-50"
      }`}
      onClick={() => setLinked(!linked)}
      aria-pressed={linked}
      title={
        linked
          ? "Selections are linked: picking a bus or branch selects it in every module. Click to unlink."
          : "Link selections so picking a bus or branch in one module selects it in the others."
      }
    >
      {linked ? <Link2 size={12} /> : <Link2Off size={12} />}
      {linked ? "Linked" : "Link selection"}
    </button>
  );
  if (!selection) return toggle;
  const available: Record<ModuleId, boolean> = {
    diagram: summary.power.available,
    power: summary.power.available,
    topology: summary.topology.available,
    diff: summary.diffable,
  };
  const targets = TARGETS.filter((t) => t.id !== module && available[t.id]);
  return (
    <>
      {toggle}
      <div
        className="min-w-0 flex items-center gap-1 rounded-full border border-teal-300 bg-teal-50 pl-2.5 pr-1 py-0.5 text-[11px] text-teal-900"
        data-testid="selection-chip"
        title={selection.element}
      >
        <Crosshair size={12} className="shrink-0 text-teal-700" />
        <span className="font-semibold truncate">
          {elementLabel(selection.element)}
        </span>
        {targets.length > 0 && (
          <span className="text-teal-700/70 shrink-0 pl-1">Show in</span>
        )}
        {targets.map((t) => (
          <button
            key={t.id}
            className="shrink-0 rounded-full px-1.5 py-px font-medium text-teal-800 hover:bg-teal-100"
            onClick={() => setModule(t.id)}
            aria-label={`Show in ${t.label}`}
          >
            {t.label}
          </button>
        ))}
        <button
          className="shrink-0 rounded-full p-0.5 text-teal-700 hover:bg-teal-100"
          onClick={() => select(null, module)}
          aria-label="Clear selection"
          title="Clear selection (Esc)"
        >
          <X size={12} />
        </button>
      </div>
    </>
  );
}
