import { describe, expect, it } from "vitest";
import { DEFAULT_TRANSFER, busList, subsystem, transferSettings } from "./transferStore";

describe("transfer settings", () => {
  it("reads typed bus lists", () => {
    expect(busList("101, 102 205;7")).toEqual([101, 102, 205, 7]);
    expect(busList(" ,x, -3 1.5 ")).toEqual([]);
  });

  it("picks one area, zone, owner or the listed buses", () => {
    expect(subsystem({ kind: "area", value: 3, buses: "" })).toEqual({ areas: [3] });
    expect(subsystem({ kind: "zone", value: 9, buses: "" })).toEqual({ zones: [9] });
    expect(subsystem({ kind: "owner", value: 2, buses: "" })).toEqual({ owners: [2] });
    expect(subsystem({ kind: "buses", value: 0, buses: "1 2" })).toEqual({ buses: [1, 2] });
  });

  it("leaves outages out for a base case only study", () => {
    const s = transferSettings({ ...DEFAULT_TRANSFER, outages: false, cutoffPct: 3 });
    expect(s.contingencies).toBeNull();
    expect(s.cutoff).toBeCloseTo(0.03);
    const t = transferSettings({ ...DEFAULT_TRANSFER, generators: true, outageKvMin: 100 });
    expect(t.contingencies).toMatchObject({ branches: true, generators: true, kv_min: 100 });
  });
});
