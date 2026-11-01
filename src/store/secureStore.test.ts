import { describe, expect, it } from "vitest";
import { DEFAULT_SECURE, channelElement, curveRows, defaultPicks, secureSettings } from "./secureStore";
import type { ChannelCurves, SecureResults } from "../lib/analysis/commands.gen";
import type { SolveSettings } from "../lib/analysis/types";

describe("secure transfer settings", () => {
  it("maps the choice and takes the contingency setup's settings", () => {
    const s = secureSettings(
      {
        ...DEFAULT_SECURE,
        sink: { kind: "buses", value: 0, buses: "44, 45" },
        source: { kind: "area", value: 1, buses: "" },
        majorMw: "50",
        minorMw: "",
        stopVoltage: false,
        stopAtViolation: false,
        channels: [{ quantity: "mw", element: "line:1:4:1" }],
      },
      {} as SolveSettings,
      null
    );
    expect(s.sink).toEqual({ buses: [44, 45] });
    expect(s.source).toEqual({ areas: [1] });
    expect(s.major_step_mw).toBe(50);
    expect(s.minor_step_mw).toBeNull();
    expect(s.stop_on_voltage).toBe(false);
    expect(s.stop_on_thermal).toBe(true);
    expect(s.stop_at_violation).toBe(false);
    expect(s.channels).toEqual([{ quantity: "mw", element: "line:1:4:1" }]);
    expect(s.contingencies?.solve?.model).toBe("ac");
  });

  it("reads a bare number as a bus for voltages", () => {
    expect(channelElement("vm", " 101 ")).toBe("bus:101");
    expect(channelElement("mw", "line:1:2:1")).toBe("line:1:2:1");
    expect(channelElement("interface", "101")).toBe("101");
  });
});

describe("secure transfer results", () => {
  it("draws the most limiting contingencies first", () => {
    const r = {
      contingencies: [
        { index: 3, limit_mw: 10 },
        { index: 0, limit_mw: 12 },
        { index: 5, limit_mw: 20 },
        { index: 1, limit_mw: null },
      ],
    } as unknown as SecureResults;
    expect(defaultPicks(r)).toEqual([3, 5]);
  });

  it("writes a channel's curves as rows", () => {
    const c = {
      transfer_mw: [0, 10],
      curves: [
        { index: 0, name: "BASE CASE", values: [1.0, 0.99] },
        { index: 2, name: "A B 1", values: [0.98, null] },
      ],
    } as unknown as ChannelCurves;
    expect(curveRows(c)).toEqual([
      { transfer_mw: 0, "BASE CASE": 1.0, "A B 1": 0.98 },
      { transfer_mw: 10, "BASE CASE": 0.99, "A B 1": "" },
    ]);
  });
});
