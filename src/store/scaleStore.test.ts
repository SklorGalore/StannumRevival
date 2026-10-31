import { describe, expect, it } from "vitest";
import { DEFAULT_SCALE_CHOICE, describeScaling, scaleSettings, type ScaleChoice } from "./scaleStore";

const choice = (patch: Partial<ScaleChoice>): ScaleChoice => ({ ...DEFAULT_SCALE_CHOICE, ...patch });

describe("scaleSettings", () => {
  it("asks for an amount first", () => {
    expect(scaleSettings(DEFAULT_SCALE_CHOICE)).toEqual({ problem: "Enter how much to scale load MW by." });
  });

  it("builds load settings at constant power factor", () => {
    const out = scaleSettings(choice({ areas: [1], mw: 10 }));
    expect(out).toEqual({
      settings: {
        buses: { areas: [1], zones: [], owners: [], kv_min: null, kv_max: null },
        load: { mw: { kind: "percent", value: 10 }, mvar: null, constant_power_factor: true, parts: "all", include_negative: false },
      },
    });
  });

  it("leaves Mvar out while the power factor is held", () => {
    const out = scaleSettings(choice({ mw: 5, scaleMvar: true, mvar: 3 }));
    expect("settings" in out && out.settings.load?.mvar).toBe(null);
    const free = scaleSettings(choice({ mw: null, constantPowerFactor: false, scaleMvar: true, mvarKind: "target", mvar: 40 }));
    expect("settings" in free && free.settings.load).toMatchObject({ mw: null, mvar: { kind: "target", value: 40 } });
  });

  it("builds generation settings and checks the kV range", () => {
    const out = scaleSettings(choice({ scaleLoad: false, scaleGeneration: true, genKind: "target", gen: 300, respectLimits: false, participation: "headroom" }));
    expect("settings" in out && out.settings.generation).toEqual({ mw: { kind: "target", value: 300 }, respect_limits: false, participation: "headroom" });
    expect(scaleSettings(choice({ mw: 1, kvMin: 200, kvMax: 100 }))).toHaveProperty("problem");
    expect(scaleSettings(choice({ scaleLoad: false }))).toEqual({ problem: "Choose load, generation or both to scale." });
  });
});

describe("describeScaling", () => {
  it("reads as one line", () => {
    expect(describeScaling(choice({ areas: [1, 2], kvMin: 100, mw: 10 }))).toBe("Load MW by +10%, constant power factor · areas 1, 2, 100 kV and up");
    expect(describeScaling(choice({ scaleLoad: false, scaleGeneration: true, genKind: "change", gen: -50 }))).toBe("Generation by -50 MW within limits · every bus");
  });
});
