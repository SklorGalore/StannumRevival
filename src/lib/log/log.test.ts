import { describe, expect, it } from "vitest";
import { asText, duration, linkify, shown, type LogEntry } from "./log";

const entry = (level: LogEntry["level"], message = "m", source = "core"): LogEntry => ({ key: 1, time: 0, level, source, message });

describe("shown", () => {
  it("hides details unless asked", () => {
    expect(shown(entry("debug"), "all", false, "")).toBe(false);
    expect(shown(entry("debug"), "all", true, "")).toBe(true);
  });
  it("filters by level", () => {
    expect(shown(entry("info"), "warn", false, "")).toBe(false);
    expect(shown(entry("warn"), "warn", false, "")).toBe(true);
    expect(shown(entry("error"), "warn", false, "")).toBe(false);
    expect(shown(entry("warn"), "error", false, "")).toBe(false);
  });
  it("searches message and source", () => {
    expect(shown(entry("info", "Solving power flow"), "all", false, "POWER")).toBe(true);
    expect(shown(entry("info", "x", "rs-power"), "all", false, "rs-")).toBe(true);
    expect(shown(entry("info", "x"), "all", false, "y")).toBe(false);
  });
});

describe("linkify", () => {
  it("links bus numbers", () => {
    expect(linkify("Voltage low at bus 101.")).toEqual([
      { text: "Voltage low at bus" },
      { text: " " },
      { text: "101", element: "bus:101" },
      { text: "." },
    ]);
  });
  it("links lists of buses", () => {
    const links = linkify("buses 4, 5 and 6 are isolated").filter((s) => "element" in s);
    expect(links.map((s) => s.text)).toEqual(["4", "5", "6"]);
  });
  it("stops at values that follow a bus", () => {
    const links = linkify("worst bus 9033, 0.0194 pu outside").filter((s) => "element" in s);
    expect(links.map((s) => s.text)).toEqual(["9033"]);
  });
  it("leaves counts alone", () => {
    expect(linkify("Read case9.m: 9 buses, 3 generators")).toEqual([{ text: "Read case9.m: 9 buses, 3 generators" }]);
  });
  it("keeps the message text intact", () => {
    const m = "Bus 7 and bus 12-14 moved";
    expect(linkify(m).map((s) => s.text).join("")).toBe(m);
  });
});

describe("formatting", () => {
  it("writes durations people can read", () => {
    expect(duration(850)).toBe("850 ms");
    expect(duration(3400)).toBe("3.4 s");
    expect(duration(42_000)).toBe("42 s");
    expect(duration(125_000)).toBe("2 min 05 s");
  });
  it("exports one line per entry", () => {
    const text = asText([entry("warn", "careful", "rs-power")]);
    expect(text).toMatch(/^\d\d:\d\d:\d\d\.\d{3} WARN  \[rs-power\] careful\n$/);
  });
});
