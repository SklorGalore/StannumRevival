import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CoreEntry, LogEntry, LogLevel } from "../lib/log/log";
import { duration } from "../lib/log/log";

/** Lines kept in the window; the oldest go first. */
const LIMIT = 5000;

/** Something the app is busy with, shown in the log strip while it runs. */
export interface Activity {
  id: number;
  label: string;
  started: number;
}

export type DockTab = "log" | "history" | "alerts";

interface LogState {
  entries: LogEntry[];
  running: Activity[];
  /** Newest core line received, so catching up never repeats one. */
  coreSeq: number;
  /** Warnings and errors for the open case since the panel was last opened. */
  unseen: { warn: number; error: number };
  open: boolean;
  setOpen: (open: boolean) => void;
  /** Which page the drawer shows: the log lines, the history of changes, or the alerts for the shown result. */
  tab: DockTab;
  setTab: (tab: DockTab) => void;
  /** Open the drawer on the history. */
  showHistory: () => void;
  /** Open the drawer on the alerts. */
  showAlerts: () => void;
  add: (level: LogLevel, message: string, source?: string) => void;
  ingest: (lines: CoreEntry[]) => void;
  clear: () => void;
  /** Start the badge afresh for a newly opened (or closed) case. */
  newCase: () => void;
  /** Mark the start of an operation; call the returned function when it ends. */
  begin: (label: string) => (outcome: { error?: string; done?: string; quiet?: boolean; cancelled?: boolean }) => void;
}

let key = 0;

function trim(entries: LogEntry[]): LogEntry[] {
  return entries.length > LIMIT ? entries.slice(entries.length - LIMIT) : entries;
}

function bump(unseen: LogState["unseen"], lines: { level: LogLevel }[], open: boolean) {
  if (open) return unseen;
  let { warn, error } = unseen;
  for (const l of lines) {
    if (l.level === "warn") warn++;
    else if (l.level === "error") error++;
  }
  return warn === unseen.warn && error === unseen.error ? unseen : { warn, error };
}

export const useLog = create<LogState>((set, get) => ({
  entries: [],
  running: [],
  coreSeq: 0,
  unseen: { warn: 0, error: 0 },
  open: false,
  // Opening shows the log lines; History opens with showHistory.
  setOpen: (open) => set(open ? { open, tab: "log", unseen: { warn: 0, error: 0 } } : { open }),
  tab: "log",
  setTab: (tab) => set({ tab }),
  showHistory: () => set({ open: true, tab: "history", unseen: { warn: 0, error: 0 } }),
  showAlerts: () => set({ open: true, tab: "alerts" }),
  add: (level, message, source = "app") => {
    const line: LogEntry = { key: ++key, time: Date.now(), level, source, message };
    set((s) => ({ entries: trim([...s.entries, line]), unseen: bump(s.unseen, [line], s.open) }));
  },
  ingest: (lines) => {
    const fresh = lines.filter((l) => l.seq > get().coreSeq);
    if (!fresh.length) return;
    const added = fresh.map((l): LogEntry => ({ key: ++key, time: l.time, level: l.level, source: l.source, message: l.message }));
    set((s) => ({
      entries: trim([...s.entries, ...added]),
      coreSeq: fresh[fresh.length - 1].seq,
      unseen: bump(s.unseen, added, s.open),
    }));
  },
  clear: () => set({ entries: [], unseen: { warn: 0, error: 0 } }),
  newCase: () => set({ unseen: { warn: 0, error: 0 } }),
  begin: (label) => {
    const activity: Activity = { id: ++key, label, started: Date.now() };
    set((s) => ({ running: [...s.running, activity] }));
    return ({ error, done, quiet, cancelled }) => {
      set((s) => ({ running: s.running.filter((a) => a.id !== activity.id) }));
      const took = duration(Date.now() - activity.started);
      if (cancelled) get().add("info", `${label} cancelled after ${took}`);
      else if (error) get().add("error", `${label} failed after ${took}: ${error}`);
      else if (!quiet) get().add("info", `${done ?? label} (${took})`);
    };
  },
}));

/** Fetch core lines the window hasn't seen. Safe to call at any time. */
export async function catchUpLog(): Promise<void> {
  try {
    // A core command, sent the way api.ts's invokeCore does (importing it here would be circular).
    const lines = await invoke<CoreEntry[]>("stannum_command", {
      command: "log_entries",
      args: { since: useLog.getState().coreSeq },
    });
    useLog.getState().ingest(lines);
  } catch {
    // No core to ask (plain browser without the dev bridge).
  }
}

/** Start receiving core lines as they are written (desktop app only). */
export function startLogStream(): () => void {
  let unlisten: (() => void) | undefined;
  let cancelled = false;
  listen<CoreEntry[]>("stannum://log", (event) => useLog.getState().ingest(event.payload))
    .then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    })
    .catch(() => {
      // Events are unavailable outside Tauri; catchUpLog() still works.
    });
  void catchUpLog();
  return () => {
    cancelled = true;
    unlisten?.();
  };
}
