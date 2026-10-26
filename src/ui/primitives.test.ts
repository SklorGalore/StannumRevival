import { describe, expect, it } from "vitest";
import { fitTabs } from "./primitives";

describe("fitTabs", () => {
  it("keeps every tab when they fit", () => {
    expect(fitTabs([100, 100, 100], 0, 300, 70)).toEqual([true, true, true]);
  });
  it("moves the tabs after the first that doesn't fit into More, in order", () => {
    expect(fitTabs([100, 100, 60, 100], 0, 300, 70)).toEqual([true, true, false, false]);
  });
  it("always keeps the open tab", () => {
    expect(fitTabs([100, 100, 100, 100], 3, 300, 70)).toEqual([true, false, false, true]);
  });
});
