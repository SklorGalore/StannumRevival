import { invokeCore, saveTextExport } from "../analysis/api";
import { catchUpLog, useLog } from "../../store/logStore";
import { useCrash } from "../../store/crashStore";
import { appVersion, diagnosticsFileName, diagnosticsText } from "./diagnostics";

/**
 * Ask where to save, then write the log and versions there. Works even when
 * the core has failed: the report then says the core wasn't reachable.
 * Resolves true once the file is written.
 */
export async function saveDiagnostics(): Promise<boolean> {
  const when = new Date();
  await catchUpLog();
  let core = null;
  try {
    core = await invokeCore("diagnostics");
  } catch {
    // Reported as "not reachable" in the file.
  }
  const app = { version: await appVersion(), userAgent: navigator.userAgent, internalErrors: useCrash.getState().count };
  const text = diagnosticsText(when, app, core, useLog.getState().entries);
  try {
    const saved = await saveTextExport(diagnosticsFileName(when), text, { name: "Text", extensions: ["txt"] });
    if (saved) useLog.getState().add("info", "Saved diagnostics. Attach the file when you report the problem.");
    return saved;
  } catch {
    // The failure is already in the log.
    return false;
  }
}
