// Save diagnostics: the log plus versions, for reporting a problem. It holds
// nothing from the case itself; the log can name buses and files the person
// worked with, and paths under their home folder are shortened to "~".

import { getVersion } from "@tauri-apps/api/app";
import type { Diagnostics } from "../analysis/commands.gen";
import { asText, type LogEntry } from "../log/log";

/** What the window knows about itself. */
export interface AppInfo {
  version: string;
  userAgent: string;
  /** Internal errors the window was told about this session. */
  internalErrors: number;
}

/** "C:\Users\ana\cases\a.raw" -> "~\cases\a.raw"; the same for macOS and Linux homes. */
export function hideHome(text: string): string {
  return text
    .replace(/[A-Za-z]:\\Users\\[^\\\s"']+/g, "~")
    .replace(/\/(?:Users|home)\/[^/\s"']+/g, "~");
}

/** The report as plain text: a header of versions, then every log line. */
export function diagnosticsText(when: Date, app: AppInfo, core: Diagnostics | null, entries: LogEntry[]): string {
  const rows: [string, string][] = [
    ["Saved", when.toISOString()],
    ["Stannum", app.version],
    ["Window", app.userAgent],
    ["Internal errors seen by the window", String(app.internalErrors)],
  ];
  if (core) {
    rows.push(
      ["Core", `stannum-core ${core.core}, command API ${core.api_version}${core.debug_build ? ", debug build" : ""}`],
      ["Solvers", `rs-power ${core.rs_power}, nodal ${core.nodal}`],
      ["Platform", `${core.os} ${core.arch}, ${core.threads} threads`],
      ["Internal errors in the core", String(core.internal_errors)],
    );
  } else {
    rows.push(["Core", "not reachable"]);
  }
  const width = Math.max(...rows.map(([k]) => k.length));
  const header = rows.map(([k, v]) => `${(k + ":").padEnd(width + 2)}${v}`).join("\n");
  return [
    "Stannum diagnostics",
    "",
    header,
    "",
    "This file holds the app's log and versions. It does not include the case file or its data.",
    "",
    `Log (${entries.length} lines, oldest first)`,
    "",
    hideHome(asText(entries)),
  ].join("\n");
}

/** The app's version from the desktop shell, or "dev" in a plain browser. */
export async function appVersion(): Promise<string> {
  try {
    return await getVersion();
  } catch {
    return "dev (browser)";
  }
}

/** "stannum-diagnostics-2026-10-28-2031.txt" in local time. */
export function diagnosticsFileName(when: Date): string {
  const two = (n: number) => String(n).padStart(2, "0");
  return `stannum-diagnostics-${when.getFullYear()}-${two(when.getMonth() + 1)}-${two(when.getDate())}-${two(when.getHours())}${two(when.getMinutes())}.txt`;
}
