// Checking for, downloading and installing a newer Stannum.
//
// Release builds ask the latest published GitHub release for its latest.json
// (see docs/releasing.md). Updates are signed, and the app only installs one
// whose signature matches the public key it was built with.
import { create } from "zustand";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { DownloadEvent, Update } from "@tauri-apps/plugin-updater";
import { useLog } from "./logStore";
import { caseChangeOk, useWorkspace } from "./workspaceStore";

export type UpdatePhase =
  /** Nothing to show. */
  | "idle"
  | "checking"
  /** The check found nothing newer. */
  | "current"
  | "available"
  | "downloading"
  /** Downloaded; the app restarts into the new version. */
  | "restarting"
  | "error";

export interface UpdateInfo {
  version: string;
  current: string;
  /** The release notes (markdown), as the release page shows them. */
  notes: string;
  date: string | null;
}

interface UpdateState {
  /** Whether this build can update itself (null until asked). */
  enabled: boolean | null;
  phase: UpdatePhase;
  update: UpdateInfo | null;
  /** Bytes downloaded, and the size when the server sent it. */
  received: number;
  total: number | null;
  error: string | null;
  /** Check once at startup. Kept per machine. */
  autoCheck: boolean;
  setAutoCheck: (on: boolean) => void;
  /** A version the user chose to skip: the startup check stays quiet about it. */
  skipped: string | null;
  /**
   * Ask the release server. A quiet check (the startup one) only shows
   * something when there is a new version; failures go to the log.
   */
  check: (opts?: { quiet?: boolean }) => Promise<void>;
  /** Download and install the update found, then restart. */
  install: () => Promise<void>;
  /** Don't offer this version again at startup. */
  skip: () => void;
  dismiss: () => void;
}

const AUTO_KEY = "stannum-update-check";
const SKIP_KEY = "stannum-update-skip";
/** The releases page, for builds that can't update themselves. */
export const RELEASES_URL = "https://github.com/SklorGalore/StannumRevival/releases";
/** How long after startup the automatic check runs, so it never slows opening. */
export const STARTUP_DELAY_MS = 8000;

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Private mode or storage switched off: the choice lasts this session.
  }
}

const NOT_ENABLED =
  "This copy of Stannum can't update itself: it is a development build. Installers for the latest version are on the releases page.";

/** A check is running (the startup one shows nothing while it does). */
let checking = false;

/** The update the last check found, kept outside the store (it holds a native resource). */
let pending: Update | null = null;

const message = (e: unknown) => (e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e));

async function updatesEnabled(): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    return await invoke<boolean>("updates_enabled");
  } catch {
    return false;
  }
}

export const useUpdate = create<UpdateState>((set, get) => ({
  enabled: null,
  phase: "idle",
  update: null,
  received: 0,
  total: null,
  error: null,
  autoCheck: read(AUTO_KEY) !== "off",
  setAutoCheck: (on) => {
    write(AUTO_KEY, on ? null : "off");
    set({ autoCheck: on });
  },
  skipped: read(SKIP_KEY),

  check: async ({ quiet = false } = {}) => {
    const busy = get().phase;
    if (checking || busy === "downloading" || busy === "restarting") return;
    checking = true;
    try {
      const enabled = get().enabled ?? (await updatesEnabled());
      set({ enabled });
      if (!enabled) {
        if (!quiet) set({ phase: "error", error: NOT_ENABLED });
        return;
      }
      // The startup check runs out of sight.
      if (!quiet) set({ phase: "checking", error: null });
      const { check } = await import("@tauri-apps/plugin-updater");
      const found = await check({ timeout: 30_000 });
      await pending?.close().catch(() => {});
      pending = found;
      if (!found) {
        set(quiet ? { update: null } : { phase: "current", update: null });
        return;
      }
      const update: UpdateInfo = { version: found.version, current: found.currentVersion, notes: found.body ?? "", date: found.date ?? null };
      const hide = quiet && get().skipped === found.version;
      set(hide ? { update } : { phase: "available", update });
      useLog.getState().add("info", `Stannum ${found.version} is available (this is ${found.currentVersion}).`, "updater");
    } catch (e) {
      const error = message(e);
      useLog.getState().add(quiet ? "warn" : "error", `Checking for updates failed: ${error}`, "updater");
      if (!quiet) set({ phase: "error", error: `Couldn't check for updates: ${error}` });
    } finally {
      checking = false;
    }
  },

  install: async () => {
    if (!pending || get().phase !== "available") return;
    // Restarting closes the case like opening another would, so ask the same questions.
    if (!(await caseChangeOk(useWorkspace.getState().caseSummary))) return;
    set({ phase: "downloading", received: 0, total: null, error: null });
    const version = pending.version;
    const done = useLog.getState().begin(`Installing Stannum ${version}`);
    try {
      await pending.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === "Started") set({ total: event.data.contentLength ?? null });
        else if (event.event === "Progress") set((s) => ({ received: s.received + event.data.chunkLength }));
      });
      done({ done: `Installed Stannum ${version}` });
      set({ phase: "restarting" });
      // On Windows the installer has already closed the app by now.
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      const error = message(e);
      done({ error });
      set({ phase: "error", error: `Couldn't install Stannum ${version}: ${error}` });
    }
  },

  skip: () => {
    const version = get().update?.version ?? null;
    write(SKIP_KEY, version);
    set({ skipped: version, phase: "idle" });
  },

  dismiss: () => set({ phase: "idle", error: null }),
}));

/** The automatic check, a little after startup. Returns a cleanup for effects. */
export function startUpdateChecks(): () => void {
  if (!isTauri()) return () => {};
  const timer = setTimeout(() => {
    if (useUpdate.getState().autoCheck) void useUpdate.getState().check({ quiet: true });
  }, STARTUP_DELAY_MS);
  return () => clearTimeout(timer);
}
