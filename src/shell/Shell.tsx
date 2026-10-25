import { useEffect } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ModuleRail } from "./ModuleRail";
import { CaseBar } from "./CaseBar";
import { CommandPalette } from "./CommandPalette";
import { useWorkspace } from "../store/workspaceStore";
import { DiagramModule } from "../modules/diagram/DiagramModule";
import { PowerFlowModule } from "../modules/power/PowerFlowModule";
import { TopologyModule } from "../modules/topology/TopologyModule";
import { CaseDiffModule } from "../modules/diff/CaseDiffModule";

const CASE_EXTENSIONS = /\.(raw|m)$/i;

/** Open a case dropped anywhere on the window (outside the diff file pickers). */
function useCaseDrop() {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    let webview;
    try {
      webview = getCurrentWebview();
    } catch {
      return; // Not running inside Tauri (e.g. plain `vite` in a browser).
    }
    webview
      .onDragDropEvent((event) => {
        if (event.payload.type !== "drop") return;
        const path = event.payload.paths.find((p) => CASE_EXTENSIONS.test(p));
        const state = useWorkspace.getState();
        if (path) void state.openCase(path);
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {
        // Drag-and-drop events are unavailable; the Open button still works.
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

export function Shell() {
  const module = useWorkspace((s) => s.module);
  useCaseDrop();
  // Modules stay mounted so switching keeps their results and scroll state.
  // `module-hidden` forces descendants hidden too: React Flow sets
  // `visibility: visible` on its nodes, which would otherwise show through.
  const pane = (id: typeof module) => `absolute inset-0 ${module === id ? "" : "module-hidden pointer-events-none"}`;
  return (
    <div className="h-screen w-screen flex bg-slate-50 overflow-hidden text-slate-800">
      <ModuleRail />
      <div className="flex-1 min-w-0 flex flex-col">
        <CaseBar />
        <CommandPalette />
        <div className="flex-1 min-h-0 relative">
          <div className={pane("diagram")} aria-hidden={module !== "diagram"}>
            <DiagramModule active={module === "diagram"} />
          </div>
          <div className={pane("power")} aria-hidden={module !== "power"}>
            <PowerFlowModule active={module === "power"} />
          </div>
          <div className={pane("topology")} aria-hidden={module !== "topology"}>
            <TopologyModule active={module === "topology"} />
          </div>
          <div className={pane("diff")} aria-hidden={module !== "diff"}>
            <CaseDiffModule active={module === "diff"} />
          </div>
        </div>
      </div>
    </div>
  );
}
