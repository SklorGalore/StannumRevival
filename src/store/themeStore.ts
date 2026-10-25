import { create } from "zustand";

export type ThemePreference = "system" | "light" | "dark";
export type Theme = "light" | "dark";

const KEY = "stannum-theme";

interface ThemeState {
  preference: ThemePreference;
  setPreference: (p: ThemePreference) => void;
  /** What the system asks for, kept current. */
  systemDark: boolean;
  /** The theme in use. */
  theme: Theme;
  /** Set while exporting the diagram, which is always drawn light and in full detail. */
  exportLight: boolean;
  setExportLight: (on: boolean) => void;
}

const media = typeof window !== "undefined" && window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;

function load(): ThemePreference {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

/** Next preference for the rail's theme button. */
export const NEXT_PREFERENCE: Record<ThemePreference, ThemePreference> = { system: "light", light: "dark", dark: "system" };

const resolve = (p: ThemePreference, systemDark: boolean): Theme => (p === "system" ? (systemDark ? "dark" : "light") : p);

export const useTheme = create<ThemeState>((set, get) => {
  const preference = load();
  const systemDark = !!media?.matches;
  return {
    preference,
    systemDark,
    theme: resolve(preference, systemDark),
    exportLight: false,
    setExportLight: (exportLight) => set({ exportLight }),
    setPreference: (p) => {
      try {
        localStorage.setItem(KEY, p);
      } catch {
        // Storage may be unavailable; the choice still holds for this session.
      }
      set({ preference: p, theme: resolve(p, get().systemDark) });
    },
  };
});

media?.addEventListener("change", (e) =>
  useTheme.setState((s) => ({ systemDark: e.matches, theme: resolve(s.preference, e.matches) }))
);

/** Keep the `dark` class on <html> in step with the theme. */
function apply(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
}
if (typeof document !== "undefined") {
  apply(useTheme.getState().theme);
  useTheme.subscribe((s) => apply(s.theme));
}
