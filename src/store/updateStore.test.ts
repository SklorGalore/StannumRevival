import { beforeEach, describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({
  tauri: true,
  enabled: true,
  found: null as unknown,
  checkError: null as Error | null,
  relaunch: vi.fn(async () => {}),
}));

// Tests run in Node, which has no localStorage.
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
  clear: () => store.clear(),
});

vi.mock("@tauri-apps/api/core", async (actual) => ({
  ...(await actual<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => native.tauri,
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "updates_enabled") return native.enabled;
    throw new Error(`no ${cmd} in tests`);
  }),
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(async () => {
    if (native.checkError) throw native.checkError;
    return native.found;
  }),
}));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: native.relaunch }));

const { useUpdate } = await import("./updateStore");
const { notesPreview } = await import("../shell/UpdateBar");

function fakeUpdate(version = "0.6.0") {
  return {
    version,
    currentVersion: "0.5.0",
    body: "## 0.6.0\n\n- **Faster** solves\n- Auto updates",
    date: "2026-11-01T00:00:00Z",
    close: vi.fn(async () => {}),
    downloadAndInstall: vi.fn(async (progress: (e: unknown) => void) => {
      progress({ event: "Started", data: { contentLength: 100 } });
      progress({ event: "Progress", data: { chunkLength: 60 } });
      progress({ event: "Progress", data: { chunkLength: 40 } });
      progress({ event: "Finished" });
    }),
  };
}

beforeEach(() => {
  Object.assign(native, { tauri: true, enabled: true, found: null, checkError: null });
  native.relaunch.mockClear();
  localStorage.clear();
  useUpdate.setState({ enabled: null, phase: "idle", update: null, received: 0, total: null, error: null, skipped: null, autoCheck: true });
});

describe("checking for updates", () => {
  it("offers a newer version", async () => {
    native.found = fakeUpdate();
    await useUpdate.getState().check();
    const s = useUpdate.getState();
    expect(s.phase).toBe("available");
    expect(s.update).toMatchObject({ version: "0.6.0", current: "0.5.0" });
  });

  it("says when this is the latest, unless the check was the quiet one", async () => {
    await useUpdate.getState().check();
    expect(useUpdate.getState().phase).toBe("current");
    useUpdate.getState().dismiss();
    await useUpdate.getState().check({ quiet: true });
    expect(useUpdate.getState().phase).toBe("idle");
  });

  it("keeps a failed startup check out of sight, but shows a failed manual one", async () => {
    native.checkError = new Error("offline");
    await useUpdate.getState().check({ quiet: true });
    expect(useUpdate.getState().phase).toBe("idle");
    await useUpdate.getState().check();
    expect(useUpdate.getState()).toMatchObject({ phase: "error", error: expect.stringContaining("offline") });
  });

  it("explains that a build without the updater can't update itself", async () => {
    native.enabled = false;
    await useUpdate.getState().check({ quiet: true });
    expect(useUpdate.getState().phase).toBe("idle");
    await useUpdate.getState().check();
    expect(useUpdate.getState()).toMatchObject({ enabled: false, phase: "error", error: expect.stringContaining("development build") });
  });

  it("stays quiet at startup about a skipped version, but not about the next one", async () => {
    native.found = fakeUpdate("0.6.0");
    await useUpdate.getState().check();
    useUpdate.getState().skip();
    expect(localStorage.getItem("stannum-update-skip")).toBe("0.6.0");
    await useUpdate.getState().check({ quiet: true });
    expect(useUpdate.getState().phase).toBe("idle");
    // Asking by hand still shows it.
    await useUpdate.getState().check();
    expect(useUpdate.getState().phase).toBe("available");
    native.found = fakeUpdate("0.7.0");
    useUpdate.getState().dismiss();
    await useUpdate.getState().check({ quiet: true });
    expect(useUpdate.getState()).toMatchObject({ phase: "available", update: { version: "0.7.0" } });
  });

  it("remembers turning the startup check off", () => {
    useUpdate.getState().setAutoCheck(false);
    expect(localStorage.getItem("stannum-update-check")).toBe("off");
    useUpdate.getState().setAutoCheck(true);
    expect(localStorage.getItem("stannum-update-check")).toBeNull();
  });
});

describe("installing", () => {
  it("downloads, counts the bytes and restarts", async () => {
    const update = fakeUpdate();
    native.found = update;
    await useUpdate.getState().check();
    await useUpdate.getState().install();
    expect(update.downloadAndInstall).toHaveBeenCalled();
    expect(useUpdate.getState()).toMatchObject({ phase: "restarting", received: 100, total: 100 });
    expect(native.relaunch).toHaveBeenCalled();
  });

  it("reports a failed install and doesn't restart", async () => {
    const update = fakeUpdate();
    update.downloadAndInstall.mockRejectedValueOnce(new Error("signature mismatch"));
    native.found = update;
    await useUpdate.getState().check();
    await useUpdate.getState().install();
    expect(useUpdate.getState()).toMatchObject({ phase: "error", error: expect.stringContaining("signature mismatch") });
    expect(native.relaunch).not.toHaveBeenCalled();
  });
});

describe("notesPreview", () => {
  it("turns the first lines of markdown notes into plain text", () => {
    expect(notesPreview("## 0.6.0\n\n- **Faster** solves\n- Auto `updates`\n- Third\n- Fourth")).toEqual(["Faster solves", "Auto updates", "Third"]);
  });
});
