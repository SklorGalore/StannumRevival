// "?" lists every keyboard shortcut in the app.
import { useEffect } from "react";
import { create } from "zustand";
import { X } from "lucide-react";
import { Modal } from "../ui/Modal";

export const useShortcuts = create<{ open: boolean; setOpen: (open: boolean) => void }>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
}));

const MOD = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "⌘" : "Ctrl";

export const SHORTCUT_GROUPS: { title: string; keys: [string, string][] }[] = [
  {
    title: "Everywhere",
    keys: [
      [`${MOD}+K`, "Find an element or run a command"],
      [`${MOD}+1 to 4`, "Diagram, Power flow, Records, Case diff"],
      [`${MOD}+Shift+1 to 4`, "Open that module side by side"],
      [`${MOD}+\``, "Show or hide the log"],
      ["Escape", "Clear the selection, or close a menu or dialog"],
      ["?", "This list"],
    ],
  },
  {
    title: "Tables",
    keys: [
      ["↑ ↓ PgUp PgDn Home End", "Move between rows"],
      ["Enter or Space", "Open the row, as a click does"],
      ["↑ on the first row", "Go to the column headers; ← → choose, Enter sorts, ↓ goes back"],
      ["F2", "Edit the row's first editable field (Records)"],
      ["Tab, Enter, Escape", "While editing: next field, keep, cancel"],
    ],
  },
  {
    title: "Tabs, menus and panes",
    keys: [
      ["← →", "Switch tabs; resize side by side panes from the divider"],
      ["↓ on a menu button", "Open the menu; ↑ ↓ move, Enter chooses"],
    ],
  },
  {
    title: "Search (Ctrl+K)",
    keys: [
      ["Enter", "Show the element in every module, or run the command"],
      ["Shift+Enter", "Draw a one-line around the element"],
    ],
  },
  {
    title: "Diagram",
    keys: [
      [`${MOD}+Z, ${MOD}+Shift+Z`, "Undo, redo"],
      [`${MOD}+C, X, V, D`, "Copy, cut, paste, duplicate"],
      [`${MOD}+A`, "Select everything"],
      [`${MOD}+S`, "Save the diagram"],
      ["Delete", "Delete the selection"],
      ["R, Shift+R", "Turn clockwise, back"],
      ["Arrows (Shift: finer)", "Nudge the selection"],
      ["Menu key or Shift+F10", "Case actions for the selected symbol"],
      ["Hold Space and drag", "Pan"],
    ],
  },
];

/** Opens on "?" outside text fields. */
export function ShortcutsDialog() {
  const open = useShortcuts((s) => s.open);
  const setOpen = useShortcuts((s) => s.setOpen);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "?" || e.ctrlKey || e.metaKey || e.altKey) return;
      const t = e.target as HTMLElement;
      if (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName)) return;
      e.preventDefault();
      useShortcuts.getState().setOpen(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  if (!open) return null;
  return (
    <Modal label="Keyboard shortcuts" onClose={() => setOpen(false)} className="w-[34rem] max-w-[calc(100vw-2rem)] max-h-[80vh] flex flex-col">
      <div className="flex items-center justify-between px-5 pt-4 pb-2">
        <h2 className="text-sm font-semibold text-slate-800">Keyboard shortcuts</h2>
        <button type="button" onClick={() => setOpen(false)} className="p-1 rounded text-slate-500 hover:bg-slate-100 hover:text-slate-800" aria-label="Close">
          <X size={16} />
        </button>
      </div>
      <div className="overflow-y-auto px-5 pb-5 flex flex-col gap-4" tabIndex={0} aria-label="Shortcut list">
        {SHORTCUT_GROUPS.map((g) => (
          <section key={g.title}>
            <h3 className="text-[11px] font-semibold tracking-wider uppercase text-slate-500 mb-1">{g.title}</h3>
            <dl className="grid grid-cols-[12rem_1fr] gap-x-4 gap-y-1 text-xs">
              {g.keys.map(([k, what]) => (
                <div key={k} className="contents">
                  <dt>
                    <kbd className="rounded border border-slate-300 bg-slate-50 px-1.5 py-px font-sans text-[11px] text-slate-700">{k}</kbd>
                  </dt>
                  <dd className="text-slate-600">{what}</dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    </Modal>
  );
}
