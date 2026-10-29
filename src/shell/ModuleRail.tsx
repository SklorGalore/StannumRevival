import { GitCompareArrows, Monitor, Moon, PencilRuler, Sheet, Sun, Zap } from "lucide-react";
import { NEXT_PREFERENCE, useTheme } from "../store/themeStore";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import type { ReactNode } from "react";
import logoUrl from "../assets/stannum-logo.svg";
import logoSmallUrl from "../assets/stannum-logo-small.svg";
import { dragRegion } from "./WindowControls";
import { useUpdate } from "../store/updateStore";
import { shortVersion } from "../lib/version/shortVersion";

export const MODULES: { id: ModuleId; label: string; icon: ReactNode; hint: string }[] = [
  { id: "diagram", label: "Diagram", icon: <PencilRuler size={18} />, hint: "One-line and network drawings, with topology layers and tables" },
  { id: "power", label: "Power flow", icon: <Zap size={18} />, hint: "AC/DC power flow and sensitivities (rs-power)" },
  { id: "records", label: "Records", icon: <Sheet size={18} />, hint: "Every record in the open case as a table: all fields, sort and filter" },
  { id: "diff", label: "Case diff", icon: <GitCompareArrows size={18} />, hint: "Exact RAW case comparison (case-diff)" },
];

/** The Stannum mark. Below 40 px it switches to the heavier small-size drawing. */
export function Logo({ size = 28 }: { size?: number }) {
  return <img src={size < 40 ? logoSmallUrl : logoUrl} width={size} height={size} alt="" aria-hidden="true" draggable={false} />;
}

const MOD = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "⌘" : "Ctrl";

export function ModuleRail() {
  const module = useWorkspace((s) => s.module);
  const setModule = useWorkspace((s) => s.setModule);
  const split = useWorkspace((s) => s.split);
  const setSplit = useWorkspace((s) => s.setSplit);
  return (
    <nav
      aria-label="Modules"
      // The rail is dark in both themes, so it keeps the light palette's tints.
      className="force-light w-[84px] shrink-0 bg-[#0f2e30] text-teal-50 flex flex-col items-stretch py-3 gap-1 select-none"
    >
      <div {...dragRegion} className="flex flex-col items-center gap-1 pb-3 mb-1 border-b border-white/10">
        <Logo />
        <span className="text-[11px] font-semibold tracking-wide">Stannum</span>
      </div>
      {MODULES.map((m, i) => (
        <button
          key={m.id}
          type="button"
          title={`${m.hint} (${MOD}+${i + 1}). Shift+click or ${MOD}+Shift+${i + 1} to open it beside the current module.`}
          aria-keyshortcuts={`Control+${i + 1}`}
          aria-current={module === m.id || split === m.id ? "page" : undefined}
          onClick={(e) => (e.shiftKey && m.id !== module ? setSplit(m.id) : setModule(m.id))}
          className={`mx-2 flex flex-col items-center gap-1 rounded-lg py-2 text-[10.5px] font-medium transition-colors ${
            module === m.id
              ? "bg-teal-500/20 text-white"
              : split === m.id
                ? "bg-teal-500/10 text-white ring-1 ring-inset ring-teal-400/30"
                : "text-teal-100/70 hover:bg-white/5 hover:text-white"
          }`}
        >
          <span aria-hidden="true">{m.icon}</span>
          {m.label}
        </button>
      ))}
      <ThemeButton />
      <VersionButton />
    </nav>
  );
}

/** The version, at the foot of the rail. Click to check for updates; a dot means one is waiting. */
function VersionButton() {
  const update = useUpdate((s) => s.update);
  const check = useUpdate((s) => s.check);
  const waiting = !!update && update.version !== __APP_VERSION__;
  return (
    <button
      type="button"
      onClick={() => void check()}
      className="relative mx-auto rounded px-1.5 text-center text-[9px] text-teal-100/70 hover:bg-white/5 hover:text-teal-50"
      title={waiting ? `Stannum ${__APP_VERSION__}. Stannum ${update.version} is available: click to see it.` : `Stannum ${__APP_VERSION__}. Click to check for updates.`}
      aria-label={waiting ? `Stannum ${__APP_VERSION__}, update ${update.version} available` : `Stannum ${__APP_VERSION__}, check for updates`}
      data-testid="version-button"
    >
      {shortVersion(__APP_VERSION__)}
      {waiting && <span className="absolute -right-0.5 top-0 h-1.5 w-1.5 rounded-full bg-amber-400" aria-hidden="true" />}
    </button>
  );
}

const THEME_LABEL = { system: "System", light: "Light", dark: "Dark" } as const;

/** Cycles System → Light → Dark. */
function ThemeButton() {
  const preference = useTheme((s) => s.preference);
  const setPreference = useTheme((s) => s.setPreference);
  const Icon = preference === "light" ? Sun : preference === "dark" ? Moon : Monitor;
  return (
    <button
      className="mt-auto mx-2 mb-1 flex flex-col items-center gap-1 rounded-lg py-2 text-[10.5px] font-medium text-teal-100/70 hover:bg-white/5 hover:text-white"
      onClick={() => setPreference(NEXT_PREFERENCE[preference])}
      title={`Theme: ${THEME_LABEL[preference]}. Click for ${THEME_LABEL[NEXT_PREFERENCE[preference]]}.`}
      aria-label={`Theme: ${THEME_LABEL[preference]}`}
    >
      <Icon size={16} />
      {THEME_LABEL[preference]}
    </button>
  );
}
