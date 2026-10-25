/**
 * One colour per standard base kV, so buses at different voltages read apart
 * (138 and 230 kV no longer share a colour). A case's voltage is matched to
 * the nearest standard level. Each level has a darker shade for the light
 * canvas and a brighter one for the dark canvas.
 */
export interface VoltageLevel {
  /** Standard level, in kV. */
  kv: number;
  light: string;
  dark: string;
}

// Hot to cool, high voltage to low. 138 and 69 kV keep their old colours.
export const VOLTAGE_LEVELS: VoltageLevel[] = [
  { kv: 765, light: "#9333ea", dark: "#c084fc" }, // purple
  { kv: 500, light: "#db2777", dark: "#f472b6" }, // pink
  { kv: 345, light: "#dc2626", dark: "#f87171" }, // red
  { kv: 230, light: "#ea580c", dark: "#fb923c" }, // orange
  { kv: 161, light: "#a16207", dark: "#d6a14a" }, // brown
  { kv: 138, light: "#ca8a04", dark: "#facc15" }, // yellow
  { kv: 115, light: "#65a30d", dark: "#a3e635" }, // lime
  { kv: 69, light: "#16a34a", dark: "#4ade80" }, // green
  { kv: 46, light: "#0d9488", dark: "#2dd4bf" }, // teal
  { kv: 34.5, light: "#0891b2", dark: "#22d3ee" }, // cyan
  { kv: 23, light: "#2563eb", dark: "#60a5fa" }, // blue (22 to 25 kV)
  { kv: 13.8, light: "#4f46e5", dark: "#818cf8" }, // indigo (11 to 15 kV)
  { kv: 4.16, light: "#c026d3", dark: "#e879f9" }, // fuchsia (2.4 to 7.2 kV)
  { kv: 0.48, light: "#78716c", dark: "#a8a29e" }, // stone (low voltage)
];

/** The standard level nearest to `kv`, compared on a log scale. */
export function voltageLevel(kv: number): VoltageLevel {
  let best = VOLTAGE_LEVELS[0];
  let gap = Infinity;
  for (const level of VOLTAGE_LEVELS) {
    const g = Math.abs(Math.log(kv / level.kv));
    if (g < gap) {
      gap = g;
      best = level;
    }
  }
  return best;
}

export function getVoltageColor(kv: number, dark = false): string {
  const level = voltageLevel(kv);
  return dark ? level.dark : level.light;
}
