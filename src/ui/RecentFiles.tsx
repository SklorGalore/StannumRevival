import { useEffect, useState } from "react";
import { FileCode, FileText, FlaskConical, X } from "lucide-react";
import { folderOf, onRecentChange, recentFiles, removeRecent, type RecentFile } from "../lib/study/recent";
import { useStudyFile } from "../store/studyFileStore";

/** Recently opened cases, run files and studies, for an empty screen. Renders nothing when there are none. */
export function RecentFiles() {
  const [files, setFiles] = useState<RecentFile[]>(recentFiles);
  useEffect(() => onRecentChange(setFiles), []);
  const openFile = useStudyFile((s) => s.openFile);
  if (files.length === 0) return null;
  return (
    <div className="mt-6 w-full max-w-md text-left" data-testid="recent-files">
      <h3 className="px-2 text-[10px] font-semibold uppercase tracking-wider text-slate-500">Recent</h3>
      <ul className="mt-1">
        {files.map((f) => (
          <li key={f.path} className="group flex items-center rounded-md hover:bg-slate-100">
            <button
              type="button"
              className="flex-1 min-w-0 flex items-center gap-2.5 px-2 py-1.5 text-left"
              onClick={() => void openFile(f.path)}
              title={f.path}
            >
              {f.kind === "study" ? (
                <FlaskConical size={15} className="shrink-0 text-teal-700" aria-label="Study" />
              ) : f.kind === "run" ? (
                <FileCode size={15} className="shrink-0 text-teal-700" aria-label="Run file" />
              ) : (
                <FileText size={15} className="shrink-0 text-slate-500" aria-label="Case" />
              )}
              <span className="min-w-0">
                <span className="block truncate text-sm text-slate-800">{f.name}</span>
                <span className="block truncate text-[11px] text-slate-500">{folderOf(f.path)}</span>
              </span>
            </button>
            <button
              type="button"
              className="mr-1 p-1 rounded text-slate-500 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-slate-200 hover:text-slate-700"
              onClick={() => removeRecent(f.path)}
              title="Remove from the list"
              aria-label={`Remove ${f.name} from recent files`}
            >
              <X size={12} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
