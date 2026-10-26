import type { RawWarning } from "../../lib/analysis/types";

/** Import warnings that share a message. */
export interface WarningGroup {
  message: string;
  count: number;
  /** In file order. */
  warnings: RawWarning[];
  /** The elements the warnings name, without repeats. */
  elements: string[];
}

/** Group warnings by message, most frequent first (file order on ties). */
export function groupWarnings(warnings: RawWarning[]): WarningGroup[] {
  const groups = new Map<string, WarningGroup>();
  for (const w of warnings) {
    let g = groups.get(w.message);
    if (!g) groups.set(w.message, (g = { message: w.message, count: 0, warnings: [], elements: [] }));
    g.count++;
    g.warnings.push(w);
    if (w.element && !g.elements.includes(w.element)) g.elements.push(w.element);
  }
  return [...groups.values()].sort((a, b) => b.count - a.count);
}

/** "line 12", "lines 12–40" or "lines 12, 40". */
export function lineSpan(warnings: RawWarning[]): string {
  const lines = warnings.map((w) => w.line);
  const lo = Math.min(...lines);
  const hi = Math.max(...lines);
  if (lo === hi) return `line ${lo}`;
  return lines.length === 2 ? `lines ${lo}, ${hi}` : `lines ${lo}–${hi}`;
}
