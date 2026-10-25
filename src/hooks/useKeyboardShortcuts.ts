import { useEffect } from "react";
import { useDiagramStore } from "../store/diagramStore";
import { saveDiagram } from "../lib/tauri/fileOps";

const ARROWS: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

/** Diagram shortcuts; pass `enabled=false` while another module is showing. */
export function useKeyboardShortcuts(enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    function onKeyDown(e: KeyboardEvent) {
      const el = e.target as HTMLElement;
      // Leave typing in fields alone.
      if (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable) return;

      const store = useDiagramStore.getState();
      const ctrl = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      const run = (fn: () => void) => {
        e.preventDefault();
        fn();
      };

      if (ctrl) {
        if (key === "z" && !e.shiftKey) return run(store.undo);
        if ((key === "z" && e.shiftKey) || key === "y") return run(store.redo);
        if (key === "c") return run(store.copySelection);
        if (key === "x") return run(() => (store.copySelection(), store.deleteSelected()));
        if (key === "v") return run(store.paste);
        if (key === "d") return run(store.duplicateSelection);
        if (key === "a") return run(store.selectAll);
        if (key === "s") return run(saveDiagram);
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") return run(store.deleteSelected);
      if (e.key === "Escape") return store.clearSelection();
      // R turns clockwise, Shift+R back.
      if (key === "r" && !e.altKey) return run(() => store.rotateSelection(e.shiftKey ? -90 : 90));
      const arrow = ARROWS[e.key];
      if (arrow) {
        // One grid step, or a single unit with Shift for fine placement.
        const step = e.shiftKey ? 1 : store.gridSize;
        return run(() => store.nudge(arrow[0] * step, arrow[1] * step));
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enabled]);
}
