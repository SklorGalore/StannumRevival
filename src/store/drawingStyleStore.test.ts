import { beforeEach, describe, expect, it } from "vitest";
import { useDrawingStyle } from "./drawingStyleStore";

describe("drawing style focus and view", () => {
  beforeEach(() => useDrawingStyle.getState().reset());

  it("keeps a path view when the network first catches up with the one-line", () => {
    useDrawingStyle.getState().showPath();
    useDrawingStyle.getState().setFocus({ bus: 4, depth: 2 });
    expect(useDrawingStyle.getState().view.kind).toBe("path");
    // The same focus again changes nothing either.
    useDrawingStyle.getState().setFocus({ bus: 4, depth: 2 });
    expect(useDrawingStyle.getState().view.kind).toBe("path");
  });

  it("goes back to the neighbourhood when the focus moves", () => {
    useDrawingStyle.getState().setFocus({ bus: 4, depth: 2 });
    useDrawingStyle.getState().showIsland(1);
    useDrawingStyle.getState().setFocus({ bus: 5, depth: 2 });
    expect(useDrawingStyle.getState().view.kind).toBe("around");
    expect(useDrawingStyle.getState().focus).toEqual({ bus: 5, depth: 2 });
  });

  it("clamps the hop count", () => {
    useDrawingStyle.getState().setFocus({ bus: 4, depth: 99 });
    expect(useDrawingStyle.getState().focus?.depth).toBe(6);
  });
});
