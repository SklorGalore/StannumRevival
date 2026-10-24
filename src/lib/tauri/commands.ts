import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";

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
