import { describe, expect, it } from "vitest";
import { elementBus, elementLabel, elementSection, elementSections, parseElement } from "./elements";

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
    expect(elementLabel("dcline:301:3021:DC 1")).toBe("DC line DC 1 (301–3021)");
    expect(elementSection("dcline:301:3021:1")).toBe("TWO-TERMINAL DC");
  });

  it("knows the kinds that have their own ids since the kinds table", () => {
    expect(parseElement("switched-shunt:5:1")).toEqual({ kind: "switched-shunt", buses: [5], circuit: "1" });
    expect(parseElement("facts:SVC:A")).toEqual({ kind: "facts", buses: [], circuit: "SVC:A" });
    expect(elementLabel("shunt:4:1")).toBe("Fixed shunt 1 at bus 4");
    expect(elementLabel("facts:STATCOM_3")).toBe("FACTS device STATCOM_3");
    expect(elementBus("switched-shunt:5:1")).toBe(5);
    expect(elementBus("facts:STATCOM_3")).toBeNull();
    expect(elementSection("machine:2:1")).toBe("INDUCTION MACHINE");
    expect(elementSections("dcline:1:2:V1")).toEqual(["TWO-TERMINAL DC", "VSC DC LINE"]);
  });
});
