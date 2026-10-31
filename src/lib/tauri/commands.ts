import { invoke, isTauri } from "@tauri-apps/api/core";
import { save, open, confirm } from "@tauri-apps/plugin-dialog";

/**
 * Ask an OK/Cancel question. Use this instead of window.confirm: in the app,
 * the dialog plugin swaps window.confirm for an async version whose Promise is
 * always truthy, so `if (!window.confirm(...))` never stops anything.
 */
export async function confirmDialog(message: string): Promise<boolean> {
  if (isTauri()) return confirm(message, { title: "Stannum", kind: "warning" });
  if (typeof window === "undefined" || typeof window.confirm !== "function") return true;
  return window.confirm(message);
}

export async function showSaveDialog(
  defaultName: string,
  filters: { name: string; extensions: string[] }[]
): Promise<string | null> {
  return save({ defaultPath: defaultName, filters }) as Promise<string | null>;
}

export async function showOpenDialog(): Promise<string | null> {
  const result = await open({
    multiple: false,
    filters: [{ name: "SLD Diagram", extensions: ["sld", "json"] }],
  });
  if (!result) return null;
  return typeof result === "string" ? result : (result as string[])[0] ?? null;
}

export async function saveDiagramFile(
  path: string,
  content: string
): Promise<void> {
  await invoke<void>("save_diagram", { path, content });
}

export async function loadDiagramFile(path: string): Promise<string> {
  return invoke<string>("load_diagram", { path });
}

export async function saveBinaryFile(
  path: string,
  dataB64: string
): Promise<void> {
  await invoke<void>("save_binary", { path, dataB64 });
}
