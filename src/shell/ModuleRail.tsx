import { GitCompareArrows, Monitor, Moon, Network, PencilRuler, Sun, Zap } from "lucide-react";
import { NEXT_PREFERENCE, useTheme } from "../store/themeStore";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import type { ReactNode } from "react";

const MODULES: { id: ModuleId; label: string; icon: ReactNode; hint: string }[] = [
  { id: "diagram", label: "Diagram", icon: <PencilRuler size={18} />, hint: "Single-line diagram editor" },
  { id: "power", label: "Power flow", icon: <Zap size={18} />, hint: "AC/DC power flow and sensitivities (rs-power)" },
  { id: "topology", label: "Topology", icon: <Network size={18} />, hint: "Islands, critical connections and paths (nodal)" },
  { id: "diff", label: "Case diff", icon: <GitCompareArrows size={18} />, hint: "Exact RAW case comparison (case-diff)" },
];

export function Logo({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <rect width="512" height="512" rx="104" fill="#0f3d3e" />
      <g fill="none" stroke="#8fd3c9" strokeWidth="22" strokeLinecap="round">
        <path d="M136 152h240M136 360h240M196 152v208M316 152v208" />
      </g>
      <circle cx="196" cy="256" r="30" fill="#f2b84b" />
      <circle cx="316" cy="256" r="30" fill="#f2b84b" />
    </svg>
  );
}

export function ModuleRail() {
  const module = useWorkspace((s) => s.module);
  const setModule = useWorkspace((s) => s.setModule);
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
          title={m.hint}
          aria-current={module === m.id ? "page" : undefined}
          onClick={() => setModule(m.id)}
          className={`mx-2 flex flex-col items-center gap-1 rounded-lg py-2 text-[10.5px] font-medium transition-colors ${
            module === m.id ? "bg-teal-500/20 text-white" : "text-teal-100/70 hover:bg-white/5 hover:text-white"
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
