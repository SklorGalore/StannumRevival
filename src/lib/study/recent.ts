// Recently opened cases and studies, newest first, for the empty screens.
// Kept in this browser profile only; a missing or unreadable list is empty.

export interface RecentFile {
  path: string;
  name: string;
  kind: "case" | "study";
  /** Milliseconds since 1970. */
  at: number;
}

const KEY = "stannum-recent-files";
export const RECENT_LIMIT = 8;

type Listener = (files: RecentFile[]) => void;
const listeners = new Set<Listener>();

export function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** The folder part of a path, for showing under the name. */
export function folderOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : "";
}

export function recentFiles(): RecentFile[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (f): f is RecentFile =>
        !!f && typeof f.path === "string" && typeof f.name === "string" && (f.kind === "case" || f.kind === "study") && typeof f.at === "number"
    );
  } catch {
    return [];
  }
}

function store(files: RecentFile[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(files));
  } catch {
    // Storage may be unavailable; the list just isn't remembered.
  }
  for (const l of listeners) l(files);
}

/** Put a file at the top of the list (moving it if it is already there). */
export function withRecent(files: RecentFile[], file: RecentFile): RecentFile[] {
  return [file, ...files.filter((f) => f.path !== file.path)].slice(0, RECENT_LIMIT);
}

export function addRecent(path: string, kind: RecentFile["kind"], now = Date.now()) {
  store(withRecent(recentFiles(), { path, name: fileName(path), kind, at: now }));
}

export function removeRecent(path: string) {
  store(recentFiles().filter((f) => f.path !== path));
}

export function onRecentChange(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
