// Small in-cell graphics for result tables, so the eye finds the heavy
// branches and the buses near their limits without reading every number.
import type { ReactNode } from "react";

const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));

/** Where a value sits on a track from lo to hi, as a fraction. */
export function trackPosition(value: number, lo: number, hi: number): number {
  return hi > lo ? clamp((value - lo) / (hi - lo), 0, 1) : 0.5;
}

/** Loading as a bar filling the cell behind the number. */
export function LoadingBar({ pct, children }: { pct: number; children: ReactNode }) {
  const tone = pct > 100 ? "bg-red-200" : pct > 90 ? "bg-amber-200" : "bg-slate-200";
  return (
    <span className="relative flex items-center justify-end h-[18px] min-w-[72px] rounded-sm overflow-hidden" data-testid="loading-bar">
      <span className={`absolute inset-y-0 left-0 ${tone}`} style={{ width: `${clamp(pct, 0, 100)}%` }} aria-hidden="true" />
      <span className="relative px-1">{children}</span>
    </span>
  );
}

/**
 * A value against its limits: a short track with the allowed band shaded and
 * a mark where the value sits. The track runs a margin past each limit, so a
 * value just outside still shows where it is.
 */
export function RangeMark({
  value,
  lo,
  hi,
  margin,
  outside,
  label,
}: {
  value: number;
  lo: number;
  hi: number;
  /** How far past each limit the track runs. */
  margin: number;
  outside: boolean;
  label: string;
}) {
  if (!(hi > lo)) return null;
  const a = lo - margin;
  const b = hi + margin;
  const at = trackPosition(value, a, b);
  const band = [trackPosition(lo, a, b), trackPosition(hi, a, b)];
  return (
    <span className="relative inline-block w-10 h-2 rounded-full bg-slate-100 align-middle" title={label} data-testid="range-mark">
      <span
        className="absolute inset-y-0 bg-teal-100"
        style={{ left: `${band[0] * 100}%`, width: `${(band[1] - band[0]) * 100}%` }}
        aria-hidden="true"
      />
      <span
        className={`absolute -top-0.5 h-3 w-[3px] -ml-[1.5px] rounded-sm ${outside ? "bg-red-600" : "bg-slate-600"}`}
        style={{ left: `${at * 100}%` }}
        aria-hidden="true"
      />
    </span>
  );
}
