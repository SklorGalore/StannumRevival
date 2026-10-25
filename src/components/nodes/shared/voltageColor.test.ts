import { describe, expect, it } from "vitest";
import { VOLTAGE_LEVELS, getVoltageColor, voltageLevel } from "./voltageColor";

describe("voltage colours", () => {
  it("gives each standard level its own colour", () => {
    expect(new Set(VOLTAGE_LEVELS.map((l) => l.light)).size).toBe(VOLTAGE_LEVELS.length);
    expect(new Set(VOLTAGE_LEVELS.map((l) => l.dark)).size).toBe(VOLTAGE_LEVELS.length);
  });

  it("keeps 138 and 230 kV apart", () => {
    expect(getVoltageColor(138)).not.toBe(getVoltageColor(230));
    expect(getVoltageColor(69)).not.toBe(getVoltageColor(34.5));
  });

  it("matches off-standard voltages to the nearest level", () => {
    expect(voltageLevel(132).kv).toBe(138);
    expect(voltageLevel(220).kv).toBe(230);
    expect(voltageLevel(400).kv).toBe(345);
    expect(voltageLevel(12.47).kv).toBe(13.8);
    expect(voltageLevel(24.9).kv).toBe(23);
    expect(voltageLevel(1000).kv).toBe(765);
    expect(voltageLevel(0.208).kv).toBe(0.48);
  });

  it("uses the brighter shade on the dark canvas", () => {
    expect(getVoltageColor(138, true)).toBe("#facc15");
  });
});
