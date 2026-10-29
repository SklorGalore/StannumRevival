import { Component, type ErrorInfo, type ReactNode } from "react";
import { LifeBuoy, OctagonX, RotateCcw, Terminal, X } from "lucide-react";
import { useCrash } from "../store/crashStore";
import { useLog } from "../store/logStore";
import { saveDiagnostics } from "../lib/diagnostics/save";

const button =
  "inline-flex items-center gap-1.5 rounded-md border border-red-300 bg-white px-2.5 py-1 text-xs font-medium text-red-800 hover:bg-red-100 focus:outline-none focus:ring-2 focus:ring-red-400/50";

/** Under the case bar: an internal error happened, the app still works, and how to report it. */
export function CrashNoticeBar() {
  const problem = useCrash((s) => s.problem);
  const dismiss = useCrash((s) => s.dismiss);
  if (!problem) return null;
  return (
    <div role="alert" data-testid="crash-notice" className="shrink-0 flex items-start gap-2.5 border-b border-red-200 bg-red-50 px-4 py-2 text-xs text-red-900">
      <OctagonX size={14} className="mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="font-semibold">Something went wrong inside Stannum: {problem.where.toLowerCase()} stopped.</p>
        <p className="mt-0.5">
          The rest of the app still works, so you can carry on or save your study. Save diagnostics to report the problem; the file holds the log and versions, not your case.
        </p>
        <p className="mt-0.5 font-mono text-[11px] text-red-800/80 break-words">{problem.message}</p>
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          <button type="button" className={button} onClick={() => void saveDiagnostics()}>
            <LifeBuoy size={12} /> Save diagnostics…
          </button>
          <button type="button" className={button} onClick={() => useLog.getState().setOpen(true)}>
            <Terminal size={12} /> Show log
          </button>
        </div>
      </div>
      <button type="button" onClick={dismiss} className="shrink-0 rounded p-0.5 hover:bg-black/5" aria-label="Dismiss" title="Dismiss">
        <X size={13} />
      </button>
    </div>
  );
}

interface BoundaryProps {
  /** What the boundary holds, as the person knows it ("the Diagram view"). */
  name: string;
  /** The whole window: offer a reload instead of redrawing one view. */
  window?: boolean;
  children: ReactNode;
}

/**
 * Catches a view that fails while drawing, so the rest of the window stays
 * usable. The failed view shows what happened and can be drawn again.
 */
export class CrashBoundary extends Component<BoundaryProps, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    const where = info.componentStack?.trim().split("\n").slice(0, 4).join(" | ");
    if (where) useLog.getState().add("debug", `Where ${this.props.name} failed: ${where}`);
    useCrash.getState().report(this.props.name, error.message || String(error));
  }

  private retry = () => {
    useCrash.getState().dismiss();
    if (this.props.window) window.location.reload();
    else this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div
        role="alert"
        data-testid="crash-view"
        className={`${this.props.window ? "h-screen w-screen" : "absolute inset-0"} flex items-center justify-center bg-slate-50 p-6`}
      >
        <div className="max-w-md rounded-lg border border-red-200 bg-white p-5 text-sm text-slate-700 shadow-sm">
          <div className="flex items-center gap-2 font-semibold text-red-800">
            <OctagonX size={16} /> {this.props.name.charAt(0).toUpperCase() + this.props.name.slice(1)} stopped with an internal error
          </div>
          <p className="mt-2">
            {this.props.window
              ? "Reload the window to carry on. Your case and study files on disk are unchanged."
              : "The other views still work. Draw this one again to carry on."}{" "}
            Save diagnostics to report the problem; the file holds the log and versions, not your case.
          </p>
          <p className="mt-2 font-mono text-[11px] text-slate-500 break-words">{error.message || String(error)}</p>
          <div className="mt-3 flex gap-2">
            <button
              type="button"
              onClick={this.retry}
              className="inline-flex items-center gap-1.5 rounded-md bg-teal-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-800"
            >
              <RotateCcw size={12} /> {this.props.window ? "Reload window" : "Draw it again"}
            </button>
            <button
              type="button"
              onClick={() => void saveDiagnostics()}
              className="inline-flex items-center gap-1.5 rounded-md border border-slate-300 px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50"
            >
              <LifeBuoy size={12} /> Save diagnostics…
            </button>
          </div>
        </div>
      </div>
    );
  }
}
