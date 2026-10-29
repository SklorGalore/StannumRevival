// Ctrl+K: find any bus, branch, generator or DC line in the open case, or run
// a command. Enter shows an element in every module; Shift+Enter draws a
// one-line around it.
import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import {
  ArrowDownCircle,
  BellOff,
  Circle,
  Columns2,
  CornerDownLeft,
  FileCode2,
  FlaskConical,
  FolderOpen,
  Keyboard,
  Save,
  GitCompareArrows,
  Link2,
  Download,
  MapPin,
  MapPinOff,
  Moon,
  Layers,
  LifeBuoy,
  Table2,
  PencilRuler,
  Play,
  Search,
  Sheet,
  Sun,
  Terminal,
  Wand2,
  X,
  Zap,
  Sparkles,
} from "lucide-react";
import { SPLIT_PARTNER, useWorkspace, type ModuleId } from "../store/workspaceStore";
import { useLog } from "../store/logStore";
import { saveDiagnostics } from "../lib/diagnostics/save";
import { useStudyFile } from "../store/studyFileStore";
import { useRunFile } from "../store/runFileStore";
import { NEXT_PREFERENCE, useTheme } from "../store/themeStore";
import { useUpdate } from "../store/updateStore";
import { caseNetwork } from "../lib/diagram/network";
import { useLayers } from "../store/layerStore";
import { paletteElements, searchElements, type PaletteElement } from "../lib/search";
import { centreBus } from "../ui/DrawAround";
import { useShortcuts } from "./ShortcutsDialog";
import { knownSampleCases } from "../ui/SampleCases";

export interface Command {
  id: string;
  title: string;
  icon: ReactNode;
  keywords: string;
  run: () => void;
}

const MODULE_COMMANDS: { id: ModuleId; title: string; icon: ReactNode }[] = [
  { id: "diagram", title: "Go to Diagram", icon: <PencilRuler size={14} /> },
  { id: "power", title: "Go to Power flow", icon: <Zap size={14} /> },
  { id: "records", title: "Go to Records", icon: <Sheet size={14} /> },
  { id: "diff", title: "Go to Case diff", icon: <GitCompareArrows size={14} /> },
];

function useCommands(): Command[] {
  const summary = useWorkspace((s) => s.caseSummary);
  const linked = useWorkspace((s) => s.linked);
  const selection = useWorkspace((s) => s.selection);
  const preference = useTheme((s) => s.preference);
  const module = useWorkspace((s) => s.module);
  const split = useWorkspace((s) => s.split);
  const logOpen = useLog((s) => s.open);
  const studyPath = useStudyFile((s) => s.path);
  const recording = useRunFile((s) => s.recording);
  const autoCheck = useUpdate((s) => s.autoCheck);
  const updatePhase = useUpdate((s) => s.phase);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  return useMemo(paletteCommands, [summary, linked, selection, preference, module, split, logOpen, studyPath, recording, autoCheck, updatePhase]);
}

/**
 * The commands Ctrl+K offers now. Scripts list and run the same ones by id
 * (stannum.attach(): app.commands(), app.run(id)).
 */
export function paletteCommands(): Command[] {
  const w = useWorkspace.getState();
  const { caseSummary: summary, linked, selection, module, split } = w;
  const preference = useTheme.getState().preference;
  const logOpen = useLog.getState().open;
  const studyPath = useStudyFile.getState().path;
  const recording = useRunFile.getState().recording;
  const next = NEXT_PREFERENCE[preference];
  const list: Command[] = [
    { id: "open", title: summary ? "Open another case…" : "Open case…", icon: <FolderOpen size={14} />, keywords: "file load raw matpower", run: () => void w.openCase() },
    { id: "open-study", title: "Open study…", icon: <FlaskConical size={14} />, keywords: "file load stannum saved work", run: () => void useStudyFile.getState().open() },
    ...knownSampleCases().map((c) => ({
      id: `open-sample-${c.id}`,
      title: `Open sample: ${c.title}`,
      icon: <Sparkles size={14} />,
      keywords: "sample example demo try first built-in tutorial",
      run: () => void w.openSample(c.id),
    })),
  ];
  if (summary) {
    const studyFile = useStudyFile.getState();
    list.push({ id: "save-study", title: "Save study", icon: <Save size={14} />, keywords: "file keep stannum edits work", run: () => void studyFile.save() });
    if (studyPath)
      list.push({ id: "save-study-as", title: "Save study as…", icon: <Save size={14} />, keywords: "file copy stannum", run: () => void studyFile.save({ as: true }) });
    for (const revision of [35, 33] as const)
      list.push({
        id: `export-raw-${revision}`,
        title: `Export case as PSS/E RAW ${revision}…`,
        icon: <Download size={14} />,
        keywords: "export raw file save write psse convert revision version",
        run: () => void studyFile.exportRaw(revision),
      });
  }
  if (summary?.power.available)
    list.push({ id: "solve", title: "Solve power flow", icon: <Play size={14} />, keywords: "run newton ac dc load flow", run: w.requestSolve });
  for (const m of MODULE_COMMANDS) list.push({ id: `go-${m.id}`, title: m.title, icon: m.icon, keywords: "module switch show", run: () => w.setModule(m.id) });
  if (split) list.push({ id: "split-close", title: "Close side by side", icon: <Columns2 size={14} />, keywords: "split view pane single", run: () => w.setSplit(null) });
  else
    for (const m of MODULE_COMMANDS)
      if (m.id !== module)
        list.push({
          id: `split-${m.id}`,
          title: `Open ${m.title.replace("Go to ", "")} side by side`,
          icon: <Columns2 size={14} />,
          keywords: `split view pane beside ${m.id === SPLIT_PARTNER[module] ? "default" : ""}`,
          run: () => w.setSplit(m.id),
        });
  if (summary?.topology.available) {
    // Topology lives in the Diagram: its tables drawer and its layers.
    const showDiagram = () => {
      if (w.module !== "diagram" && w.split !== "diagram") w.setModule("diagram");
    };
    list.push({
      id: "tables",
      title: "Show topology tables",
      icon: <Table2 size={14} />,
      keywords: "topology buses islands critical connections bridges equipment diagnostics nodal",
      run: () => {
        showDiagram();
        useLayers.getState().setTables(true);
      },
    });
    list.push({
      id: "layers",
      title: "Show layers",
      icon: <Layers size={14} />,
      keywords: "topology islands bridges cut buses articulation shortest path filters colours fields nodal",
      run: () => {
        showDiagram();
        useLayers.getState().setPanel(true);
      },
    });
  }
  if (summary) {
    list.push({
      id: "link",
      title: linked ? "Unlink selection" : "Link selection",
      icon: <Link2 size={14} />,
      keywords: "linked selection follow modules",
      run: () => w.setLinked(!linked),
    });
    if (summary.power.available) {
      list.push({
        id: "coordinates",
        title: summary.coordinates?.path ? "Attach other bus coordinates…" : "Attach bus coordinates…",
        icon: <MapPin size={14} />,
        keywords: "map geography latitude longitude csv location substation place",
        run: () => void w.attachCoordinates(),
      });
      if (summary.coordinates?.path)
        list.push({
          id: "coordinates-clear",
          title: `Detach bus coordinates (${summary.coordinates.name})`,
          icon: <MapPinOff size={14} />,
          keywords: "map geography latitude longitude csv location remove",
          run: () => void w.detachCoordinates(),
        });
    }
    if (selection) list.push({ id: "clear", title: "Clear selection", icon: <X size={14} />, keywords: "deselect escape", run: () => w.select(null, w.module) });
    list.push({ id: "close", title: "Close case", icon: <X size={14} />, keywords: "file", run: () => void w.closeCase() });
  }
  const runFile = useRunFile.getState();
  list.push(
    recording
      ? { id: "record-stop", title: "Stop recording and save run file…", icon: <Save size={14} />, keywords: "record macro script automation stnrun", run: () => void runFile.saveRecording() }
      : { id: "record", title: "Record a run file", icon: <Circle size={14} />, keywords: "record macro script automation stnrun replay", run: runFile.startRecording }
  );
  list.push({
    id: "shortcuts",
    title: "Keyboard shortcuts",
    icon: <Keyboard size={14} />,
    keywords: "keys hotkeys help accessibility ?",
    run: () => useShortcuts.getState().setOpen(true),
  });
  list.push({ id: "run-file", title: "Run a run file…", icon: <FileCode2 size={14} />, keywords: "play replay macro script automation stnrun batch", run: () => void runFile.run() });
  list.push({
    id: "theme",
    title: next === "system" ? "Use the system theme" : `Switch to ${next} theme`,
    icon: next === "dark" ? <Moon size={14} /> : <Sun size={14} />,
    keywords: "theme dark light appearance colour",
    run: () => useTheme.getState().setPreference(next),
  });
  list.push({
    id: "log",
    title: logOpen ? "Hide log" : "Show log",
    icon: <Terminal size={14} />,
    keywords: "console output messages progress errors warnings",
    run: () => useLog.getState().setOpen(!logOpen),
  });
  list.push({
    id: "save-diagnostics",
    title: "Save diagnostics…",
    icon: <LifeBuoy size={14} />,
    keywords: "report problem bug crash error log versions support",
    run: () => void saveDiagnostics(),
  });
  const updates = useUpdate.getState();
  if (updates.phase === "available" && updates.update)
    list.push({
      id: "install-update",
      title: `Install Stannum ${updates.update.version} and restart`,
      icon: <ArrowDownCircle size={14} />,
      keywords: "update upgrade new version release download install",
      run: () => void updates.install(),
    });
  list.push({
    id: "check-updates",
    title: "Check for updates",
    icon: <ArrowDownCircle size={14} />,
    keywords: "update upgrade new version release download install about",
    run: () => void updates.check(),
  });
  list.push({
    id: "auto-update-check",
    title: updates.autoCheck ? "Stop checking for updates at startup" : "Check for updates at startup",
    icon: updates.autoCheck ? <BellOff size={14} /> : <ArrowDownCircle size={14} />,
    keywords: "update upgrade automatic startup notify version",
    run: () => updates.setAutoCheck(!updates.autoCheck),
  });
  return list;
}

type Row = { kind: "command"; command: Command } | { kind: "element"; item: PaletteElement };

/** Opens with Ctrl+K (Cmd+K on macOS) from anywhere. */
export function CommandPalette() {
  const open = useWorkspace((s) => s.paletteOpen);
  const setOpen = useWorkspace((s) => s.setPaletteOpen);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(!useWorkspace.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, [setOpen]);

  if (!open) return null;
  return <Palette onClose={() => setOpen(false)} />;
}

function Palette({ onClose }: { onClose: () => void }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const commands = useCommands();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [elements, setElements] = useState<PaletteElement[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    setElements(null);
    setLoadError(null);
    if (!summary?.power.available) return;
    let live = true;
    caseNetwork()
      .then((net) => live && setElements(paletteElements(net)))
      .catch((e: Error) => live && setLoadError(e.message));
    return () => {
      live = false;
    };
  }, [caseVersion, summary?.power.available]);

  const rows: Row[] = useMemo(() => {
    const q = query.trim().toLowerCase();
    const cmds = commands.filter((c) => !q || `${c.title} ${c.keywords}`.toLowerCase().includes(q));
    const found = elements && q ? searchElements(elements, q) : [];
    // A query that looks like a bus number or name puts elements first.
    const elementsFirst = found.length > 0 && (/^\d/.test(q) || cmds.length === 0 || !cmds.some((c) => c.title.toLowerCase().startsWith(q)));
    const e: Row[] = found.map((item) => ({ kind: "element", item }));
    const c: Row[] = (q ? cmds : cmds.slice(0, 8)).map((command) => ({ kind: "command", command }));
    return elementsFirst ? [...e, ...c] : [...c, ...e];
  }, [query, commands, elements]);

  useEffect(() => setActive(0), [query]);
  // Back to where the keyboard was when the palette closes. Read while
  // rendering: by the time effects run, the search box has taken focus.
  const [before] = useState(() => document.activeElement as HTMLElement | null);
  useEffect(() => {
    return () => {
      if (before?.isConnected && before !== document.body) before.focus({ preventScroll: true });
    };
  }, [before]);
  useEffect(() => {
    listRef.current?.querySelector("[aria-selected=true]")?.scrollIntoView({ block: "nearest" });
  }, [active]);

  function choose(row: Row | undefined, draw: boolean) {
    if (!row) return;
    onClose();
    if (row.kind === "command") return row.command.run();
    const w = useWorkspace.getState();
    if (draw && centreBus(row.item.element) !== null && summary?.power.available) w.drawAround(row.item.element, w.module);
    else w.focus(row.item.element);
  }

  function onKeyDown(e: ReactKeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(rows.length - 1, a + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(0, a - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(rows[active], e.shiftKey);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Tab") {
      // Nothing else in the palette takes focus.
      e.preventDefault();
    }
  }

  const firstElement = rows.findIndex((r) => r.kind === "element");
  const firstCommand = rows.findIndex((r) => r.kind === "command");
  const hint = !summary
    ? "Open a case to search its buses and branches."
    : !summary.power.available
      ? "Element search needs a case rs-power can read."
      : loadError
        ? `Couldn't read the case: ${loadError}`
        : !elements
          ? "Reading the case…"
          : query.trim() && firstElement < 0
            ? `Nothing in ${summary.name} matches “${query.trim()}”.`
            : null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-[12vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Search and commands"
        className="w-[min(640px,92vw)] rounded-lg bg-white shadow-2xl border border-slate-200 flex flex-col overflow-hidden"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 px-3 border-b border-slate-200">
          <Search size={16} className="text-slate-500 shrink-0" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={summary?.power.available ? "Bus number or name, branch, generator, or a command…" : "Type a command…"}
            className="flex-1 py-3 text-sm bg-transparent outline-none text-slate-800 placeholder:text-slate-500"
            aria-label="Search"
            role="combobox"
            aria-expanded="true"
            aria-controls="palette-results"
            aria-autocomplete="list"
            aria-activedescendant={rows[active] ? `palette-option-${active}` : undefined}
          />
          <kbd className="text-[10px] text-slate-500 border border-slate-200 rounded px-1" aria-hidden="true">Esc</kbd>
        </div>
        <ul id="palette-results" ref={listRef} role="listbox" className="max-h-[52vh] overflow-y-auto py-1">
          {rows.map((row, i) => {
            const selected = i === active;
            const heading =
              i === firstElement ? "In this case" : i === firstCommand ? "Commands" : null;
            return (
              <li key={row.kind === "command" ? row.command.id : row.item.element} role="presentation">
                {heading && <p className="px-3 pt-2 pb-1 text-[10px] font-semibold tracking-wider uppercase text-slate-500" aria-hidden="true">{heading}</p>}
                <div
                  role="option"
                  id={`palette-option-${i}`}
                  aria-selected={selected}
                  className={`mx-1 flex items-center gap-2.5 rounded-md px-2.5 py-1.5 cursor-pointer ${selected ? "bg-teal-50" : ""}`}
                  onMouseMove={() => setActive(i)}
                  onClick={(e) => choose(row, e.shiftKey)}
                >
                  {row.kind === "command" ? (
                    <>
                      <span className="text-slate-500">{row.command.icon}</span>
                      <span className="text-sm text-slate-800">{row.command.title}</span>
                    </>
                  ) : (
                    <>
                      <span className="min-w-0 flex-1">
                        <span className="block text-sm text-slate-800 truncate">{row.item.title}</span>
                        <span className="block text-[11px] text-slate-500 truncate">{row.item.detail}</span>
                      </span>
                      {selected && summary?.power.available && centreBus(row.item.element) !== null && (
                        <button
                          type="button"
                          tabIndex={-1}
                          className="shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium text-teal-700 hover:bg-teal-100"
                          onClick={(e) => {
                            e.stopPropagation();
                            choose(row, true);
                          }}
                          title="Draw a one-line around it (Shift+Enter)"
                        >
                          <Wand2 size={12} /> Draw around
                        </button>
                      )}
                    </>
                  )}
                  {selected && <CornerDownLeft size={13} className="shrink-0 text-slate-500" aria-hidden="true" />}
                </div>
              </li>
            );
          })}
        </ul>
        {hint && <p role="status" className="px-3 py-2 text-[11px] text-slate-500 border-t border-slate-100">{hint}</p>}
        <p aria-hidden="true" className="px-3 py-1.5 text-[10.5px] text-slate-500 border-t border-slate-100 bg-slate-50 flex gap-3">
          <span>↑↓ choose</span>
          <span>Enter show everywhere</span>
          <span>Shift+Enter draw around</span>
        </p>
      </div>
    </div>
  );
}
