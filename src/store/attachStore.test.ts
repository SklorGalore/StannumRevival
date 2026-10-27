import { describe, expect, it } from "vitest";
import { answerUi } from "./attachStore";
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
