import type { BusTap } from "../../../store/types";

/** Taps for hand-drawn buses: one on top, four below. */
export function defaultTaps(width: number): BusTap[] {
  return [
    { id: "bus-top", x: width / 2, side: "top" },
    ...[0.2, 0.4, 0.6, 0.8].map((f, i) => ({ id: `bus-tap-${i}`, x: width * f, side: "bottom" as const })),
  ];
}

/** Every handle that already sits on a bus, including the end handles. */
export function busTaps(data: { length: number; taps?: BusTap[] }): BusTap[] {
  return data.taps ?? defaultTaps(data.length ?? 200);
}
