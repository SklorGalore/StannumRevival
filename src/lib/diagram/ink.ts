// Colours for drawing symbols, in colour or in black and white for print.
import { useDiagramStore } from "../../store/diagramStore";
import { useTheme } from "../../store/themeStore";
import { getVoltageColor } from "../../components/nodes/shared/voltageColor";
import { RESULT_TEXT } from "./results";

/** One stroke weight for every symbol, so the drawing reads as one set. */
export const STROKE = 1.5;
export const SELECTED = "#3b82f6";

export interface Ink {
  mono: boolean;
  /** Symbol outlines and stems. */
  line: string;
  /** Names and labels. */
  text: string;
  /** Ratings and secondary text. */
  muted: string;
  /** Power flow numbers. */
  result: string;
  /** Out of service equipment. */
  off: string;
  /** Wires while power flow results are shown (neutral, so loading stands out). */
  wire: string;
  /** Colour of a conductor or bus at this voltage. */
  voltage: (kv: number) => string;
  /** Warning colour (overload, violation), or plain ink in print mode. */
  alert: (color: string) => string;
}

const COLOR: Ink = {
  mono: false,
  line: "#1e293b",
  text: "#475569",
  muted: "#64748b",
  result: RESULT_TEXT,
  off: "#94a3b8",
  wire: "#334155",
  voltage: (kv) => (kv > 0 ? getVoltageColor(kv) : "#475569"),
  alert: (c) => c,
};

/** The colour palette on the dark theme's canvas. */
const DARK: Ink = {
  mono: false,
  line: "#e2e8f0",
  text: "#cbd5e1",
  muted: "#94a3b8",
  result: "#5eead4",
  off: "#64748b",
  wire: "#94a3b8",
  voltage: (kv) => (kv > 0 ? getVoltageColor(kv, true) : "#cbd5e1"),
  alert: (c) => c,
};

const MONO: Ink = {
  mono: true,
  line: "#000000",
  text: "#000000",
  muted: "#333333",
  result: "#000000",
  off: "#808080",
  wire: "#000000",
  voltage: () => "#000000",
  alert: () => "#000000",
};

/** Whether the diagram is drawn on the dark canvas: dark theme, not print style, not exporting. */
export function useDarkCanvas(): boolean {
  const mono = useDiagramStore((s) => s.colorMode === "mono");
  const dark = useTheme((s) => s.theme === "dark" && !s.exportLight);
  return dark && !mono;
}

export function useInk(): Ink {
  const mono = useDiagramStore((s) => s.colorMode === "mono");
  const dark = useDarkCanvas();
  return mono ? MONO : dark ? DARK : COLOR;
}
