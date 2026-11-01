// Attach: a Python script drives this window (stannum.attach()). The script's
// core commands run on the window's own session, in Rust; this file keeps the
// window in step with what they changed and answers the script's ui.*
// commands. See stannum-core's attach.rs for the protocol.
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api } from "../lib/analysis/api";
import { decodeSolveReport } from "../lib/analysis/wire";
import type { ContingencySummary, SolveReport } from "../lib/analysis/types";
import { paletteCommands } from "../shell/CommandPalette";
import { centreBus } from "../ui/DrawAround";
import { FIRST_PAGE, useContingencies } from "./contingencyStore";
import { useTransfers } from "./transferStore";
import { usePv } from "./pvStore";
import { useLog } from "./logStore";
import { useStudy } from "./studyStore";
import { useTheme } from "./themeStore";
import { useWorkspace, type ModuleId } from "./workspaceStore";

interface AttachState {
  /** Whether scripts can attach to this window. */
  listening: boolean;
  /** Scripts attached now. */
  clients: number;
}

export const useAttach = create<AttachState>(() => ({ listening: false, clients: 0 }));

/** A failure for the script, read by stannum as its typed error. */
export class UiError extends Error {
  constructor(
    readonly kind: "invalid_arguments" | "not_found" | "no_case" | "unknown_command" | "unsupported",
    message: string
  ) {
    super(message);
  }
}

const MODULES: ModuleId[] = ["diagram", "power", "records", "diff"];

function text(args: Record<string, unknown>, key: string): string {
  const v = args[key];
  if (typeof v !== "string" || !v) throw new UiError("invalid_arguments", `${key}: expected text.`);
  return v;
}

function needCase() {
  const summary = useWorkspace.getState().caseSummary;
  if (!summary) throw new UiError("no_case", "Open a case first.");
  return summary;
}

/** What the window shows: module, selection, case and result. */
export function windowState() {
  const w = useWorkspace.getState();
  return {
    module: w.module,
    split: w.split,
    selection: w.selection?.element ?? null,
    case: w.caseSummary && { id: w.caseSummary.id, name: w.caseSummary.name, path: w.caseSummary.path },
    solved: w.solveReport !== null,
    shown: w.shown?.name ?? null,
    theme: useTheme.getState().preference,
    palette_open: w.paletteOpen,
  };
}

/** The window's ui.* commands. Each returns what the script gets back. */
export const UI_COMMANDS: Record<string, (args: Record<string, unknown>) => unknown> = {
  "ui.state": () => windowState(),
  "ui.commands": () => paletteCommands().map(({ id, title }) => ({ id, title })),
  "ui.run": (args) => {
    const id = text(args, "id");
    const command = paletteCommands().find((c) => c.id === id);
    if (!command) throw new UiError("not_found", `No Ctrl+K command "${id}" now. app.commands() lists the ones there are.`);
    command.run();
    return { id: command.id, title: command.title };
  },
  "ui.go": (args) => {
    const module = text(args, "module") as ModuleId;
    if (!MODULES.includes(module)) throw new UiError("invalid_arguments", `module: one of ${MODULES.join(", ")}.`);
    useWorkspace.getState().setModule(module);
    return windowState();
  },
  "ui.show": (args) => {
    needCase();
    const element = text(args, "element");
    useWorkspace.getState().focus(element);
    return windowState();
  },
  "ui.clear_selection": () => {
    const w = useWorkspace.getState();
    w.select(null, w.module);
    return windowState();
  },
  "ui.draw_around": (args) => {
    const summary = needCase();
    const element = text(args, "element");
    if (!summary.power.available) throw new UiError("unsupported", "Drawing around an element needs a case rs-power can read.");
    if (centreBus(element) === null) throw new UiError("invalid_arguments", `element: "${element}" isn't a bus, branch or generator id.`);
    const w = useWorkspace.getState();
    w.drawAround(element, w.module);
    return windowState();
  },
  "ui.say": (args) => {
    const message = text(args, "message");
    const level = args.level === "warn" || args.level === "error" ? args.level : "info";
    useLog.getState().add(level, message, "script");
    return null;
  },
};

/** Answer one ui.* command: a result, or an error the script can read. */
export function answerUi(command: string, args: unknown): { ok: unknown } | { error: { kind: string; message: string } } {
  const run = UI_COMMANDS[command];
  if (!run) {
    const known = Object.keys(UI_COMMANDS).join(", ");
    return { error: { kind: "unknown_command", message: `The window has no command ${command}. It has ${known}.` } };
  }
  try {
    const ok = run((args && typeof args === "object" ? args : {}) as Record<string, unknown>);
    return { ok: ok ?? null };
  } catch (e) {
    if (e instanceof UiError) return { error: { kind: e.kind, message: e.message } };
    return { error: { kind: "internal", message: e instanceof Error ? e.message : String(e) } };
  }
}

/** Commands that leave a different case (or the same one reopened). */
const CASE_CHANGES = new Set(["open_case", "open_sample", "close_case", "run_file"]);
/** Commands that change what the case holds but keep its study. */
const CASE_EDITS = new Set(["apply_record_edits", "scale_load_generation"]);
/** Commands that change the case summary's attachments or path. */
const SUMMARY_CHANGES = new Set(["load_dc_grids", "clear_dc_grids", "load_coordinates", "clear_coordinates", "save_case_as"]);

/** Bring the window up to date after a script's command changed something. */
export async function catchUp(command: string, args: Record<string, unknown>, answer: unknown): Promise<void> {
  const w = useWorkspace.getState();
  useLog.getState().add("info", `Script: ${command.replace(/_/g, " ")}`, "script");
  if (CASE_CHANGES.has(command)) {
    const summary = await api.caseSummary();
    useLog.getState().newCase();
    useWorkspace.setState((s) => ({
      caseSummary: summary,
      caseVersion: s.caseVersion + 1,
      openError: null,
      solveReport: null,
      shown: null,
      selection: null,
      drawRequest: null,
      module: s.module === "power" && summary && !summary.power.available && summary.topology.available ? "diagram" : s.module,
    }));
    useContingencies.getState().clear();
    useTransfers.getState().clear();
    usePv.getState().clear();
    // The new case version cleared the study; a run file may have left edits.
    if (summary?.power.available) await adoptStudyEdits();
    return;
  }
  if (CASE_EDITS.has(command)) {
    // Scaling answers with its report and the case after it.
    const summary = command === "scale_load_generation" ? (answer as { case: unknown }).case : answer;
    useWorkspace.setState((s) => ({
      caseSummary: summary as typeof s.caseSummary,
      caseVersion: s.caseVersion + 1,
      editedVersion: s.caseVersion + 1,
      solveReport: null,
      shown: null,
      drawRequest: null,
    }));
    return;
  }
  if (SUMMARY_CHANGES.has(command)) {
    const summary = await api.caseSummary();
    useWorkspace.setState({ caseSummary: summary });
    return;
  }
  if (command === "set_study_edits") {
    await adoptStudyEdits();
    return;
  }
  if (command === "solve_power_flow") {
    const report: SolveReport = decodeSolveReport(answer as Parameters<typeof decodeSolveReport>[0]);
    w.adoptSolve(report);
    return;
  }
  if (command === "run_contingencies") {
    const summary = answer as ContingencySummary;
    const rows = await api.contingencyRows(0, FIRST_PAGE);
    useContingencies.setState({ summary, rows, open: null, detail: null, error: null });
  }
}

/** Show the study edits the core holds now, without solving again: the
 * script solves when it wants to. */
async function adoptStudyEdits() {
  try {
    const edits = await api.studyEdits();
    useStudy.setState((s) => ({ entries: edits.map((edit) => ({ edit })), revision: s.revision + 1, fromScript: true, error: null }));
  } catch {
    // The case has no power flow model, so no edits either.
  }
}

/** Start answering scripts (desktop app only). Returns a function that stops. */
export function startAttach(): () => void {
  const stops: (() => void)[] = [];
  let cancelled = false;
  const keep = (p: Promise<() => void>) =>
    p
      .then((fn) => (cancelled ? fn() : stops.push(fn)))
      .catch(() => {
        // Events are unavailable outside Tauri: nothing can attach.
      });
  keep(listen<number>("stannum://attach-clients", (e) => useAttach.setState({ clients: e.payload })));
  keep(
    listen<{ id: number; command: string; args: unknown }>("stannum://attach-ui", (e) => {
      const { id, command, args } = e.payload;
      void invoke("attach_ui_reply", { id, ...answerUi(command, args) }).catch(() => undefined);
    })
  );
  // One change at a time, in the order the script made them.
  let queue = Promise.resolve();
  keep(
    listen<{ command: string; args: Record<string, unknown>; answer: unknown }>("stannum://attach-changed", (e) => {
      const { command, args, answer } = e.payload;
      queue = queue.then(() => catchUp(command, args ?? {}, answer)).catch((err) => {
        useLog.getState().add("warn", `Couldn't show what the script changed: ${err instanceof Error ? err.message : String(err)}`, "script");
      });
    })
  );
  invoke<{ listening: boolean; clients: number }>("attach_status")
    .then((s) => useAttach.setState({ listening: s.listening, clients: s.clients }))
    .catch(() => undefined);
  return () => {
    cancelled = true;
    stops.forEach((fn) => fn());
  };
}
