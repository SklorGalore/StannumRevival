import { describe, expect, it } from "vitest";
import { rotateNode, rotateSelected } from "./rotate";
import { rotatePoint, rotateSide, rotatedSize } from "../../components/nodes/shared/SymbolFrame";
import { sideForTerminal, sideToward, tapPoint, terminalOf } from "./bus";
import type { SLDNode } from "../../store/types";

const node = (type: string, data: Record<string, unknown>, x = 100, y = 100) =>
  ({ id: "n", type, position: { x, y }, data }) as unknown as SLDNode;

describe("SymbolFrame geometry", () => {
  it("turns a terminal at the top centre round the sides", () => {
    // 40×60 symbol, terminal at (20, 0).
    expect(rotatePoint(20, 0, 40, 60, 90)).toEqual({ x: 60, y: 20 });
    expect(rotatePoint(20, 0, 40, 60, 180)).toEqual({ x: 20, y: 60 });
    expect(rotatePoint(20, 0, 40, 60, 270)).toEqual({ x: 0, y: 20 });
    expect(rotatedSize(40, 60, 90)).toEqual({ w: 60, h: 40 });
    expect(["top", "right", "bottom", "left"].map((s) => rotateSide(s as "top", 90))).toEqual([
      "right",
      "bottom",
      "left",
      "top",
    ]);
  });
});

describe("rotateNode", () => {
  it("turns a symbol about its centre", () => {
    const load = node("load", { label: "L" }, 100, 100); // 40×60, centre (120, 130)
    const r = rotateNode(load, 90);
    expect((r.data as { rotation: number }).rotation).toBe(90);
    // Now 60×40 about the same centre.
    expect(r.position).toEqual({ x: 90, y: 110 });
    expect((rotateNode(r, -90).data as { rotation: number }).rotation).toBe(0);
    expect((rotateNode(load, -90).data as { rotation: number }).rotation).toBe(270);
  });

  it("swaps a bus between horizontal and vertical about its centre", () => {
    const bus = node("bus", { length: 200, orientation: "horizontal" }, 0, 0); // centre (100, 3)
    const v = rotateNode(bus, 90);
    expect((v.data as { orientation: string }).orientation).toBe("vertical");
    expect(v.position).toEqual({ x: 97, y: -97 });
    expect(rotateNode(v, 90).position).toEqual({ x: 0, y: 0 });
  });

  it("snaps the turned position to the grid", () => {
    const load = node("load", {}, 100, 100);
    let r = rotateNode(load, 90, 20);
    expect(r.position).toEqual({ x: 100, y: 100 });
    // Four turns bring it back where it started.
    for (let i = 0; i < 3; i++) r = rotateNode(r, 90, 20);
    expect(r.position).toEqual({ x: 100, y: 100 });
    expect(rotateNode(node("transformer", {}, 100, 100), 90, 20).position).toEqual({ x: 80, y: 120 });
  });
});

describe("bus sides", () => {
  const vbus = node("bus", { length: 200, orientation: "vertical" }, 0, 0);
  it("puts a vertical bus's top side on its right", () => {
    expect(tapPoint(vbus, 40, "top")).toEqual({ x: 6, y: 40 });
    expect(tapPoint(vbus, 40, "bottom")).toEqual({ x: 0, y: 40 });
    expect(sideToward(vbus, { x: 50, y: 10 })).toBe("top");
    expect(sideToward(vbus, { x: -50, y: 10 })).toBe("bottom");
  });
  it("matches a terminal to the side of the bar it faces", () => {
    expect(sideForTerminal(false, "top")).toBe("bottom");
    expect(sideForTerminal(false, "left")).toBeNull();
    expect(sideForTerminal(true, "left")).toBe("top");
    expect(sideForTerminal(true, "right")).toBe("bottom");
  });
  it("finds a turned symbol's terminal", () => {
    const gen = node("generator", { rotation: 90 }, 0, 0); // 80×40 turned
    expect(terminalOf(gen)).toEqual({ p: { x: 80, y: 20 }, side: "right" });
  });
});

describe("rotateSelected", () => {
  it("carries symbols sitting on a turned bus round with it", () => {
    const bus = { ...node("bus", { length: 200, orientation: "horizontal", taps: [{ id: "t", x: 40, side: "bottom" }] }, 0, 0), selected: true };
    // Load hanging from the tap: terminal at (40, 6).
    const load = { ...node("load", {}, 20, 6), id: "l" };
    const far = { ...node("load", {}, 300, 300), id: "far" };
    const edges = [
      { id: "e", source: "n", sourceHandle: "t", target: "l", targetHandle: "load-top" },
      { id: "f", source: "n", sourceHandle: "t", target: "far", targetHandle: "load-top" },
    ] as never;
    const [b, l, f] = rotateSelected([bus as SLDNode, load as SLDNode, far as SLDNode], edges, 90);
    expect(b.position).toEqual({ x: 97, y: -97 });
    // The bottom tap is now on the left of the bar, 40 down from its top.
    expect((l.data as { rotation: number }).rotation).toBe(90);
    expect(terminalOf(l)).toEqual({ p: { x: 97, y: -57 }, side: "right" });
    // A symbol wired from elsewhere stays put.
    expect(f).toBe(far);
    // And back again.
    const [b2, l2] = rotateSelected([b, { ...l, measured: undefined } as SLDNode], edges, 90);
    expect((b2.data as { orientation: string }).orientation).toBe("horizontal");
    expect((l2.data as { rotation: number }).rotation).toBe(0);
    expect(l2.position).toEqual({ x: 20, y: 6 });
  });
});

describe("tap sides after a turn", () => {
  it("moves a branch tap to the side facing its far end", () => {
    const bus = { ...node("bus", { length: 200, orientation: "horizontal", taps: [{ id: "t", x: 40, side: "bottom" }] }, 0, 0), selected: true };
    // The far bus lies to the right and below: once vertical, that's the right ("top") side.
    const other = { ...node("bus", { length: 200, orientation: "horizontal" }, 300, 300), id: "o" };
    const edges = [{ id: "e", source: "n", sourceHandle: "t", target: "o" }] as never;
    const [b] = rotateSelected([bus as SLDNode, other as SLDNode], edges, 90);
    expect((b.data as { taps: { side: string }[] }).taps[0].side).toBe("top");
  });
});
