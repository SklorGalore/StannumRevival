import { useDiagramStore } from "../../store/diagramStore";
import { useWorkspace } from "../../store/workspaceStore";

export function StatusBar() {
  const isDirty = useDiagramStore((s) => s.isDirty);
  const currentFilePath = useDiagramStore((s) => s.currentFilePath);
  const viewport = useDiagramStore((s) => s.viewport);
  const nodes = useDiagramStore((s) => s.nodes);
  const edges = useDiagramStore((s) => s.edges);
  const caseName = useDiagramStore((s) => s.metadata.caseName);
  const openName = useWorkspace((s) => s.caseSummary?.name);

  const fileName = currentFilePath
    ? currentFilePath.split("/").pop()
    : "Untitled";

  const zoom = Math.round((viewport.zoom ?? 1) * 100);

  return (
    <div className="h-6 flex items-center px-3 gap-3 text-[11px] text-slate-500 tabular-nums border-t border-slate-200 bg-white">
      <span>
        {fileName}
        {isDirty && " *"}
      </span>
      <span className="text-slate-300">·</span>
      <span>{zoom}%</span>
      <span className="text-slate-300">·</span>
      <span>
        {nodes.length} component{nodes.length !== 1 ? "s" : ""}
        {" · "}
        {edges.length} connection{edges.length !== 1 ? "s" : ""}
      </span>
      {caseName && (
        <>
          <span className="text-slate-300">·</span>
          <span className={caseName === openName ? "" : "text-amber-700"}>
            {caseName === openName ? `Linked to ${caseName}` : `Drawn from ${caseName}; open it to see results`}
          </span>
        </>
      )}
    </div>
  );
}
