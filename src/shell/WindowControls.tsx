import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * On Windows the app hides the native title bar (see `tauri.windows.conf.json`)
 * and draws its own: the case bar becomes the drag region and carries the
 * minimize, maximize and close buttons. macOS and Linux keep the system one.
 */
export const DRAWS_TITLE_BAR =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window && /Windows/.test(navigator.userAgent);

/** Spread on an element to make it (and its non-interactive children) drag the window. */
export const dragRegion = DRAWS_TITLE_BAR ? ({ "data-tauri-drag-region": "deep" } as const) : {};

/** How long the pointer rests on maximize before the Windows 11 snap layouts open. */
const SNAP_DELAY_MS = 600;

function useWindowState() {
  const [maximized, setMaximized] = useState(false);
  const [focused, setFocused] = useState(true);
  useEffect(() => {
    if (!DRAWS_TITLE_BAR) return;
    const win = getCurrentWindow();
    let cancelled = false;
    const unlisten: (() => void)[] = [];
    const sync = () =>
      win
        .isMaximized()
        .then((m) => !cancelled && setMaximized(m))
        .catch(() => {});
    sync();
    win.onResized(sync).then((fn) => (cancelled ? fn() : unlisten.push(fn)), () => {});
    win
      .onFocusChanged(({ payload }) => setFocused(payload))
      .then((fn) => (cancelled ? fn() : unlisten.push(fn)), () => {});
    return () => {
      cancelled = true;
      unlisten.forEach((fn) => fn());
    };
  }, []);
  return { maximized, focused };
}

// Ten-pixel line drawings in the style of the Windows caption glyphs.
const Glyph = ({ d }: { d: string }) => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1" aria-hidden="true" shapeRendering="crispEdges">
    <path d={d} />
  </svg>
);
const MINIMIZE = "M0 5.5h10";
const MAXIMIZE = "M0.5 0.5h9v9h-9z";
const RESTORE = "M0.5 2.5h7v7h-7z M2.5 2.5v-2h7v7h-2";
const CLOSE = "M0 0l10 10M10 0l-10 10";

const BUTTON = "h-full w-[46px] grid place-items-center text-slate-600 transition-colors duration-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-teal-500/60";

export function WindowControls() {
  const { maximized, focused } = useWindowState();
  const snapTimer = useRef<number | undefined>(undefined);
  if (!DRAWS_TITLE_BAR) return null;
  const win = getCurrentWindow();
  const cancelSnap = () => window.clearTimeout(snapTimer.current);
  return (
    // Sits flush in the top-right corner, like the system buttons it replaces.
    <div className={`self-stretch -mr-4 ml-2 pl-2 flex shrink-0 relative before:absolute before:left-0 before:top-3 before:bottom-3 before:w-px before:bg-slate-200 ${focused ? "" : "[&_button:not(:hover)_svg]:opacity-40"}`} data-window-controls>
      <button type="button" className={`${BUTTON} hover:bg-slate-200/70 active:bg-slate-300/70`} onClick={() => void win.minimize()} title="Minimize" aria-label="Minimize">
        <Glyph d={MINIMIZE} />
      </button>
      <button
        type="button"
        className={`${BUTTON} hover:bg-slate-200/70 active:bg-slate-300/70`}
        onClick={() => {
          cancelSnap();
          void win.toggleMaximize();
        }}
        onPointerEnter={() => {
          cancelSnap();
          snapTimer.current = window.setTimeout(() => void invoke("show_snap_layouts").catch(() => {}), SNAP_DELAY_MS);
        }}
        onPointerLeave={cancelSnap}
        title={maximized ? "Restore down" : "Maximize"}
        aria-label={maximized ? "Restore down" : "Maximize"}
      >
        <Glyph d={maximized ? RESTORE : MAXIMIZE} />
      </button>
      <button
        type="button"
        className={`${BUTTON} hover:bg-[#c42b1c] hover:text-white active:bg-[#c42b1c]/80 active:text-white`}
        onClick={() => void win.close()}
        title="Close"
        aria-label="Close"
      >
        <Glyph d={CLOSE} />
      </button>
    </div>
  );
}
