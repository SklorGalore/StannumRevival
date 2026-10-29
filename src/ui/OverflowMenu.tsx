import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, MoreHorizontal } from "lucide-react";

export interface MenuItem {
  id: string;
  label: ReactNode;
  icon?: ReactNode;
  /** Shown after the label, e.g. a row count. */
  badge?: ReactNode;
  onClick: () => void;
  /** A toggle that is on, or the current choice: shown with a tick. */
  active?: boolean;
  disabled?: boolean;
  danger?: boolean;
  title?: string;
}

/**
 * A "More" button that lists what didn't fit in a narrow toolbar or tab bar.
 * The list is drawn over everything (fixed, in a portal), so a toolbar that
 * clips its overflow doesn't clip the menu.
 */
export function OverflowMenu({
  items,
  label = "More",
  showLabel = false,
  className = "",
  icon,
}: {
  items: MenuItem[];
  label?: string;
  showLabel?: boolean;
  className?: string;
  /** Drawn on the button in place of the "More" dots. */
  icon?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);
  useLayoutEffect(() => {
    if (!open || !button.current) return;
    const b = button.current.getBoundingClientRect();
    const w = menu.current?.offsetWidth ?? 220;
    setAt({ top: b.bottom + 4, left: Math.max(8, Math.min(b.right - w, window.innerWidth - w - 8)) });
  }, [open]);
  // Opened from the keyboard or not, the first item takes focus so arrows work at once.
  useEffect(() => {
    if (open && at) menu.current?.querySelector<HTMLButtonElement>("[role=menuitem]:not(:disabled)")?.focus({ preventScroll: true });
  }, [open, at]);
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!menu.current?.contains(t) && !button.current?.contains(t)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault(); // Escape closes the menu, not the selection too.
      setOpen(false);
      button.current?.focus();
    };
    const close = () => setOpen(false);
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
      window.removeEventListener("resize", close);
    };
  }, [open]);
  if (!items.length) return null;
  return (
    <>
      <button
        ref={button}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={label}
        title={label}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
        className={`shrink-0 flex items-center gap-1 px-2 py-1.5 rounded text-xs font-medium text-slate-600 hover:bg-slate-100 whitespace-nowrap ${
          open ? "bg-slate-100" : ""
        } ${className}`}
      >
        {showLabel ? (
          <>
            {label}
            <ChevronDown size={12} />
          </>
        ) : (
          (icon ?? <MoreHorizontal size={15} />)
        )}
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            role="menu"
            aria-label={label}
            className="fixed z-[100] min-w-48 max-h-[70vh] overflow-y-auto rounded-md border border-slate-200 bg-white py-1 shadow-lg text-xs"
            style={at ?? { top: -9999, left: -9999 }}
            onKeyDown={(e) => {
              const items = [...e.currentTarget.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not(:disabled)")];
              const at = items.indexOf(document.activeElement as HTMLButtonElement);
              const to = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: items.length - 1 }[e.key];
              if (to !== undefined && items.length) {
                e.preventDefault();
                items[(to + items.length) % items.length].focus();
              } else if (e.key === "Tab") {
                // The menu sits in a portal at the end of the page: Tab leaves it from the button.
                e.preventDefault();
                setOpen(false);
                button.current?.focus();
              }
            }}
          >
            {items.map((item) => (
              <button
                key={item.id}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                title={item.title}
                tabIndex={-1}
                onClick={() => {
                  setOpen(false);
                  button.current?.focus();
                  item.onClick();
                }}
                className={`w-full flex items-center gap-2 px-3 py-1.5 text-left whitespace-nowrap focus:outline-none disabled:opacity-40 disabled:cursor-not-allowed ${
                  item.danger ? "text-red-600 hover:bg-red-50 focus:bg-red-50" : "text-slate-700 hover:bg-slate-100 focus:bg-slate-100"
                }`}
              >
                <span className="w-4 shrink-0 flex justify-center text-slate-500">{item.icon}</span>
                <span className="flex-1">{item.label}</span>
                {item.badge}
                {item.active && <Check size={13} className="text-teal-700" />}
              </button>
            ))}
          </div>,
          document.body
        )}
    </>
  );
}
