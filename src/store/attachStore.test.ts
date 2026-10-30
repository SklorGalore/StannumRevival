import { describe, expect, it, vi } from "vitest";
import { api } from "../lib/analysis/api";
import { answerUi, catchUp } from "./attachStore";
import { useWorkspace } from "./workspaceStore";

describe("answerUi", () => {
  it("switches module and reports the window", () => {
    const got = answerUi("ui.go", { module: "records" });
    expect(got).toMatchObject({ ok: { module: "records" } });
    expect(useWorkspace.getState().module).toBe("records");
  });

  it("refuses a module that isn't there", () => {
    expect(answerUi("ui.go", { module: "nope" })).toMatchObject({ error: { kind: "invalid_arguments" } });
  });

  it("needs a case to show an element", () => {
    expect(answerUi("ui.show", { element: "bus:1" })).toMatchObject({ error: { kind: "no_case" } });
  });

  it("lists and runs Ctrl+K commands by id", () => {
    const list = answerUi("ui.commands", {}) as { ok: { id: string; title: string }[] };
    expect(list.ok.map((c) => c.id)).toContain("go-diagram");
    expect(answerUi("ui.run", { id: "go-diagram" })).toMatchObject({ ok: { id: "go-diagram" } });
    expect(useWorkspace.getState().module).toBe("diagram");
    expect(answerUi("ui.run", { id: "nope" })).toMatchObject({ error: { kind: "not_found" } });
  });

  it("names the commands it has when asked for another", () => {
    const got = answerUi("ui.fly", {}) as { error: { kind: string; message: string } };
    expect(got.error.kind).toBe("unknown_command");
    expect(got.error.message).toContain("ui.state");
  });
});

describe("catchUp", () => {
  it("adopts a sample a script opened", async () => {
    const summary = { id: 7, name: "case9.m", path: "/tmp/case9.m", power: { available: false }, topology: { available: true } };
    const spy = vi.spyOn(api, "caseSummary").mockResolvedValue(summary as never);
    await catchUp("open_sample", { id: "case9" }, summary);
    expect(useWorkspace.getState().caseSummary?.id).toBe(7);
    spy.mockRestore();
  });
});
