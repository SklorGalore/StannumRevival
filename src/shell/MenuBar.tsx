import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronRight } from "lucide-react";
import { useWorkspace } from "../store/workspaceStore";
import { useStudyFile } from "../store/studyFileStore";
import { count } from "../lib/analysis/format";
import { WindowControls, dragRegion } from "./WindowControls";
import { APP_MENUS, type MenuEntry } from "./appMenus";

/**
 * The top row: File, View, Analysis and Help, the open case's name in the
 * middle, and on Windows the window buttons (the row is the title bar there).
 */
export function MenuBar() {
  const [open, setOpen] = useState<string | null>(null);
  const [focusFirst, setFocusFirst] = useState(false);
  const bar = useRef<HTMLDivElement>(null);
  const tops = useRef<Record<string, HTMLButtonElement | null>>({});

  const openMenu = (id: string | null, byKey = false) => {
    setOpen(id);
    setFocusFirst(byKey);
  };
  const step = (from: string, by: number, byKey = true) => {
    const i = APP_MENUS.findIndex((m) => m.id === from);
    const next = APP_MENUS[(i + by + APP_MENUS.length) % APP_MENUS.length].id;
    tops.current[next]?.focus();
    if (open) openMenu(next, byKey);
  };

  return (
    <div {...dragRegion} className="h-8 shrink-0 flex items-center gap-2 pl-2 pr-4 bg-white border-b border-slate-100 whitespace-nowrap text-xs">
      <div ref={bar} role="menubar" aria-label="Main menu" className="flex items-center shrink-0">
        {APP_MENUS.map((m) => (
          <button
            key={m.id}
            ref={(el) => {
              tops.current[m.id] = el;
            }}
            type="button"
            role="menuitem"
            aria-haspopup="menu"
            aria-expanded={open === m.id}
            tabIndex={m.id === APP_MENUS[0].id ? 0 : -1}
            className={`px-2.5 py-1 rounded text-slate-700 hover:bg-slate-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/50 ${open === m.id ? "bg-slate-100" : ""}`}
            onClick={() => openMenu(open === m.id ? null : m.id)}
            // Once one menu is open, pointing at another opens it, as in a desktop menu bar.
            onPointerEnter={() => open && open !== m.id && openMenu(m.id)}
            onKeyDown={(e) => {
              if (e.key === "ArrowRight") (e.preventDefault(), step(m.id, 1));
              else if (e.key === "ArrowLeft") (e.preventDefault(), step(m.id, -1));
              else if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") (e.preventDefault(), openMenu(m.id, true));
            }}
          >
            {m.label}
          </button>
        ))}
      </div>
      <CaseTitle />
      {open && (
        <Dropdown
          key={open}
          anchor={tops.current[open]}
          entries={APP_MENUS.find((m) => m.id === open)!.items()}
          label={APP_MENUS.find((m) => m.id === open)!.label}
          focusFirst={focusFirst}
          ignore={bar.current}
          onClose={(refocus) => {
            const id = open;
            setOpen(null);
            if (refocus) tops.current[id]?.focus();
          }}
          onSide={(by) => step(open, by)}
        />
      )}
      <WindowControls />
    </div>
  );
}

/** The open case's name and size, centred in the menu bar. */
function CaseTitle() {
  const summary = useWorkspace((s) => s.caseSummary);
  const opening = useWorkspace((s) => s.opening);
  const studyOpening = useStudyFile((s) => s.busy === "opening");
  const counts = summary?.power.info?.counts;
  const topo = summary?.topology.info;
  const revision = summary?.power.info?.revision ?? (topo && !topo.derived ? topo.revision : null);
  if (!summary)
    return (
      <span role="status" className="flex-1 min-w-0 text-center text-slate-500 truncate">
        {studyOpening ? "Opening study…" : opening ? "Opening case…" : "No case open"}
      </span>
    );
  const format =
    summary.format === "matpower"
      ? "MATPOWER"
      : summary.format === "stn"
        ? /\.stnb$/i.test(summary.name)
          ? "Binary Stannum case (preview)"
          : "Stannum case (preview)"
        : `PSS/E RAW${revision ? ` v${revision}` : ""}`;
  const size = counts
    ? ` · ${count(counts.buses)} buses · ${count(counts.generators)} generators · ${count(counts.branches)} branches · ${counts.base_mva} MVA base`
    : topo
      ? ` · ${count(topo.buses)} buses · ${count(topo.equipment)} equipment`
      : "";
  return (
    <div className="flex-1 min-w-0 flex justify-center items-baseline gap-2" data-case-summary>
      <span className="min-w-0 shrink-0 max-w-[60%] font-semibold text-slate-800 truncate" title={summary.path}>
        {summary.name}
      </span>
      <span className="min-w-0 text-[11px] text-slate-500 truncate">
        {format}
        {size}
      </span>
    </div>
  );
}

/** One open menu, drawn over everything. A submenu opens to its right. */
function Dropdown({
  anchor,
  entries,
  label,
  focusFirst,
  ignore,
  onClose,
  onSide,
  nested = false,
}: {
  anchor: HTMLElement | null | undefined;
  entries: MenuEntry[];
  label: string;
  focusFirst: boolean;
  ignore?: HTMLElement | null;
  onClose: (refocus: boolean) => void;
  /** Left or right arrow at the top level: the next menu along the bar. */
  onSide?: (by: number) => void;
  nested?: boolean;
}) {
  const menu = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);
  const [sub, setSub] = useState<{ id: string; anchor: HTMLElement; byKey: boolean } | null>(null);

  useLayoutEffect(() => {
    if (!anchor) return;
    const a = anchor.getBoundingClientRect();
    const w = menu.current?.offsetWidth ?? 240;
    const h = menu.current?.offsetHeight ?? 300;
    const left = nested ? (a.right + w > window.innerWidth - 8 ? a.left - w : a.right) : a.left;
    const top = nested ? a.top - 4 : a.bottom + 2;
    setAt({ top: Math.max(8, Math.min(top, window.innerHeight - h - 8)), left: Math.max(8, Math.min(left, window.innerWidth - w - 8)) });
  }, [anchor, nested]);

  useEffect(() => {
    if (at && focusFirst) menu.current?.querySelector<HTMLButtonElement>("[role^=menuitem]:not(:disabled)")?.focus({ preventScroll: true });
  }, [at, focusFirst]);

  // A click anywhere else, or the window changing size, closes the menu.
  useEffect(() => {
    if (nested) return;
    const away = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("[data-app-menu]") || ignore?.contains(t)) return;
      onClose(false);
    };
    const resize = () => onClose(false);
    // Escape with the pointer in the menu but focus elsewhere (the menu's own keys stop it first).
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      onClose(true);
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    window.addEventListener("resize", resize);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
      window.removeEventListener("resize", resize);
    };
  }, [nested, ignore, onClose]);

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.target instanceof HTMLElement && !menu.current?.contains(e.target)) return;
    const items = [...(menu.current?.querySelectorAll<HTMLButtonElement>(":scope > [role^=menuitem]:not(:disabled)") ?? [])];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: items.length - 1 }[e.key];
    if (to !== undefined && items.length) {
      e.preventDefault();
      e.stopPropagation();
      items[(to + items.length) % items.length].focus();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose(true);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      e.stopPropagation();
      if (nested) onClose(true);
      else onSide?.(-1);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      e.stopPropagation();
      const el = document.activeElement as HTMLElement | null;
      const id = el?.dataset.submenu;
      if (id && el) setSub({ id, anchor: el, byKey: true });
      else if (!nested) onSide?.(1);
    } else if (e.key === "Tab") {
      e.preventDefault();
      onClose(true);
    }
  };

  const run = (fn: () => void) => {
    onClose(false);
    fn();
  };

  return createPortal(
    <div
      ref={menu}
      role="menu"
      aria-label={label}
      data-app-menu
      className="fixed z-[100] min-w-56 max-h-[80vh] overflow-y-auto rounded-md border border-slate-200 bg-white py-1 shadow-lg text-xs"
      style={at ?? { top: -9999, left: -9999 }}
      onKeyDown={onKeyDown}
    >
      {entries.map((entry, i) => {
        if (entry.kind === "separator") return <div key={`sep-${i}`} role="separator" className="my-1 h-px bg-slate-200" />;
        const row =
          "w-full flex items-center gap-2 px-3 py-1.5 text-left whitespace-nowrap text-slate-700 hover:bg-slate-100 focus:bg-slate-100 focus:outline-none disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent";
        if (entry.kind === "submenu")
          return (
            <button
              key={entry.id}
              type="button"
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={sub?.id === entry.id}
              data-submenu={entry.id}
              disabled={entry.disabled || entry.items.length === 0}
              tabIndex={-1}
              className={`${row} ${sub?.id === entry.id ? "bg-slate-100" : ""}`}
              onPointerEnter={(e) => setSub({ id: entry.id, anchor: e.currentTarget, byKey: false })}
              onClick={(e) => setSub({ id: entry.id, anchor: e.currentTarget, byKey: false })}
            >
              <span className="w-3.5 shrink-0" />
              <span className="flex-1">{entry.label}</span>
              <ChevronRight size={13} className="text-slate-500" />
            </button>
          );
        return (
          <button
            key={entry.id}
            type="button"
            role={entry.checked !== undefined ? "menuitemcheckbox" : "menuitem"}
            aria-checked={entry.checked}
            disabled={entry.disabled}
            title={entry.title}
            tabIndex={-1}
            className={row}
            onPointerEnter={() => setSub(null)}
            onClick={() => run(entry.run)}
          >
            <span className="w-3.5 shrink-0 flex justify-center">{entry.checked && <Check size={13} className="text-teal-700" />}</span>
            <span className="flex-1">{entry.label}</span>
            {entry.shortcut && <span className="pl-6 text-[11px] text-slate-500">{entry.shortcut}</span>}
          </button>
        );
      })}
      {sub && (
        <Dropdown
          key={sub.id}
          nested
          anchor={sub.anchor}
          label={(entries.find((e) => e.kind === "submenu" && e.id === sub.id) as Extract<MenuEntry, { kind: "submenu" }>).label}
          entries={(entries.find((e) => e.kind === "submenu" && e.id === sub.id) as Extract<MenuEntry, { kind: "submenu" }>).items}
          focusFirst={sub.byKey}
          onClose={(refocus) => {
            const a = sub.anchor;
            setSub(null);
            if (refocus) a.focus();
            else onClose(false);
          }}
        />
      )}
    </div>,
    document.body
  );
}
