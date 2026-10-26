import { Share2, Spline } from "lucide-react";
import { useDrawingStyle, type DrawingStyle } from "../../../store/drawingStyleStore";
import { useWorkspace } from "../../../store/workspaceStore";
import { showNetwork, showOneLine } from "./switchStyle";

/** The Network / One-line switch at the start of the Diagram's toolbar. */
export function StyleSwitch() {
  const style = useDrawingStyle((s) => s.style);
  const busy = useDrawingStyle((s) => s.busy);
  const summary = useWorkspace((s) => s.caseSummary);
  const networkReady = !!summary?.topology.available;
  const options: { id: DrawingStyle; label: string; icon: React.ReactNode; title: string; disabled: boolean }[] = [
    {
      id: "network",
      label: "Network",
      icon: <Share2 size={13} />,
      title: networkReady
        ? "Draw the neighbourhood as a bus-branch graph: fast for hundreds of buses, with bridges and cut buses marked"
        : summary
          ? `This case has no topology to draw: ${summary.topology.error ?? "it couldn't be read"}`
          : "Open a case to draw its network",
      disabled: !networkReady,
    },
    {
      id: "oneline",
      label: "One-line",
      icon: <Spline size={13} />,
      title: "Draw the neighbourhood as a one-line: symbols, routed wires, results and editing",
      disabled: false,
    },
  ];
  return (
    <div role="radiogroup" aria-label="Drawing style" className="flex items-center rounded-md border border-slate-200 bg-slate-50 p-0.5 mr-2 shrink-0">
      {options.map((o) => {
        const on = style === o.id;
        return (
          <button
            key={o.id}
            role="radio"
            aria-checked={on}
            disabled={o.disabled || busy}
            title={o.title}
            onClick={() => {
              if (on) return;
              if (o.id === "network") showNetwork();
              else void showOneLine();
            }}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium transition-colors ${
              on ? "bg-white text-teal-800 shadow-sm ring-1 ring-inset ring-teal-200" : "text-slate-600 hover:text-slate-900"
            } disabled:opacity-40 disabled:cursor-not-allowed`}
          >
            {o.icon}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
