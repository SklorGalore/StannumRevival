import { GitCompareArrows, Monitor, Moon, PencilRuler, Sun, Zap } from "lucide-react";
import { NEXT_PREFERENCE, useTheme } from "../store/themeStore";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import type { ReactNode } from "react";
import logoUrl from "../assets/stannum-logo.svg";
import logoSmallUrl from "../assets/stannum-logo-small.svg";

export const MODULES: { id: ModuleId; label: string; icon: ReactNode; hint: string }[] = [
  { id: "diagram", label: "Diagram", icon: <PencilRuler size={18} />, hint: "One-line and network drawings, with topology layers and tables" },
  { id: "power", label: "Power flow", icon: <Zap size={18} />, hint: "AC/DC power flow and sensitivities (rs-power)" },
  { id: "diff", label: "Case diff", icon: <GitCompareArrows size={18} />, hint: "Exact RAW case comparison (case-diff)" },
];

/** The Stannum mark. Below 40 px it switches to the heavier small-size drawing. */
export function Logo({ size = 28 }: { size?: number }) {
  return <img src={size < 40 ? logoSmallUrl : logoUrl} width={size} height={size} alt="" aria-hidden="true" draggable={false} />;
}

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
      <div className="flex flex-col items-center gap-1 pb-3 mb-1 border-b border-white/10">
        <Logo />
        <span className="text-[11px] font-semibold tracking-wide">Stannum</span>
      </div>
      {MODULES.map((m) => (
        <button
          key={m.id}
          title={`${m.hint}. Shift+click to open it beside the current module.`}
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
          {m.icon}
          {m.label}
        </button>
      ))}
      <ThemeButton />
      <div className="text-center text-[9px] text-teal-100/40">v0.2</div>
    </nav>
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
