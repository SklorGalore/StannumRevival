import { useEffect, useRef, useState, type ReactNode } from "react";

const FOCUSABLE =
  'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

/** The parts of `root` that Tab can reach, in order. */
export function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => !el.closest("[inert]") && el.getClientRects().length > 0);
}

/**
 * A dialog over the whole window. Focus moves into it, Tab and Shift+Tab stay
 * inside, Escape (or a click on the dimmed backdrop) closes it, and focus goes
 * back to where it was when it closes.
 */
export function Modal({
  label,
  onClose,
  children,
  className = "",
}: {
  /** Read out when the dialog opens. */
  label: string;
  onClose: () => void;
  children: ReactNode;
  className?: string;
}) {
  const box = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  // Read while rendering: a field with autoFocus takes focus before effects run.
  const [before] = useState(() => document.activeElement as HTMLElement | null);
  useEffect(() => {
    const el = box.current;
    // The first field, or the first button, unless something asked for focus itself.
    if (el && !el.contains(document.activeElement)) {
      const all = focusables(el);
      (all.find((f) => f.matches("input, select, textarea")) ?? all[0] ?? el).focus();
    }
    return () => {
      if (before?.isConnected) before.focus({ preventScroll: true });
    };
  }, [before]);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 whitespace-normal" onClick={() => close.current()}>
      <div
        ref={box}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
        className={`bg-white rounded-lg shadow-xl focus:outline-none ${className}`}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close.current();
            return;
          }
          if (e.key !== "Tab") return;
          const all = focusables(e.currentTarget);
          if (!all.length) return e.preventDefault();
          const first = all[0];
          const last = all[all.length - 1];
          if (e.shiftKey && (document.activeElement === first || document.activeElement === e.currentTarget)) {
            e.preventDefault();
            last.focus();
          } else if (!e.shiftKey && document.activeElement === last) {
            e.preventDefault();
            first.focus();
          }
        }}
      >
        {children}
      </div>
    </div>
  );
}
