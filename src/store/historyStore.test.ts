import { describe, expect, it } from "vitest";
import { DEFAULT_SOLVE_SETTINGS } from "../lib/analysis/api";
import { historySteps, stepLine, withCall, type HistoryEntry } from "./historyStore";

const at = (id: number, casePath: string | null = "/c/case9.m") => ({ id, time: 0, casePath });

describe("withCall", () => {
  it("keeps actions, without the case id", () => {
    let entries = withCall([], "open_case", { path: "/c/case9.m" }, at(1, null));
    entries = withCall(entries, "diagram_network", { case: 1 }, at(2));
    entries = withCall(entries, "solve_power_flow", { case: 1, settings: { model: "ac" } }, at(3));
    expect(entries.map(({ command, args, casePath }) => ({ command, args, casePath }))).toEqual([
      { command: "open_case", args: { path: "/c/case9.m" }, casePath: "/c/case9.m" },
      { command: "solve_power_flow", args: { settings: { model: "ac" } }, casePath: "/c/case9.m" },
    ]);
  });

  it("skips a setter called again with the same arguments", () => {
    const edits = [{ kind: "generation", element: "generator:2:1", mw: 120 }];
    let entries = withCall([], "set_study_edits", { edits }, at(1));
    entries = withCall(entries, "solve_power_flow", {}, at(2));
    entries = withCall(entries, "set_study_edits", { edits }, at(3));
    expect(entries.map((e) => e.command)).toEqual(["set_study_edits", "solve_power_flow"]);
    entries = withCall(entries, "set_study_edits", { edits: [] }, at(4));
    expect(entries).toHaveLength(3);
    // Solving twice is two solves.
    expect(withCall(entries, "solve_power_flow", {}, at(5))).toHaveLength(4);
  });
});

describe("historySteps", () => {
  const entry = (command: HistoryEntry["command"], args: Record<string, unknown>, casePath = "/c/case9.m"): HistoryEntry => ({
    id: 1,
    time: 0,
    command,
    args,
    casePath,
  });

  it("starts a part of the history from the case that was open", () => {
    expect(historySteps([entry("solve_power_flow", {})])).toEqual([
      { command: "open_case", args: { path: "/c/case9.m" } },
      { command: "solve_power_flow", args: {} },
    ]);
  });

  it("keeps a part that opens its own case as it is", () => {
    expect(historySteps([entry("open_sample", { id: "case9" }), entry("solve_power_flow", {})])).toEqual([
      { command: "open_sample", args: { id: "case9" } },
      { command: "solve_power_flow", args: {} },
    ]);
  });
});

describe("stepLine", () => {
  it("writes plain words bare and the rest as JSON", () => {
    expect(stepLine({ command: "export_raw", args: { path: "/out/a.raw", revision: 35, note: null } })).toBe("export_raw path=/out/a.raw revision=35");
    expect(stepLine({ command: "set_study_edits", args: { edits: [] } })).toBe("set_study_edits edits=[]");
    expect(stepLine({ command: "open_case", args: { path: "C:/My cases/a.raw" } })).toBe('open_case path="C:/My cases/a.raw"');
  });

  it("leaves out solve settings at their defaults", () => {
    expect(stepLine({ command: "solve_power_flow", args: { settings: DEFAULT_SOLVE_SETTINGS } })).toBe("solve_power_flow");
    expect(stepLine({ command: "solve_power_flow", args: { settings: { ...DEFAULT_SOLVE_SETTINGS, model: "dc" } } })).toBe("solve_power_flow model=dc");
  });
});
