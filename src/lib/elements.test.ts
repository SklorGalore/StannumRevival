import { describe, expect, it } from "vitest";
import { elementBus, elementLabel, elementSection, parseElement } from "./elements";

describe("shared element ids", () => {
  it("parses branches, three-winding transformers and machines", () => {
    expect(parseElement("line:101:102:2")).toEqual({ kind: "line", buses: [101, 102], circuit: "2" });
    expect(parseElement("transformer:10:20:30:T1")).toEqual({ kind: "transformer", buses: [10, 20, 30], circuit: "T1" });
    expect(parseElement("generator:101:G1")).toEqual({ kind: "generator", buses: [101], circuit: "G1" });
    expect(parseElement("area:1")).toBeNull();
  });

  it("finds the bus a selection sits at", () => {
    expect(elementBus("bus:7")).toBe(7);
    expect(elementBus("load:7:1")).toBe(7);
    expect(elementBus("line:7:8:1")).toBeNull();
  });

  it("names the Case diff section for each kind", () => {
    expect(elementSection("bus:7")).toBe("BUS");
    expect(elementSection("line:7:8:1")).toBe("BRANCH");
    expect(elementSection("switch:7:8:1")).toBe("SYSTEM SWITCHING DEVICE");
    expect(elementSection("transformer:7:8:1")).toBe("TRANSFORMER");
    expect(elementSection("nonsense")).toBeNull();
  });

  it("reads well", () => {
    expect(elementLabel("line:1001:1071:2")).toBe("Line 1001–1071 circuit 2");
    expect(elementLabel("load:5:1")).toBe("Load 1 at bus 5");
  });
});
