// AC solution methods as the solve settings offer them.
import type { SolveReport, SolveSettings } from "./types";

export type AcMethod = SolveSettings["method"];

export const AC_METHODS: { value: AcMethod; label: string; about: string }[] = [
  {
    value: "auto",
    label: "Auto",
    about:
      "Cheap decoupled iterations first, finishing with full Newton when they slow down or fail. Newton alone on cases under 2,000 buses.",
  },
  {
    value: "newton",
    label: "Full Newton",
    about: "Newton–Raphson with the whole Jacobian rebuilt and factored at every iteration. The most robust method.",
  },
  {
    value: "fixed_slope",
    label: "Fixed-slope decoupled (FDNS)",
    about:
      "Decoupled Newton with the angle and magnitude blocks of the Jacobian taken once per pass and reused. Includes DC converter and remote regulation terms.",
  },
  {
    value: "fast_decoupled_xb",
    label: "Fast decoupled (XB)",
    about: "Stott–Alsac fast decoupled load flow: constant matrices from the network, resistance left out of the angle matrix.",
  },
  {
    value: "fast_decoupled_bx",
    label: "Fast decoupled (BX)",
    about: "Fast decoupled load flow with resistance left out of the magnitude matrix instead; can suit high R/X networks better.",
  },
];

/** Reports saved before there was a choice were all Newton. */
export const methodLabel = (m: AcMethod | undefined) => AC_METHODS.find((x) => x.value === (m ?? "newton"))?.label ?? m;

/** "11 decoupled + 3 Newton", "11 decoupled", or "" when all were Newton. */
export function iterationSplit(report: Pick<SolveReport, "iterations" | "decoupled_iterations">): string {
  const decoupled = report.decoupled_iterations ?? 0;
  const newton = report.iterations - decoupled;
  if (decoupled === 0) return "";
  return newton > 0 ? `${decoupled} decoupled + ${newton} Newton` : `${decoupled} decoupled`;
}

/** How the overview describes the switched shunts and taps after a solve. */
export function controlsDetail(
  report: Pick<SolveReport, "voltage_control_passes" | "shunts_locked" | "taps_locked">,
): string {
  if (report.shunts_locked && report.taps_locked) return "Locked; counted from their case values";
  const locked = report.shunts_locked ? "shunts locked" : report.taps_locked ? "taps locked" : "";
  const moved = report.voltage_control_passes
    ? `Moved, over ${report.voltage_control_passes} switching pass${report.voltage_control_passes === 1 ? "" : "es"}`
    : "Moved from their case values";
  return locked ? `${moved}; ${locked}` : moved;
}

/** Tooltip for a voltage control that can be locked. */
export const LOCKED_HINT = (what: string, value: string) =>
  `Off: ${what.toLowerCase()} are locked. Each keeps its ${value} where it is, which is where the last solve left it unless Flat start is ticked, and at its case value then.`;
