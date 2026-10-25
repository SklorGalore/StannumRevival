import type { BusData, BusTap, Rotation, SLDNode } from "../../store/types";
import { symbolSize } from "./sizes";

/** Thickness of a bus bar. */
export const BAR = 6;

type Point = { x: number; y: number };
export type Side = "top" | "right" | "bottom" | "left";

export const isVertical = (data: BusData) => !data.star && data.orientation === "vertical";

/** Distance along the bar from its start (left end, or top of a vertical bus). */
export function along(bus: SLDNode, p: Point): number {
  return isVertical(bus.data as BusData) ? p.y - bus.position.y : p.x - bus.position.x;
}

/** Offset from the bar's centre line: down for a horizontal bus, right for a
 *  vertical one. Positive is the side a vertical bus calls "top". */
export function across(bus: SLDNode, p: Point): number {
  return isVertical(bus.data as BusData)
    ? p.x - (bus.position.x + BAR / 2)
    : p.y - (bus.position.y + BAR / 2);
}

/** How far `p` lies out from the bar's centre line on `side`. */
export function outward(bus: SLDNode, p: Point, side: BusTap["side"]): number {
  const positive = isVertical(bus.data as BusData) ? side === "top" : side === "bottom";
  return across(bus, p) * (positive ? 1 : -1);
}

/** The side of the bar that faces `p`. */
export function sideToward(bus: SLDNode, p: Point): BusTap["side"] {
  const d = across(bus, p);
  return isVertical(bus.data as BusData) ? (d > 0 ? "top" : "bottom") : d > 0 ? "bottom" : "top";
}

/** Where a tap at `x` along the bar on `side` sits, in diagram coordinates. */
export function tapPoint(bus: SLDNode, x: number, side: BusTap["side"]): Point {
  const { x: px, y: py } = bus.position;
  if (isVertical(bus.data as BusData)) return { x: side === "top" ? px + BAR : px, y: py + x };
  return { x: px + x, y: side === "top" ? py : py + BAR };
}

/** Screen side a tap faces. */
export function tapFacing(vertical: boolean, side: BusTap["side"]): Side {
  if (!vertical) return side;
  return side === "top" ? "right" : "left";
}

/** Which side of a bar a symbol's terminal on `terminal` side can sit on:
 *  the terminal must face the bar. Null when the bar runs the wrong way. */
export function sideForTerminal(vertical: boolean, terminal: Side): BusTap["side"] | null {
  if (vertical) return terminal === "left" ? "top" : terminal === "right" ? "bottom" : null;
  return terminal === "top" ? "bottom" : terminal === "bottom" ? "top" : null;
}

/** The point and side of a one-terminal symbol's terminal, from its box. The
 *  terminal sits mid-way along the top of the upright symbol. */
export function terminalOf(n: SLDNode): { p: Point; side: Side } {
  const { w, h } = symbolSize(n);
  const r = ((n.data as { rotation?: Rotation }).rotation ?? 0) as Rotation;
  const { x, y } = n.position;
  switch (r) {
    case 90:
      return { p: { x: x + w, y: y + h / 2 }, side: "right" };
    case 180:
      return { p: { x: x + w / 2, y: y + h }, side: "bottom" };
    case 270:
      return { p: { x, y: y + h / 2 }, side: "left" };
    default:
      return { p: { x: x + w / 2, y }, side: "top" };
  }
}

/** Node position that puts the terminal of a w×h symbol at `p`. */
export function placeTerminal(p: Point, side: Side, w: number, h: number): Point {
  switch (side) {
    case "right":
      return { x: p.x - w, y: p.y - h / 2 };
    case "bottom":
      return { x: p.x - w / 2, y: p.y - h };
    case "left":
      return { x: p.x, y: p.y - h / 2 };
    default:
      return { x: p.x - w / 2, y: p.y };
  }
}
