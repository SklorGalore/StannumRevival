import { describe, expect, it } from "vitest";
import { DEFAULT_SOLVE_SETTINGS } from "../analysis/api";
import { compareToFile, describeChange, parseStudyFile, serializeStudyFile, STUDY_FORMAT, STUDY_VERSION, type StudyFile } from "./studyFile";

const fp = { size: 10, sha256: "aa", modified_ms: 1 };

function study(over: Partial<StudyFile> = {}): StudyFile {
  return {
    format: STUDY_FORMAT,
    version: STUDY_VERSION,
    app: "0.2.0",
    savedAt: "2026-10-27T00:00:00.000Z",
    case: { path: "/cases/case9.m", relative: "case9.m", name: "case9.m", format: "matpower", fingerprint: fp },
    dcGrids: null,
    solveSettings: DEFAULT_SOLVE_SETTINGS,
    edits: [{ edit: { kind: "status", element: "branch:1:4:1", online: false }, was: "in service" }],
    solves: [],
    compare: null,
    contingencies: { branches: true, generators: false, method: "ac", acCount: 20, limitPct: 90, rating: "b", baseRating: "a" },
    diagram: null,
    view: { module: "power", split: "diagram", splitRatio: 0.6 },
    ...over,
  };
}

describe("study files", () => {
  it("round-trips", async () => {
    const s = study();
    expect(await parseStudyFile(serializeStudyFile(s))).toEqual(s);
  });

  it("says plainly what is wrong with a file", async () => {
    await expect(parseStudyFile("not json")).rejects.toThrow(/isn't JSON/);
    await expect(parseStudyFile('{"format":"other"}')).rejects.toThrow(/isn't a Stannum study/);
    await expect(parseStudyFile(JSON.stringify({ ...study(), version: 9 }))).rejects.toThrow(/newer Stannum/);
    const bad = { ...study(), edits: [{ edit: { kind: "load", bus: "x" } }] };
    await expect(parseStudyFile(JSON.stringify(bad))).rejects.toThrow(/damaged: edits\.0\.edit/);
  });

  it("keeps compare pairs by position", () => {
    expect(compareToFile({ a: 7, b: "current" }, [5, 7])).toEqual({ a: 1, b: "current" });
    expect(compareToFile({ a: 3, b: 7 }, [5, 7])).toBeNull();
    expect(compareToFile(null, [])).toBeNull();
  });

  it("notices a changed case", () => {
    expect(describeChange(fp, { ...fp, modified_ms: 2 })).toBeNull();
    expect(describeChange(fp, { ...fp, sha256: "bb" })).toMatch(/different/);
    expect(describeChange(fp, { ...fp, sha256: "bb", size: 12 })).toMatch(/10 bytes then, 12 now/);
  });
});
