import { create } from "zustand";
import { useLog } from "./logStore";

/** An internal error the person should hear about: a bug, not a bad input. */
export interface Problem {
  /** What was running, such as "Solving power flow" or "The Diagram view". */
  where: string;
  message: string;
  time: number;
}

interface CrashState {
  problem: Problem | null;
  /** How many internal errors this session has seen. */
  count: number;
  /**
   * Tell the person about an internal error. `log` writes it to the log too;
   * leave it off when the failure is logged already.
   */
  report: (where: string, message: string, log?: boolean) => void;
  dismiss: () => void;
}

export const useCrash = create<CrashState>((set) => ({
  problem: null,
  count: 0,
  report: (where, message, log = true) => {
    if (log) useLog.getState().add("error", `Internal error in ${where.toLowerCase()}: ${message}`);
    set((s) => ({ problem: { where, message, time: Date.now() }, count: s.count + 1 }));
  },
  dismiss: () => set({ problem: null }),
}));

/**
 * Log errors nothing caught (a failed promise or event handler), so they
 * reach Save diagnostics. They don't interrupt the person: the view that
 * raised them is still working.
 */
export function watchUncaughtErrors(): () => void {
  const onError = (e: ErrorEvent) => {
    useLog.getState().add("error", `Uncaught error: ${e.message}${e.filename ? ` (${e.filename}:${e.lineno})` : ""}`);
  };
  const onRejection = (e: PromiseRejectionEvent) => {
    // Failed core and shell calls are in the log already.
    if (e.reason instanceof Error && e.reason.name === "StannumError") return;
    const reason = e.reason instanceof Error ? e.reason.message : String(e.reason);
    useLog.getState().add("error", `Unhandled failure: ${reason}`);
  };
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}
