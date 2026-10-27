import { describe, expect, it } from "vitest";
import { describeRun, recordStep } from "./runFileStore";
import type { RunReport } from "../lib/analysis/commands.gen";

describe("recordStep", () => {
  it("keeps actions, without the case id", () => {
    let steps = recordStep([], "open_case", { path: "/c/case9.m" });
    steps = recordStep(steps, "diagram_network", { case: 1 });
    steps = recordStep(steps, "solve_power_flow", { case: 1, settings: { model: "ac" } });
    expect(steps).toEqual([
      { command: "open_case", args: { path: "/c/case9.m" } },
      { command: "solve_power_flow", args: { settings: { model: "ac" } } },
    ]);
  });

  it("skips a setter called again with the same arguments", () => {
    const edits = [{ kind: "generation", element: "generator:2:1", mw: 120 }];
    let steps = recordStep([], "set_study_edits", { edits });
    steps = recordStep(steps, "solve_power_flow", {});
    steps = recordStep(steps, "set_study_edits", { edits });
    expect(steps.map((s) => s.command)).toEqual(["set_study_edits", "solve_power_flow"]);
    steps = recordStep(steps, "set_study_edits", { edits: [] });
    expect(steps).toHaveLength(3);
    // Solving twice is two solves.
    expect(recordStep(steps, "solve_power_flow", {})).toHaveLength(4);
  });
});

describe("describeRun", () => {
  const report = (over: Partial<RunReport>): RunReport => ({
    path: "/runs/case9_study.stnrun",
    steps: 3,
    outcomes: [
      { line: 1, command: "open_case", seconds: 0.1, error: null, result: null },
      { line: 2, command: "solve_power_flow", seconds: 0.2, error: null, result: null },
      { line: 4, command: "export_matpower", seconds: 0.1, error: null, result: null },
    ],
    failed: 0,
    cancelled: false,
    seconds: 0.4,
    ...over,
  });

  it("says how a run went", () => {
    expect(describeRun(report({}))).toEqual({ tone: "ok", title: "Ran case9_study.stnrun: 3 steps in 400 ms.", lines: [] });
    const failed = report({
      steps: 4,
      failed: 1,
      outcomes: [
        { line: 1, command: "open_case", seconds: 0.1, error: null, result: null },
        { line: 2, command: "element_detail", seconds: 0, error: { kind: "not_found", message: "bus:99 isn't in this case." }, result: null },
      ],
    });
    expect(describeRun(failed)).toEqual({
      tone: "warning",
      title: "case9_study.stnrun stopped: a line failed, 2 not run.",
      lines: ["Line 2, element_detail: bus:99 isn't in this case."],
    });
    expect(describeRun(report({ seconds: 12.34 })).title).toBe("Ran case9_study.stnrun: 3 steps in 12.3 s.");
    expect(describeRun(report({ cancelled: true, outcomes: [] })).title).toBe("Cancelled case9_study.stnrun after 0 of 3 steps.");
  });
});
