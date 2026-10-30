import { describe, expect, it } from "vitest";
import { shortVersion } from "./shortVersion";

describe("shortVersion", () => {
  it("shows major.minor for a release", () => {
    expect(shortVersion("0.5.0")).toBe("v0.5");
    expect(shortVersion("1.0.0")).toBe("v1.0");
  });
  it("keeps the pre-release suffix", () => {
    expect(shortVersion("1.0.0-rc.1")).toBe("v1.0-rc.1");
    expect(shortVersion("1.0.0-beta-2")).toBe("v1.0-beta-2");
  });
  it("drops build metadata", () => {
    expect(shortVersion("1.0.0+abc")).toBe("v1.0");
  });
});
