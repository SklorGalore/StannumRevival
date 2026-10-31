import type { ReactNode } from "react";
import { Columns2, FlaskConical, FolderOpen, ListChecks, Play, Search, Sigma } from "lucide-react";
import { useStudy } from "../store/studyStore";
import { SPLIT_PARTNER, useWorkspace } from "../store/workspaceStore";
import { SelectionChip } from "./SelectionChip";
import { ResultChip } from "./ResultChip";
import { ScriptChip } from "./ScriptChip";
import { useWidth } from "../ui/useWidth";
import { StudyFileButton } from "./StudyFileControls";
import { useStudyFile } from "../store/studyFileStore";
import { MOD_KEY } from "./appMenus";

const Divider = () => <span aria-hidden="true" className="mx-1 h-5 w-px shrink-0 bg-slate-200" />;

function ToolButton({
  icon,
  label,
  name,
  title,
  onClick,
  disabled,
  pressed,
  primary,
  narrow,
}: {
  icon: ReactNode;
  label: string;
  /** The accessible name, when it says more than the label. */
  name?: string;
  title: string;
  onClick: () => void;
  disabled?: boolean;
  pressed?: boolean;
  primary?: boolean;
  narrow: boolean;
}) {
  const look = primary
    ? "bg-teal-700 text-white hover:bg-teal-800 disabled:bg-slate-300 disabled:text-white"
    : pressed
      ? "bg-teal-50 text-teal-800 ring-1 ring-inset ring-teal-300 hover:bg-teal-100"
      : "text-slate-600 hover:bg-slate-100 hover:text-slate-800";
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-pressed={pressed}
      aria-label={name ?? label}
      title={title}
      className={`shrink-0 inline-flex items-center gap-1.5 rounded-md px-2 py-1.5 text-xs font-medium disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent ${look}`}
    >
      {icon}
      <span className={narrow ? "sr-only" : ""}>{label}</span>
    </button>
  );
}

/** The toolbar under the menu bar: the everyday actions, then what is selected and shown. */
export function CaseBar() {
  const summary = useWorkspace((s) => s.caseSummary);
  const opening = useWorkspace((s) => s.opening);
  const openError = useWorkspace((s) => s.openError);
  const openFile = useStudyFile((s) => s.openFile);
  const studyOpening = useStudyFile((s) => s.busy === "opening");
  const setPaletteOpen = useWorkspace((s) => s.setPaletteOpen);
  const module = useWorkspace((s) => s.module);
  const split = useWorkspace((s) => s.split);
  const setSplit = useWorkspace((s) => s.setSplit);
  const requestSolve = useWorkspace((s) => s.requestSolve);
  const requestSensitivities = useWorkspace((s) => s.requestSensitivities);
  const showPowerTab = useWorkspace((s) => s.showPowerTab);
  const solved = useWorkspace((s) => !!s.solveReport);
  const edits = useStudy((s) => s.entries.length);
  const showStudy = useStudy((s) => s.show);
  const power = !!summary?.power.available;
  const noPower = summary ? "This case has no power flow data" : "Open a case first";

  // In a narrow window the buttons drop their labels, and the selection's
  // "Show in" links fold into a menu, rather than running into each other.
  const [ref, width] = useWidth<HTMLDivElement>(1440);
  const narrow = width < 1100;
  return (
    <div ref={ref} role="toolbar" aria-label="Case" className="h-10 shrink-0 flex items-center gap-1 px-3 border-b border-slate-200 bg-white whitespace-nowrap">
      <ToolButton
        icon={<FolderOpen size={14} />}
        label="Open"
        name="Open case or run file…"
        title="Open a case or a study"
        onClick={() => void openFile()}
        disabled={opening || studyOpening}
        primary={!summary}
        narrow={narrow && !!summary}
      />
      <StudyFileButton narrow={narrow} />
      {summary && (
        <>
          <Divider />
          <ToolButton
            icon={<Play size={14} />}
            label="Solve"
            name="Solve power flow"
            title={power ? "Solve the power flow with the current settings" : noPower}
            onClick={requestSolve}
            disabled={!power}
            primary
            narrow={narrow}
          />
          <ToolButton
            icon={<Sigma size={14} />}
            label="Sensitivities"
            title={power ? "Calculate sensitivities around the current operating point" : noPower}
            onClick={requestSensitivities}
            disabled={!power}
            narrow={narrow}
          />
          <ToolButton
            icon={<ListChecks size={14} />}
            label="Contingencies"
            name="Show contingencies"
            title={solved ? "Run outages against the solved case" : "Solve the case first"}
            onClick={() => showPowerTab("contingencies")}
            disabled={!solved}
            narrow={narrow}
          />
          <Divider />
          <ToolButton
            icon={<Columns2 size={14} />}
            label="Side by side"
            title={split ? "Back to one module at a time" : "Show two modules side by side, with linked selection"}
            onClick={() => setSplit(split ? null : SPLIT_PARTNER[module])}
            pressed={!!split}
            narrow={narrow}
          />
        </>
      )}
      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        className="ml-2 shrink-0 inline-flex items-center gap-2 rounded-md border border-slate-300 px-2.5 py-1 text-xs text-slate-500 hover:bg-slate-50 hover:text-slate-700"
        title="Find a bus, branch or generator, or run a command"
        aria-keyshortcuts="Control+K"
      >
        <Search size={13} aria-hidden="true" />
        <span className={narrow ? "sr-only" : ""}>{summary ? "Find in case" : "Commands"}</span>
        <kbd className="rounded border border-slate-200 px-1 text-[10px] text-slate-500" aria-hidden="true">
          {MOD_KEY.trim() === "⌘" ? "⌘K" : "Ctrl K"}
        </kbd>
      </button>
      <div className="ml-auto min-w-0 flex items-center justify-end gap-2">
        {openError && (
          <span role="alert" className="min-w-0 text-[11px] text-red-700 truncate" title={openError}>
            {openError}
          </span>
        )}
        {summary && edits > 0 && (
          <button
            type="button"
            onClick={showStudy}
            className="shrink-0 inline-flex items-center gap-1 rounded-full border border-amber-300 bg-amber-50 px-2 py-0.5 text-[11px] font-medium text-amber-900 hover:bg-amber-100"
            title="Power flow is solving this case with study edits. Click to see them."
            data-testid="study-badge"
          >
            <FlaskConical size={12} /> {edits} edit{edits === 1 ? "" : "s"}
          </button>
        )}
        {summary && <ResultChip />}
        <SelectionChip />
        <ScriptChip />
      </div>
    </div>
  );
}
