// The history: every command that changed the case or wrote a file, kept as
// it happens, in run file form. Copy or save any part of it as a run file;
// nothing has to be started first.
import { create } from "zustand";
import { api, DEFAULT_SOLVE_SETTINGS, pickSavePath, RUN_FILE_FILTERS, toStannumError, watchCoreCalls } from "../lib/analysis/api";
import { ACTION_COMMANDS, type CoreCommand, type Step } from "../lib/analysis/commands.gen";
import { useLog } from "./logStore";
import { useWorkspace } from "./workspaceStore";

/** Steps kept; the oldest go first. */
const LIMIT = 5000;

const ACTIONS = new Set<CoreCommand>(ACTION_COMMANDS);

/** Setters: calling one again with the same arguments changes nothing. */
const SETTERS = new Set<CoreCommand>(["set_study_edits", "topology_set_merge"]);

/** Commands that start from a case of their own. */
const OPENERS = new Set<string>(["open_case", "open_sample"]);

export interface HistoryEntry {
  id: number;
  /** Milliseconds since 1970. */
  time: number;
  command: CoreCommand;
  /** The arguments, without `case`: a run file works on whatever case is open. */
  args: Record<string, unknown>;
  /** The case file open when the command ran, so a part of the history can start from it. */
  casePath: string | null;
}

/**
 * Add one call to the history. Only actions are kept, `case` is left out,
 * and a setter called again with the same arguments as last time is skipped.
 */
export function withCall(
  entries: HistoryEntry[],
  command: CoreCommand,
  args: Record<string, unknown>,
  at: { id: number; time: number; casePath: string | null }
): HistoryEntry[] {
  if (!ACTIONS.has(command)) return entries;
  const { case: _case, ...rest } = args;
  if (SETTERS.has(command)) {
    let last: HistoryEntry | undefined;
    for (let i = entries.length - 1; i >= 0 && !last; i--) if (entries[i].command === command) last = entries[i];
    if (last && JSON.stringify(last.args) === JSON.stringify(rest)) return entries;
  }
  const casePath = command === "open_case" && typeof rest.path === "string" ? rest.path : at.casePath;
  const next = [...entries, { id: at.id, time: at.time, command, args: rest, casePath }];
  return next.length > LIMIT ? next.slice(next.length - LIMIT) : next;
}

/**
 * Entries as run file steps. A part that doesn't open a case first starts
 * by opening the case that was open when its first step ran.
 */
export function historySteps(entries: HistoryEntry[]): Step[] {
  const steps: Step[] = entries.map((e) => ({ command: e.command, args: e.args }));
  const first = entries[0];
  if (first && !OPENERS.has(first.command) && first.casePath) steps.unshift({ command: "open_case", args: { path: first.casePath } });
  return steps;
}

/** Solve settings that differ from the defaults; the rest go unsaid, as in a saved run file. */
function changedSettings(settings: unknown): Record<string, unknown> | null {
  if (!settings || typeof settings !== "object") return null;
  const defaults = DEFAULT_SOLVE_SETTINGS as unknown as Record<string, unknown>;
  const changed = Object.entries(settings as Record<string, unknown>).filter(([k, v]) => JSON.stringify(v) !== JSON.stringify(defaults[k]));
  return changed.length ? Object.fromEntries(changed) : null;
}

/** One step as a short line for the list: `solve_power_flow settings={…}`. */
export function stepLine(entry: Pick<HistoryEntry, "command" | "args">): string {
  const parts = [entry.command as string];
  const solves = entry.command === "solve_power_flow" || entry.command === "calculate_sensitivities";
  for (const [key, given] of Object.entries(entry.args)) {
    const value = solves && key === "settings" ? changedSettings(given) : given;
    if (value === null || value === undefined) continue;
    const text = typeof value === "string" && /^[^\s"#[{]+$/.test(value) ? value : JSON.stringify(value);
    parts.push(`${key}=${text}`);
  }
  return parts.join(" ");
}

interface HistoryState {
  entries: HistoryEntry[];
  /** Ids of the entries picked for Copy and Save; none picked means all. */
  picked: Set<number>;
  error: string | null;
  toggle: (id: number, range?: boolean) => void;
  pickAll: (on: boolean) => void;
  clear: () => void;
  /** The picked entries (or all) as run file text, on the clipboard. */
  copy: () => Promise<boolean>;
  /** The picked entries (or all) as a run file, asking where. */
  save: () => Promise<boolean>;
}

let nextId = 1;
let anchor: number | null = null;

/** The entries Copy and Save use: the picked ones, or all when none are. */
export function chosen(s: Pick<HistoryState, "entries" | "picked">): HistoryEntry[] {
  return s.picked.size === 0 ? s.entries : s.entries.filter((e) => s.picked.has(e.id));
}

export const useHistory = create<HistoryState>((set, get) => ({
  entries: [],
  picked: new Set(),
  error: null,
  toggle: (id, range) =>
    set((s) => {
      const picked = new Set(s.picked);
      const at = s.entries.findIndex((e) => e.id === id);
      const from = range && anchor !== null ? s.entries.findIndex((e) => e.id === anchor) : -1;
      if (from !== -1 && at !== -1) {
        // Shift-click picks everything between the last click and this one.
        const [lo, hi] = from < at ? [from, at] : [at, from];
        for (let i = lo; i <= hi; i++) picked.add(s.entries[i].id);
      } else if (picked.has(id)) picked.delete(id);
      else picked.add(id);
      anchor = id;
      return { picked };
    }),
  pickAll: (on) => set((s) => ({ picked: on ? new Set(s.entries.map((e) => e.id)) : new Set() })),
  clear: () => {
    anchor = null;
    set({ entries: [], picked: new Set(), error: null });
  },
  copy: async () => {
    const entries = chosen(get());
    if (entries.length === 0) return false;
    try {
      await navigator.clipboard.writeText(await api.runFileText(historySteps(entries)));
      set({ error: null });
      return true;
    } catch (e) {
      set({ error: `Couldn't copy the history: ${toStannumError(e).message}` });
      return false;
    }
  },
  save: async () => {
    const entries = chosen(get());
    if (entries.length === 0) return false;
    const summary = useWorkspace.getState().caseSummary;
    const base = summary ? summary.name.replace(/\.(raw|m|stnb?)$/i, "") : "history";
    const path = await pickSavePath(`${base}.stnrun`, RUN_FILE_FILTERS);
    if (!path) return false;
    try {
      await api.saveRunFile(path, historySteps(entries));
      set({ error: null });
      useLog.getState().add("info", `Saved ${entries.length} step${entries.length === 1 ? "" : "s"} of the history as ${path}`);
      return true;
    } catch (e) {
      set({ error: `Couldn't save the run file: ${toStannumError(e).message}` });
      return false;
    }
  },
}));

// Always on: every action the app sends is kept.
watchCoreCalls((command, args) => {
  const casePath = useWorkspace.getState().caseSummary?.path ?? null;
  useHistory.setState((s) => {
    const entries = withCall(s.entries, command, args, { id: nextId, time: Date.now(), casePath });
    if (entries === s.entries) return s;
    nextId++;
    return { entries };
  });
});
