import { describe, expect, it } from "vitest";
import { withEdit, type StudyEntry } from "./studyStore";

const out = (element: string): StudyEntry => ({ edit: { kind: "status", element, online: false } });

describe("study edits", () => {
  it("replaces an earlier edit to the same thing", () => {
    const a = withEdit([], { edit: { kind: "generation", element: "generator:2:1", mw: 100 } });
    const b = withEdit(a, { edit: { kind: "generation", element: "generator:2:1", mw: 120 } });
    expect(b).toEqual([{ edit: { kind: "generation", element: "generator:2:1", mw: 120 } }]);
  });

  it("drops an outage when the element is put back", () => {
    const a = withEdit([out("line:4:5:1"), out("line:5:6:1")], { edit: { kind: "status", element: "line:4:5:1", online: true } });
    expect(a).toEqual([out("line:5:6:1")]);
  });

  it("keeps a status and an output edit to the same generator apart", () => {
    const a = withEdit([out("generator:2:1")], { edit: { kind: "generation", element: "generator:2:1", mw: 50 } });
    expect(a).toHaveLength(2);
  });
});
