import { Map as MapIcon, Share2, Spline } from "lucide-react";
import { useDrawingStyle, type DrawingStyle } from "../../../store/drawingStyleStore";
import { useWorkspace } from "../../../store/workspaceStore";
import { showNetwork, showOneLine } from "./switchStyle";

/** The Network / One-line / Map switch at the start of the Diagram's toolbar. */
export function StyleSwitch({ compact = false }: { compact?: boolean }) {
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
    {
      id: "map",
      label: "Map",
      icon: <MapIcon size={13} />,
      title: !summary
        ? "Open a case to draw it on a map"
        : !summary.power.available
          ? "This case can't be read for power flow, so it has no buses to place"
          : summary.coordinates
            ? "Draw the whole case at its bus coordinates, with the neighbourhood highlighted"
            : "Draw the case on a map: attach a CSV of bus coordinates first (RAW files with substation places draw on their own)",
      disabled: !summary?.power.available,
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
            aria-label={o.label}
            onClick={() => {
              if (on) return;
              if (o.id === "oneline") void showOneLine();
              else showNetwork(o.id);
            }}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded text-xs whitespace-nowrap font-medium transition-colors ${
              on ? "bg-white text-teal-800 shadow-sm ring-1 ring-inset ring-teal-200" : "text-slate-600 hover:text-slate-900"
            } disabled:opacity-40 disabled:cursor-not-allowed`}
          >
            {o.icon}
            {!compact && o.label}
          </button>
        );
      })}
    </div>
  );
}
