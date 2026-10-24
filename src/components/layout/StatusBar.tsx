import { useDiagramStore } from "../../store/diagramStore";

export function StatusBar() {
  const isDirty = useDiagramStore((s) => s.isDirty);
  const currentFilePath = useDiagramStore((s) => s.currentFilePath);
  const viewport = useDiagramStore((s) => s.viewport);
  const nodes = useDiagramStore((s) => s.nodes);
  const edges = useDiagramStore((s) => s.edges);

  const fileName = currentFilePath
    ? currentFilePath.split("/").pop()
    : "Untitled";

  const zoom = Math.round((viewport.zoom ?? 1) * 100);

  return (
    <div className="h-6 flex items-center px-3 gap-4 text-xs text-slate-400 font-mono border-t border-slate-200 bg-slate-50">
      <span>
        {fileName}
        {isDirty && " *"}
      </span>
      <span className="text-slate-300">|</span>
      <span>Zoom: {zoom}%</span>
      <span className="text-slate-300">|</span>
      <span>
        {nodes.length} component{nodes.length !== 1 ? "s" : ""}
        {" · "}
        {edges.length} connection{edges.length !== 1 ? "s" : ""}
      </span>
    </div>
  );
}
