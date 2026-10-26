import { describe, expect, it } from "vitest";
import { bestFold, foldGroups, hopGroups } from "./compact";

const star = (centre: number, around: number[]) => around.map((b) => ({ a: centre, b }));
const w100 = () => 100;
const rowWidth = (r: number[]) => r.length * 100;

describe("hopGroups", () => {
  it("puts a single studied bus in the middle, its neighbours shared above and below", () => {
    const { groups } = hopGroups([1, 2, 3, 4, 5], star(1, [2, 3, 4, 5]), [1], w100, true);
    expect(groups.map((g) => g.side)).toEqual(["up", "centre", "down"]);
    expect(groups[1].buses).toEqual([1]);
    expect(groups[0].buses.length).toBe(2);
    expect(groups[2].buses.length).toBe(2);
  });

  it("keeps a neighbour's own neighbours on its side", () => {
    // 1 - 2 - 4 and 1 - 3 - 5
    const links = [...star(1, [2, 3]), { a: 2, b: 4 }, { a: 3, b: 5 }];
    const { groups } = hopGroups([1, 2, 3, 4, 5], links, [1], w100, true);
    const side = (bus: number) => groups.find((g) => g.buses.includes(bus))!.side;
    expect(side(4)).toBe(side(2));
    expect(side(5)).toBe(side(3));
    expect(side(2)).not.toBe(side(3));
    // Hop 2 is furthest from the centre on both sides.
    expect(groups[0].hop).toBe(2);
    expect(groups[groups.length - 1].hop).toBe(2);
  });

  it("draws everything below a bus when asked for one side", () => {
    const { groups } = hopGroups([1, 2, 3], star(1, [2, 3]), [1], w100, false);
    expect(groups.map((g) => g.side)).toEqual(["centre", "down"]);
  });

  it("grows a branch's from-side up and its to-side down", () => {
    // 10 - 20 is the branch; 11 hangs off 10, 21 off 20.
    const links = [
      { a: 10, b: 20 },
      { a: 10, b: 11 },
      { a: 20, b: 21 },
    ];
    const { groups } = hopGroups([10, 11, 20, 21], links, [10, 20], w100, true);
    expect(groups.map((g) => [g.side, g.buses])).toEqual([
      ["up", [11]],
      ["centre", [10]],
      ["centre", [20]],
      ["down", [21]],
    ]);
  });

  it("still places buses not joined to the centre", () => {
    const { groups } = hopGroups([1, 2, 3], [{ a: 1, b: 2 }], [1], w100, true);
    expect(groups.flatMap((g) => g.buses).sort()).toEqual([1, 2, 3]);
  });
});

describe("foldGroups", () => {
  const ten = [{ side: "down" as const, hop: 1, buses: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] }];

  it("shares a hop evenly between rows, in order", () => {
    const rows = foldGroups(ten, w100, 300);
    expect(rows.length).toBe(4);
    expect(rows.flat()).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(Math.max(...rows.map(rowWidth))).toBeLessThanOrEqual(300);
  });

  it("stacks rows above the centre upwards, nearest first", () => {
    const rows = foldGroups([{ side: "up", hop: 1, buses: [1, 2, 3, 4] }], w100, 200);
    expect(rows).toEqual([
      [3, 4],
      [1, 2],
    ]);
  });
});

describe("bestFold", () => {
  const group = [{ side: "down" as const, hop: 1, buses: Array.from({ length: 30 }, (_, i) => i + 1) }];

  it("folds a long hop row towards the view's shape", () => {
    const fit = bestFold(group, w100, 1.6, 240);
    expect(fit.rows.length).toBeGreaterThan(1);
    const ratio = fit.width / fit.height;
    expect(ratio).toBeGreaterThan(0.8);
    expect(ratio).toBeLessThan(3.2);
  });

  it("folds more for a tall, narrow view", () => {
    expect(bestFold(group, w100, 0.6, 240).rows.length).toBeGreaterThan(bestFold(group, w100, 2.4, 240).rows.length);
  });

  it("leaves a drawing that already fits unfolded", () => {
    const small = [{ side: "down" as const, hop: 1, buses: [1, 2, 3] }];
    expect(bestFold(small, w100, 1.6, 240).rows.length).toBe(1);
  });
});
