// Shape of the three-winding transformer symbol, shared by the node and the
// layout (which needs its terminals as fixed ports).
import type { Rotation } from "../../store/types";

const R = 16;
const left = { x: 27, y: 60 };

export const T3 = {
  w: 80,
  h: 100,
  r: R,
  circles: { top: { x: 40, y: 38 }, left, right: { x: 53, y: 60 } },
  /** Where the bottom stems (x = 20 and x = 60) meet their circles. */
  legY: left.y + Math.sqrt(R * R - (left.x - 20) ** 2),
};

export type T3Handle = "x3-a" | "x3-b" | "x3-c";

/** Terminals of the upright symbol: one on top, two below, on the 20 px grid. */
export const T3_TERMINALS: { id: T3Handle; x: number; y: number; side: "top" | "bottom" }[] = [
  { id: "x3-a", x: 40, y: 0, side: "top" },
  { id: "x3-b", x: 20, y: 100, side: "bottom" },
  { id: "x3-c", x: 60, y: 100, side: "bottom" },
];

/** A terminal of the symbol turned upright (0) or upside down (180). */
export function t3Terminal(id: T3Handle, rotation: Rotation = 0): { x: number; y: number; up: boolean } {
  const t = T3_TERMINALS.find((t) => t.id === id)!;
  return rotation === 180 ? { x: T3.w - t.x, y: T3.h - t.y, up: t.side === "bottom" } : { x: t.x, y: t.y, up: t.side === "top" };
}
