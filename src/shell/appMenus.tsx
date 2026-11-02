// What the menu bar's File, View, Analysis and Help menus hold. Every entry
// runs the same action as its Ctrl+K command, so the menus and the palette
// never drift apart.
import type { ReactNode } from "react";
import { SPLIT_PARTNER, useWorkspace } from "../store/workspaceStore";
import { useStudyFile } from "../store/studyFileStore";
import { useRunFile } from "../store/runFileStore";
import { useTheme, type ThemePreference } from "../store/themeStore";
import { useLog } from "../store/logStore";
import { useUpdate } from "../store/updateStore";
import { useLayers } from "../store/layerStore";
import { useDiagramStore } from "../store/diagramStore";
import { useShortcuts } from "./ShortcutsDialog";
import { MODULES } from "./ModuleRail";
import { recentFiles } from "../lib/study/recent";
import { knownSampleCases } from "../ui/SampleCases";
import { saveDiagnostics } from "../lib/diagnostics/save";
import { openScaling } from "../modules/power/ScaleDialog";

export const DOCS_URL = "https://sklorgalore.github.io/StannumRevival/";

const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
export const MOD_KEY = MAC ? "⌘" : "Ctrl ";

export type MenuEntry =
  | {
      kind: "item";
      id: string;
      label: string;
      run: () => void;
      shortcut?: string;
      /** A toggle or a choice: shown with a tick when on. */
      checked?: boolean;
      disabled?: boolean;
      title?: string;
      icon?: ReactNode;
    }
  | { kind: "submenu"; id: string; label: string; items: MenuEntry[]; disabled?: boolean }
  | { kind: "separator" };

export interface AppMenu {
  id: string;
  label: string;
  items: () => MenuEntry[];
}

const RAW_TITLE = "The case with its Records edits and study edits, every record included";

const sep: MenuEntry = { kind: "separator" };
const item = (id: string, label: string, run: () => void, more: Partial<Extract<MenuEntry, { kind: "item" }>> = {}): MenuEntry => ({
  kind: "item",
  id,
  label,
  run,
  ...more,
});

export async function openLink(url: string) {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener");
  }
}

/** Records edits saved into a new case file; a failure shows where opening errors do. */
export function saveCaseAs() {
  useWorkspace
    .getState()
    .saveCaseAs()
    .catch((e: Error) => useWorkspace.setState({ openError: e.message }));
}

function fileMenu(): MenuEntry[] {
  const w = useWorkspace.getState();
  const summary = w.caseSummary;
  const study = useStudyFile.getState();
  const runFile = useRunFile.getState();
  const recent = recentFiles();
  const samples = knownSampleCases();
  const busy = w.opening || study.busy === "opening";
  const list: MenuEntry[] = [
    item("open", "Open case or run file…", () => void study.openFile(), { disabled: busy }),
    {
      kind: "submenu",
      id: "recent",
      label: "Open recent",
      disabled: recent.length === 0 || busy,
      items: recent.map((f) => item(`recent-${f.path}`, f.name, () => void study.openFile(f.path), { title: f.path })),
    },
    {
      kind: "submenu",
      id: "samples",
      label: "Open sample case",
      disabled: samples.length === 0 || busy,
      items: samples.map((c) => item(`sample-${c.id}`, c.title, () => void w.openSample(c.id))),
    },
  ];
  if (summary) {
    const exports: MenuEntry[] = [];
    exports.push(
      item("export-stnb", "Stannum case (.stnb)…", () => void study.exportStn(true), {
        title: "An open Stannum case in columns (Apache Arrow): small, quick to open, and read by pyarrow and pandas as it is",
      }),
      item("export-stn", "Stannum case as text (.stn)…", () => void study.exportStn(false), {
        title: "The same case as text, one record per line: readable, and diffs well in version control",
      })
    );
    if (summary.power.available) {
      exports.push(item("export-matpower", "MATPOWER…", () => void study.exportMatpower(false)));
      if (w.solveReport) exports.push(item("export-matpower-solved", "MATPOWER, with the latest results…", () => void study.exportMatpower(true)));
    }
    exports.push(
      item("export-raw-35", "PSS/E RAW 35…", () => void study.exportRaw(35), { title: RAW_TITLE }),
      item("export-raw-33", "PSS/E RAW 33…", () => void study.exportRaw(33), { title: RAW_TITLE })
    );
    list.push(
      sep,
      item("save-run-file", "Save as run file", () => void study.save(), {
        disabled: !!study.busy,
        title: "Write a run file that opens this case, attaches its side files, sets the study edits and solves",
      }),
      item("save-run-file-as", "Save run file as…", () => void study.save({ as: true }), { disabled: !!study.busy }),
      item("save-case-as", "Save case as…", saveCaseAs, { title: "Write the case, with its Records edits, to a new file" }),
      { kind: "submenu", id: "export", label: "Export case", items: exports }
    );
    if (summary.power.available) {
      list.push(
        sep,
        item("coordinates", summary.coordinates?.path ? "Attach other bus coordinates…" : "Attach bus coordinates…", () => void w.attachCoordinates())
      );
      if (summary.coordinates?.path) list.push(item("coordinates-clear", "Detach bus coordinates", () => void w.detachCoordinates(), { title: summary.coordinates.name }));
      list.push(item("vsc-grids", "Attach VSC DC grids…", () => w.showPowerTab("vsc")));
    }
  }
  list.push(
    sep,
    item("history", "History…", () => useLog.getState().showHistory(), { title: "Every change and export so far, to copy or save as a run file" }),
    item("run-file", "Run a run file…", () => void runFile.run())
  );
  if (summary) list.push(sep, item("close", "Close case", () => void w.closeCase()));
  return list;
}

const THEMES: { id: ThemePreference; label: string }[] = [
  { id: "system", label: "Same as the system" },
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
];

function viewMenu(): MenuEntry[] {
  const w = useWorkspace.getState();
  const summary = w.caseSummary;
  const theme = useTheme.getState();
  const log = useLog.getState();
  const list: MenuEntry[] = MODULES.map((m, i) =>
    item(`go-${m.id}`, m.label, () => w.setModule(m.id), { checked: w.module === m.id, shortcut: `${MOD_KEY}${i + 1}` })
  );
  list.push(
    sep,
    item("split", "Side by side", () => w.setSplit(w.split ? null : SPLIT_PARTNER[w.module]), { checked: !!w.split, disabled: !summary }),
    item("link", "Link selection", () => w.setLinked(!w.linked), { checked: w.linked, disabled: !summary })
  );
  if (summary?.topology.available) {
    const showDiagram = () => {
      if (w.module !== "diagram" && w.split !== "diagram") w.setModule("diagram");
    };
    list.push(
      sep,
      item("layers", "Layers", () => (showDiagram(), useLayers.getState().setPanel(true))),
      item("tables", "Topology tables", () => (showDiagram(), useLayers.getState().setTables(true)))
    );
  }
  const diagram = useDiagramStore.getState();
  list.push(sep, item("animate-flows", "Animated flow arrows", diagram.toggleAnimateFlows, { checked: diagram.animateFlows, title: "Flow arrows on the one-line move the way the power goes" }));
  const warnings = summary?.power.info?.warnings.length ?? 0;
  if (warnings) list.push(item("import-warnings", `Import warnings (${warnings.toLocaleString()})`, () => w.showPowerTab("warnings")));
  list.push(
    sep,
    item("log", "Log", () => log.setOpen(!log.open), { checked: log.open }),
    item("alerts", "Alerts", () => log.showAlerts(), { title: "Critical and marginal limit problems in the shown result" }),
    { kind: "submenu", id: "theme", label: "Theme", items: THEMES.map((t) => item(`theme-${t.id}`, t.label, () => theme.setPreference(t.id), { checked: theme.preference === t.id })) }
  );
  return list;
}

function analysisMenu(): MenuEntry[] {
  const w = useWorkspace.getState();
  const power = !!w.caseSummary?.power.available;
  const why = w.caseSummary ? "This case has no power flow data" : "Open a case first";
  return [
    item("solve", "Solve power flow", w.requestSolve, { disabled: !power, title: power ? undefined : why }),
    item("sensitivities", "Calculate sensitivities", w.requestSensitivities, { disabled: !power, title: power ? undefined : why }),
    item("contingencies", "Contingencies", () => w.showPowerTab("contingencies"), {
      disabled: !w.solveReport,
      title: w.solveReport ? undefined : "Solve the case first",
    }),
    item("transfers", "Transfer limits", () => w.showPowerTab("transfers"), {
      disabled: !w.solveReport,
      title: w.solveReport ? undefined : "Solve the case first",
    }),
    item("pv", "PV curve (voltage stability)", () => w.showPowerTab("pv"), {
      disabled: !w.solveReport,
      title: w.solveReport ? undefined : "Solve the case first",
    }),
    item("qv", "QV curve (reactive margin)", () => w.showPowerTab("qv"), {
      disabled: !w.solveReport,
      title: w.solveReport ? undefined : "Solve the case first",
    }),
    item("secure", "Secure transfer limit", () => w.showPowerTab("secure"), {
      disabled: !w.solveReport,
      title: w.solveReport ? undefined : "Solve the case first",
    }),
    sep,
    item("scale", "Scale load and generation…", openScaling, {
      disabled: !power,
      title: power ? "Scale load and generation by area, zone, owner and kV" : why,
    }),
    item("study", "Study edits", () => w.showPowerTab("study"), { disabled: !power }),
  ];
}

function helpMenu(): MenuEntry[] {
  const updates = useUpdate.getState();
  const list: MenuEntry[] = [
    item("palette", "Find in case or run a command…", () => useWorkspace.getState().setPaletteOpen(true), { shortcut: `${MOD_KEY}K` }),
    item("shortcuts", "Keyboard shortcuts", () => useShortcuts.getState().setOpen(true), { shortcut: "?" }),
    item("docs", "Documentation", () => void openLink(DOCS_URL)),
    sep,
  ];
  if (updates.phase === "available" && updates.update) list.push(item("install-update", `Install Stannum ${updates.update.version} and restart`, () => void updates.install()));
  list.push(
    item("check-updates", "Check for updates", () => void updates.check()),
    item("auto-update-check", "Check for updates at startup", () => updates.setAutoCheck(!updates.autoCheck), { checked: updates.autoCheck }),
    sep,
    item("save-diagnostics", "Save diagnostics…", () => void saveDiagnostics())
  );
  return list;
}

export const APP_MENUS: AppMenu[] = [
  { id: "file", label: "File", items: fileMenu },
  { id: "view", label: "View", items: viewMenu },
  { id: "analysis", label: "Analysis", items: analysisMenu },
  { id: "help", label: "Help", items: helpMenu },
];
