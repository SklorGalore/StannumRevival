import { afterEach, describe, expect, it, vi } from "vitest";
import { queue } from "./remeasure";

describe("queue", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("re-measures all the nodes queued in a frame with one update", () => {
    const frames: (() => void)[] = [];
    vi.stubGlobal("requestAnimationFrame", (cb: () => void) => frames.push(cb));
    const update = vi.fn();
    for (let i = 0; i < 300; i++) queue(update, `n${i % 150}`);
    expect(frames).toHaveLength(1);
    frames[0]();
    expect(update).toHaveBeenCalledTimes(1);
    expect(update.mock.calls[0][0]).toHaveLength(150);
    // The next change starts a new batch.
    queue(update, "n1");
    expect(frames).toHaveLength(2);
    frames[1]();
    expect(update).toHaveBeenLastCalledWith(["n1"]);
  });
});
