import type { BusTap } from "../../../store/types";

/** Pitch of the drawing grid that default taps sit on. Bars start on the
 *  grid, so whole steps along the bar are grid points. */
const TAP_STEP = 20;
const onStep = (x: number, width: number) =>
  Math.min(Math.floor(width / TAP_STEP) * TAP_STEP, Math.max(0, Math.round(x / TAP_STEP) * TAP_STEP));

/** Taps for hand-drawn buses: one on top, four below, on whole grid steps so
 *  a lengthened bar doesn't move them off the grid wires run on. */
export function defaultTaps(width: number): BusTap[] {
  return [
    { id: "bus-top", x: onStep(width / 2, width), side: "top" },
    ...[0.2, 0.4, 0.6, 0.8].map((f, i) => ({ id: `bus-tap-${i}`, x: onStep(width * f, width), side: "bottom" as const })),
  ];
}

/** Every handle that already sits on a bus, including the end handles. */
export function busTaps(data: { length: number; taps?: BusTap[] }): BusTap[] {
  return data.taps ?? defaultTaps(data.length ?? 200);
}
