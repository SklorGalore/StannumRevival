import { describe, expect, it } from "vitest";
import { groupWarnings, lineSpan } from "./warnings";

describe("groupWarnings", () => {
  it("groups by message, most frequent first, keeping each element once", () => {
    const groups = groupWarnings([
      { line: 3, message: "a" },
      { line: 5, message: "b", element: "gen:1:1" },
      { line: 6, message: "b", element: "gen:2:1" },
      { line: 6, message: "b", element: "gen:2:1" },
    ]);
    expect(groups.map((g) => [g.message, g.count])).toEqual([
      ["b", 3],
      ["a", 1],
    ]);
    expect(groups[0].elements).toEqual(["gen:1:1", "gen:2:1"]);
  });
});

describe("lineSpan", () => {
  it("names one line, two lines or a range", () => {
    expect(lineSpan([{ line: 4, message: "" }])).toBe("line 4");
    expect(lineSpan([{ line: 4, message: "" }, { line: 9, message: "" }])).toBe("lines 4, 9");
    expect(lineSpan([{ line: 9, message: "" }, { line: 4, message: "" }, { line: 6, message: "" }])).toBe("lines 4–9");
  });

  it("leaves out line 0, which is no line", () => {
    expect(lineSpan([{ line: 0, message: "" }, { line: 7, message: "" }])).toBe("line 7");
    expect(lineSpan([{ line: 0, message: "" }])).toBe("");
  });
});
