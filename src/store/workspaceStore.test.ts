import { beforeEach, describe, expect, it } from "vitest";
import { useWorkspace } from "./workspaceStore";

describe("split view", () => {
  beforeEach(() => useWorkspace.setState({ module: "diagram", split: null, linked: false, splitLinked: false, selection: null }));

  it("links selection while open, and unlinks on close when it linked it", () => {
    const w = useWorkspace.getState();
    w.setSplit("power");
    expect(useWorkspace.getState()).toMatchObject({ split: "power", linked: true, splitLinked: true });
    useWorkspace.getState().setSplit(null);
    expect(useWorkspace.getState()).toMatchObject({ split: null, linked: false });
  });

  it("leaves linking on when it was on already", () => {
    useWorkspace.setState({ linked: true });
    useWorkspace.getState().setSplit("power");
    useWorkspace.getState().setSplit(null);
    expect(useWorkspace.getState().linked).toBe(true);
  });

  it("keeps a module shown on the right where it is", () => {
    useWorkspace.getState().setSplit("power");
    useWorkspace.getState().requestSolve();
    expect(useWorkspace.getState()).toMatchObject({ module: "diagram", split: "power" });
    useWorkspace.getState().setModule("power");
    expect(useWorkspace.getState().module).toBe("diagram");
    useWorkspace.getState().setModule("diff");
    expect(useWorkspace.getState()).toMatchObject({ module: "diff", split: "power" });
  });

  it("swaps sides", () => {
    useWorkspace.getState().setSplit("power");
    useWorkspace.getState().swapSplit();
    expect(useWorkspace.getState()).toMatchObject({ module: "power", split: "diagram" });
  });
});
