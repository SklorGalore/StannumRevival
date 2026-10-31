import {
  showSaveDialog,
  showOpenDialog,
  saveDiagramFile,
  loadDiagramFile,
  confirmDialog,
} from "./commands";
import { serialize } from "../serialization/serialize";
import { deserialize } from "../serialization/deserialize";
import { useDiagramStore } from "../../store/diagramStore";
import { useLog } from "../../store/logStore";

export async function saveDiagram(): Promise<void> {
  const store = useDiagramStore.getState();
  const path = store.currentFilePath ?? (await saveAsDialog());
  if (!path) return;
  const json = serialize(store);
  await saveDiagramFile(path, json);
  store.setFilePath(path);
  store.markClean();
}

export async function saveAsDialog(): Promise<string | null> {
  const store = useDiagramStore.getState();
  const title = store.metadata.title.replace(/[^a-zA-Z0-9_-]/g, "_");
  const path = await showSaveDialog(`${title}.sld`, [
    { name: "SLD Diagram", extensions: ["sld", "json"] },
  ]);
  if (!path) return null;
  const json = serialize(store);
  await saveDiagramFile(path, json);
  store.setFilePath(path);
  store.markClean();
  return path;
}

export async function openDiagram(): Promise<void> {
  const store = useDiagramStore.getState();
  if (store.isDirty) {
    const ok = await confirmDialog(
      "You have unsaved changes. Open a different file anyway?"
    );
    if (!ok) return;
  }
  const path = await showOpenDialog();
  if (!path) return;
  let diagram;
  try {
    diagram = deserialize(await loadDiagramFile(path));
  } catch (e) {
    // A damaged file, or one from a newer Stannum: say so rather than fail silently.
    const message = `Couldn't open ${path.split(/[\\/]/).pop()}: ${(e as { message?: string } | null)?.message ?? String(e)}`;
    useLog.getState().add("error", message);
    window.alert(message);
    return;
  }
  store.loadDiagramFromJSON(diagram);
  store.setFilePath(path);
}
