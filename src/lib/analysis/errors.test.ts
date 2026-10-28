import { describe, expect, it } from "vitest";
import { isErrorKind, StannumError, toStannumError } from "./api";

describe("toStannumError", () => {
  it("keeps the core's kind and message", () => {
    const e = toStannumError({ kind: "cancelled", message: "Solve cancelled. The previous results are kept." });
    expect(e).toBeInstanceOf(StannumError);
    expect(e).toBeInstanceOf(Error);
    expect(e.kind).toBe("cancelled");
    expect(e.message).toBe("Solve cancelled. The previous results are kept.");
    expect(isErrorKind(e, "cancelled")).toBe(true);
    expect(isErrorKind(e, "no_case")).toBe(false);
  });

  it("reads text and plain errors from the shell", () => {
    expect(toStannumError("No such file")).toMatchObject({ kind: "shell", message: "No such file" });
    expect(toStannumError(new Error("boom"))).toMatchObject({ kind: "shell", message: "boom" });
  });

  it("passes a StannumError through", () => {
    const e = new StannumError("no_case", "Open a case first.");
    expect(toStannumError(e)).toBe(e);
  });
});
