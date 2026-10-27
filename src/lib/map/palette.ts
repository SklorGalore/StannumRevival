// The map's colours. The outlines take the shell's own colour tokens, so they
// follow the light and dark themes and sit quietly under the network: kV
// colours, results and the teal selection stay the brightest things on it.
import type { Ink } from "../diagram/ink";

export interface MapPalette {
  water: string;
  land: string;
  states: string;
  countries: string;
  /** Lines and dots outside the focus neighbourhood are drawn at this opacity. */
  dimmed: number;
  ink: Ink;
}

let probe: CanvasRenderingContext2D | null = null;

/**
 * A CSS colour (a token's oklch() value, say) as rgb(), which every WebGL
 * map style parser reads. Read back from a 1×1 canvas so the browser does
 * the conversion.
 */
export function toRgb(css: string, fallback: string): string {
  if (!css) return fallback;
  try {
    probe ??= document.createElement("canvas").getContext("2d", { willReadFrequently: true });
    if (!probe) return fallback;
    probe.clearRect(0, 0, 1, 1);
    probe.fillStyle = fallback;
    probe.fillStyle = css;
    probe.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
    return a === 0 ? fallback : `rgb(${r}, ${g}, ${b})`;
  } catch {
    return fallback;
  }
}

/** The current value of a shell colour token, such as `--color-slate-100`. */
function token(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return toRgb(value, fallback);
}

/** Colours for the current theme, print style or not. */
export function mapPalette(ink: Ink, dark: boolean): MapPalette {
  if (ink.mono) {
    // Print style: black network on white, outlines in light grey.
    return { water: "#ffffff", land: "#ffffff", states: "#d4d4d4", countries: "#a3a3a3", dimmed: 0.35, ink };
  }
  // In the dark theme the tokens are already the dark values (see index.css);
  // water sits a step darker than land in both themes.
  return dark
    ? {
        water: token("--color-slate-50", "#0b1220"),
        land: token("--color-white", "#141c2e"),
        states: token("--color-slate-200", "#273449"),
        countries: token("--color-slate-300", "#3a475d"),
        dimmed: 0.28,
        ink,
      }
    : {
        water: token("--color-slate-100", "#f1f5f9"),
        land: token("--color-white", "#ffffff"),
        states: token("--color-slate-200", "#e2e8f0"),
        countries: token("--color-slate-300", "#cbd5e1"),
        dimmed: 0.3,
        ink,
      };
}
