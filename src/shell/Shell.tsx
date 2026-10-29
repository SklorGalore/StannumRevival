import { useEffect, useRef, type ReactNode, type RefObject } from "react";
import { ArrowLeftRight, X } from "lucide-react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ModuleRail } from "./ModuleRail";
import { CaseBar } from "./CaseBar";
import { StudyNoticeBar } from "./StudyFileControls";
import { RunFileBar } from "./RunFileBar";
import { UpdateBar } from "./UpdateBar";
import { startUpdateChecks } from "../store/updateStore";
import { useRunFile } from "../store/runFileStore";
import { startAttach } from "../store/attachStore";
import { RUN_FILE_EXTENSION } from "../lib/analysis/api";
import { useStudyFile } from "../store/studyFileStore";
import { STUDY_EXTENSION } from "../lib/study/studyFile";
import { CommandPalette } from "./CommandPalette";
import { LogDock } from "./LogDock";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import { MODULES } from "./ModuleRail";
import { DiagramModule } from "../modules/diagram/DiagramModule";
import { PowerFlowModule } from "../modules/power/PowerFlowModule";
import { CaseDiffModule } from "../modules/diff/CaseDiffModule";
import { RecordsModule } from "../modules/records/RecordsModule";

const CASE_EXTENSIONS = /\.(raw|m)$/i;

/** Open a case or study, or run a run file, dropped anywhere on the window (outside the diff file pickers). */
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
        const run = event.payload.paths.find((p) => RUN_FILE_EXTENSION.test(p));
        if (run) return void useRunFile.getState().run(run);
        const path = event.payload.paths.find((p) => CASE_EXTENSIONS.test(p) || STUDY_EXTENSION.test(p));
        if (path) void useStudyFile.getState().openFile(path);
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

/** Above a pane in split view: which module it shows, and swap or close. */
function PaneHeader({ side }: { side: "left" | "right" }) {
  const module = useWorkspace((s) => s.module);
  const split = useWorkspace((s) => s.split);
  const setModule = useWorkspace((s) => s.setModule);
  const setSplit = useWorkspace((s) => s.setSplit);
  const swap = useWorkspace((s) => s.swapSplit);
  const shown = side === "left" ? module : split!;
  const other = side === "left" ? split : module;
  return (
    <div className="h-8 shrink-0 flex items-center gap-1.5 px-2 border-b border-slate-200 bg-slate-50 text-xs">
      <select
        aria-label={side === "left" ? "Left pane" : "Right pane"}
        className="bg-transparent font-medium text-slate-700 rounded px-1 py-0.5 hover:bg-slate-200/60 focus:outline-none focus:ring-2 focus:ring-teal-500/40"
        value={shown}
        onChange={(e) => {
          const m = e.target.value as ModuleId;
          if (side === "left") setModule(m);
          else setSplit(m);
        }}
      >
        {MODULES.filter((m) => m.id !== other).map((m) => (
          <option key={m.id} value={m.id}>
            {m.label}
          </option>
        ))}
      </select>
      {side === "right" && (
        <span className="ml-auto flex items-center gap-0.5">
          <button type="button" className="p-1 rounded text-slate-500 hover:bg-slate-200/60 hover:text-slate-800" onClick={swap} title="Swap sides" aria-label="Swap sides">
            <ArrowLeftRight size={13} />
          </button>
          <button
            type="button"
            className="p-1 rounded text-slate-500 hover:bg-slate-200/60 hover:text-slate-800"
            onClick={() => setSplit(null)}
            title="Close split view"
            aria-label="Close split view"
          >
            <X size={13} />
          </button>
        </span>
      )}
    </div>
  );
}

/** Drag to share the width between the two panes; double-click to even them. */
function Divider({ container }: { container: RefObject<HTMLDivElement | null> }) {
  const ratio = useWorkspace((s) => s.splitRatio);
  const setRatio = useWorkspace((s) => s.setSplitRatio);
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize panes"
      aria-valuenow={Math.round(ratio * 100)}
      tabIndex={0}
      className="absolute top-0 bottom-0 z-20 w-1.5 -ml-[3px] cursor-col-resize bg-slate-200 hover:bg-teal-400 focus:bg-teal-400 focus:outline-none"
      style={{ left: `${ratio * 100}%` }}
      onDoubleClick={() => setRatio(0.5)}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") setRatio(ratio - 0.05);
        if (e.key === "ArrowRight") setRatio(ratio + 0.05);
      }}
      onPointerDown={(e) => {
        const box = container.current?.getBoundingClientRect();
        if (!box) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        const move = (ev: PointerEvent) => setRatio((ev.clientX - box.left) / box.width);
        const up = () => {
          window.removeEventListener("pointermove", move);
          window.removeEventListener("pointerup", up);
        };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      }}
    />
  );
}

export function Shell() {
  const module = useWorkspace((s) => s.module);
  const split = useWorkspace((s) => s.split);
  const ratio = useWorkspace((s) => s.splitRatio);
  const body = useRef<HTMLDivElement>(null);
  useCaseDrop();
  // Python scripts can drive the window (stannum.attach()).
  useEffect(() => startAttach(), []);
  // Release builds look for a newer version a little after startup.
  useEffect(() => startUpdateChecks(), []);
  const shown = (id: ModuleId) => id === module || id === split;
  // Modules stay mounted so switching keeps their results and scroll state.
  // `module-hidden` forces descendants hidden too: React Flow sets
  // `visibility: visible` on its nodes, which would otherwise show through.
  const pane = (id: ModuleId, content: ReactNode) => {
    const side = !split ? null : id === module ? "left" : id === split ? "right" : null;
    const style = !split || !side ? undefined : side === "left" ? { left: 0, width: `${ratio * 100}%` } : { left: `${ratio * 100}%`, right: 0 };
    return (
      <div
        key={id}
        className={`absolute ${split && side ? "inset-y-0" : "inset-0"} flex flex-col ${shown(id) ? "" : "module-hidden pointer-events-none"}`}
        style={style}
        aria-hidden={!shown(id)}
        data-pane={side ?? undefined}
      >
        {side && <PaneHeader side={side} />}
        <div className="flex-1 min-h-0 relative">{content}</div>
      </div>
    );
  };
  return (
    <div className="h-screen w-screen flex bg-slate-50 overflow-hidden text-slate-800">
      <ModuleRail />
      <div className="flex-1 min-w-0 flex flex-col">
        <CaseBar />
        <StudyNoticeBar />
        <RunFileBar />
        <UpdateBar />
        <CommandPalette />
        <div ref={body} className="flex-1 min-h-0 relative">
          {pane("diagram", <DiagramModule active={shown("diagram")} />)}
          {pane("power", <PowerFlowModule active={shown("power")} />)}
          {pane("records", <RecordsModule active={shown("records")} />)}
          {pane("diff", <CaseDiffModule active={shown("diff")} />)}
          {split && <Divider container={body} />}
        </div>
        <LogDock />
      </div>
    </div>
  );
}
