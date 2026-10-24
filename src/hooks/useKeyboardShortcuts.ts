import { useEffect } from "react";
import { useDiagramStore } from "../store/diagramStore";
import { saveDiagram } from "../lib/tauri/fileOps";

export function useKeyboardShortcuts() {
  const deleteSelected = useDiagramStore((s) => s.deleteSelected);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement).tagName;
      // Don't fire when focus is in an input/textarea
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      const ctrl = e.ctrlKey || e.metaKey;

      if ((e.key === "Delete" || e.key === "Backspace") && !ctrl) {
        e.preventDefault();
        deleteSelected();
      }

      if (ctrl && e.key === "s") {
        e.preventDefault();
        saveDiagram();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [deleteSelected]);
}
