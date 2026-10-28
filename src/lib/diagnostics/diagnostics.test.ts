import { describe, expect, it } from "vitest";
import { diagnosticsFileName, diagnosticsText, hideHome } from "./diagnostics";
import type { LogEntry } from "../log/log";

const core = { core: "0.2.0", api_version: 3, rs_power: "0.1.0", nodal: "0.1.0", os: "windows", arch: "x86_64", debug_build: false, threads: 8, internal_errors: 1 };
const app = { version: "0.3.0", userAgent: "Mozilla/5.0 (Windows NT 10.0)", internalErrors: 1 };
const line = (message: string, level: LogEntry["level"] = "info"): LogEntry => ({ key: 1, time: Date.UTC(2026, 9, 28, 20, 31), level, source: "core", message });

describe("diagnostics", () => {
  it("shortens home folders on every platform", () => {
    expect(hideHome("Opened C:\\Users\\ana\\cases\\a.raw")).toBe("Opened ~\\cases\\a.raw");
    expect(hideHome("Opened /Users/ana/cases/a.raw and /home/bo/b.m")).toBe("Opened ~/cases/a.raw and ~/b.m");
    expect(hideHome("/opt/cases/c.raw")).toBe("/opt/cases/c.raw");
  });

  it("puts versions first, then the log", () => {
    const text = diagnosticsText(new Date(Date.UTC(2026, 9, 28)), app, core, [line("Opened /home/ana/x.raw"), line("Internal error on thread main: boom", "error")]);
    expect(text).toMatch(/^Stannum diagnostics\n/);
    expect(text).toContain("Stannum:");
    expect(text).toContain("stannum-core 0.2.0, command API 3");
    expect(text).toContain("rs-power 0.1.0, nodal 0.1.0");
    expect(text).toContain("windows x86_64, 8 threads");
    expect(text).toContain("Log (2 lines, oldest first)");
    expect(text).toContain("[core] Opened ~/x.raw");
    expect(text).toContain("ERROR [core] Internal error on thread main: boom");
    expect(text.indexOf("Platform")).toBeLessThan(text.indexOf("Log ("));
  });

  it("says when the core can't be reached", () => {
    expect(diagnosticsText(new Date(), app, null, [])).toContain("not reachable");
  });

  it("names the file by date and time", () => {
    expect(diagnosticsFileName(new Date(2026, 9, 28, 20, 5))).toBe("stannum-diagnostics-2026-10-28-2005.txt");
  });
});
