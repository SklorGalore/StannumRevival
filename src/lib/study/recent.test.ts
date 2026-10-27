import { describe, expect, it } from "vitest";
import { fileName, folderOf, RECENT_LIMIT, withRecent, type RecentFile } from "./recent";

const f = (path: string, at = 0): RecentFile => ({ path, name: fileName(path), kind: "case", at });

describe("recent files", () => {
  it("moves a reopened file to the top and keeps the list short", () => {
    let list: RecentFile[] = [];
    for (let i = 0; i < RECENT_LIMIT + 3; i++) list = withRecent(list, f(`/c/${i}.m`, i));
    expect(list).toHaveLength(RECENT_LIMIT);
    expect(list[0].path).toBe(`/c/${RECENT_LIMIT + 2}.m`);
    list = withRecent(list, f("/c/5.m", 99));
    expect(list[0]).toMatchObject({ path: "/c/5.m", at: 99 });
    expect(list.filter((x) => x.path === "/c/5.m")).toHaveLength(1);
  });

  it("splits Windows and Unix paths", () => {
    expect(fileName("C:\\Cases\\texas.RAW")).toBe("texas.RAW");
    expect(folderOf("C:\\Cases\\texas.RAW")).toBe("C:\\Cases");
    expect(fileName("/home/a/case9.m")).toBe("case9.m");
    expect(folderOf("case9.m")).toBe("");
  });
});
