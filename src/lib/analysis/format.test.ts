import { describe, expect, it } from "vitest";
import { branchParts } from "./format";

describe("branch ids taken apart for exports", () => {
  it("gives the ends and circuit of lines, transformers and switches", () => {
    expect(branchParts("line:1053:12835:2")).toEqual({ from_bus: 1053, to_bus: 12835, circuit: "2" });
    expect(branchParts("transformer:2096:2097:1")).toEqual({ from_bus: 2096, to_bus: 2097, circuit: "1" });
    expect(branchParts("switch:5:6:A")).toEqual({ from_bus: 5, to_bus: 6, circuit: "A" });
  });

  it("leaves other ids blank", () => {
    const blank = { from_bus: "", to_bus: "", circuit: "" };
    expect(branchParts("bus:5")).toEqual(blank);
    expect(branchParts("generator:2:1")).toEqual(blank);
    expect(branchParts("line:1:2:1&line:2:3:1")).toEqual(blank);
    expect(branchParts(null)).toEqual(blank);
  });
});
