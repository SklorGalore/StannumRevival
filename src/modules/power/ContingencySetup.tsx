// The contingency setup: what to take out, what to check, the subsystems
// both pick from, the limits problems are judged by, and the AC solves. A
// dialog, so the Contingencies tab keeps just a summary and Run.
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { FileText, FolderOpen, Play, Plus, Save, Trash2 } from "lucide-react";
import { PLANNING_CATEGORIES, useContingencies, type ContingencyChoice, type ContingencySource, type SolveOverrides } from "../../store/contingencyStore";
import { LISTS_FILTERS, api, pickFile, saveTextExport } from "../../lib/analysis/api";
import type { CategoryCount, SolveReport, SolveSettings } from "../../lib/analysis/types";
import type { ListedSubsystem } from "../../lib/analysis/commands.gen";
import { count } from "../../lib/analysis/format";
import { RATINGS, ratingLabel, type Rating } from "../../lib/analysis/ratings";
import { describeSubsystem, subsystemId, subsystemsText, type SetupSubsystem } from "../../lib/analysis/stnl";
import { Button, ErrorBanner, inputClass } from "../../ui/primitives";
import { Modal } from "../../ui/Modal";
import { useCaseGroups, type CaseGroups } from "../../ui/useCaseGroups";

export type SetupTab = "outages" | "monitor" | "subsystems" | "limits" | "solution";

const TABS: { id: SetupTab; label: string; hint: string }[] = [
  { id: "outages", label: "Outages", hint: "What to take out" },
  { id: "monitor", label: "Monitor", hint: "What to check after each outage" },
  { id: "subsystems", label: "Subsystems", hint: "Sets of buses to pick from" },
  { id: "limits", label: "Limits & reporting", hint: "Ratings and what counts as a problem" },
  { id: "solution", label: "Solution", hint: "Screening and AC solves" },
];

/** The last part of a path, for showing a chosen file. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** The most pairs the core runs, as in stannum-core's contingency::MAX_PAIRS. */
const MAX_PAIRS = 250_000;

/** How many pairs N-2 of `n` elements makes, and whether that is too many. */
function pairNote(n: number): string {
  const pairs = (n * (n - 1)) / 2;
  const base = `${count(n)} elements make ${count(pairs)} pairs.`;
  return pairs > MAX_PAIRS ? `${base} That is more than ${count(MAX_PAIRS)}: narrow them with a subsystem or the bus filters.` : base;
}

/** The subsystem a choice names, if the setup still has it. */
export function chosenSubsystem(c: ContingencyChoice, id: string | null): SetupSubsystem | null {
  return (id && c.subsystems.find((s) => s.id === id)) || null;
}

const SOLVE_OPTIONS: { key: keyof SolveOverrides; label: string; hint: string }[] = [
  { key: "enforce_q_limits", label: "Generator Q limits", hint: "Hold generators within their reactive limits" },
  { key: "adjust_transformer_taps", label: "Transformer taps", hint: "Let regulating transformers move their taps" },
  { key: "switch_shunts", label: "Switched shunts", hint: "Let switched shunts step to hold their voltages" },
];

/** The setup in one line, for the Contingencies tab. */
export function setupSummary(c: ContingencyChoice, scoped: boolean): string {
  const parts: string[] = [];
  if (c.source === "planning") parts.push(`Planning events (${c.categories.length} categor${c.categories.length === 1 ? "y" : "ies"}, ${c.kvMin} kV and up)`);
  else if (c.source === "lists") parts.push(c.listsPath ? `Contingencies of ${fileName(c.listsPath)}` : "Contingencies of a lists file (none chosen)");
  else {
    const kinds = [c.branches && "branches", c.generators && "generators", c.buses && "buses"].filter(Boolean).join(", ") || "nothing";
    const within = chosenSubsystem(c, c.outageSubsystem);
    parts.push(`${c.pairs ? "N-2" : "N-1"} of ${kinds}${within ? ` in ${within.name || within.id}` : scoped ? " touching the filtered buses" : ""}`);
  }
  const watched = chosenSubsystem(c, c.monitorSubsystem);
  parts.push(
    c.monitorFromLists
      ? `checks the monitors of ${c.monitorsPath ? fileName(c.monitorsPath) : "a lists file"}`
      : watched
        ? `checks ${watched.name || watched.id}${c.monitorVoltages ? "" : " (flows only)"}`
        : "checks every branch and bus"
  );
  const rating = c.rating === c.baseRating ? ratingLabel(c.rating) : `${ratingLabel(c.baseRating)} before, ${ratingLabel(c.rating)} after`;
  parts.push(`${rating} at ${c.limitPct}%${c.reportExisting ? ", existing problems included" : c.worseByPct !== 1 ? `, worse by ${c.worseByPct} pts` : ""}`);
  parts.push(c.method === "ac" ? "AC for every outage" : `linear screen, AC for the worst ${count(c.acCount)}`);
  return parts.join(" · ");
}

export function ContingencySetup({
  report,
  solve,
  scope,
  canRun,
  runTitle,
  onRun,
  onClose,
  initialTab = "outages",
}: {
  report: SolveReport | null;
  solve: SolveSettings;
  scope: { elements: string[]; buses: number } | null;
  canRun: boolean;
  runTitle?: string;
  onRun: () => void;
  onClose: () => void;
  initialTab?: SetupTab;
}) {
  const [tab, setTab] = useState<SetupTab>(initialTab);
  const choice = useContingencies((s) => s.choice);
  const groups = useCaseGroups();
  const areaNames = useMemo(() => new Map((report?.areas ?? []).map((a) => [a.number, a.name.trim()])), [report]);
  return (
    <Modal label="Contingency setup" onClose={onClose} className="w-[min(960px,calc(100vw-2rem))] h-[min(640px,calc(100vh-2rem))] flex flex-col">
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-200">
        <h2 className="text-sm font-semibold text-slate-800">Contingency setup</h2>
        <span className="text-[11px] text-slate-500">Subsystems save to .stnl lists files</span>
      </div>
      <div className="flex flex-1 min-h-0">
        <nav role="tablist" aria-label="Setup steps" aria-orientation="vertical" className="w-48 shrink-0 border-r border-slate-200 bg-slate-50 p-2 flex flex-col gap-0.5">
          {TABS.map((t, i) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              onClick={() => setTab(t.id)}
              data-testid={`setup-tab-${t.id}`}
              className={`text-left rounded px-2 py-1.5 ${tab === t.id ? "bg-white shadow-sm ring-1 ring-slate-200" : "hover:bg-slate-100"}`}
            >
              <span className="block text-xs font-medium text-slate-800">
                <span className="tabular-nums text-slate-400 mr-1.5">{i + 1}</span>
                {t.label}
              </span>
              <span className="block text-[11px] text-slate-500 ml-4">{t.hint}</span>
            </button>
          ))}
        </nav>
        <div role="tabpanel" className="flex-1 min-w-0 overflow-auto p-4 text-xs" data-testid={`setup-panel-${tab}`}>
          {tab === "outages" && <OutagesPanel report={report} scope={scope} onSubsystems={() => setTab("subsystems")} />}
          {tab === "monitor" && <MonitorPanel onSubsystems={() => setTab("subsystems")} />}
          {tab === "subsystems" && <SubsystemsPanel groups={groups} areaNames={areaNames} />}
          {tab === "limits" && <LimitsPanel />}
          {tab === "solution" && <SolutionPanel solve={solve} />}
        </div>
      </div>
      <div className="flex items-center gap-2 px-4 py-3 border-t border-slate-200">
        <span className="text-[11px] text-slate-500 min-w-0 flex-1 truncate" data-testid="setup-summary">
          {setupSummary(choice, !!scope)}
        </span>
        {tab !== "solution" && (
          <Button type="button" onClick={() => setTab(TABS[TABS.findIndex((t) => t.id === tab) + 1].id)}>
            Next
          </Button>
        )}
        <Button type="button" onClick={onClose} data-testid="setup-done">
          Done
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={!canRun}
          title={runTitle}
          onClick={() => {
            onClose();
            onRun();
          }}
          data-testid="setup-run"
          className="whitespace-nowrap"
        >
          <Play size={13} /> Run contingencies
        </Button>
      </div>
    </Modal>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600" title={hint}>
      {label}
      {children}
    </label>
  );
}

function Group({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  return (
    <section className="mb-5">
      <h3 className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase mb-2">{title}</h3>
      <div className="flex flex-wrap items-end gap-x-5 gap-y-3">{children}</div>
      {note && <p className="mt-2 text-[11px] text-slate-500 max-w-2xl">{note}</p>}
    </section>
  );
}

function ListsFile({ path, onPick, testId }: { path: string | null; onPick: (p: string) => void; testId: string }) {
  return (
    <div className="flex items-center gap-2 min-w-0">
      <Button
        type="button"
        onClick={async () => {
          const p = await pickFile(LISTS_FILTERS);
          if (p) onPick(p);
        }}
        data-testid={testId}
      >
        <FileText size={13} /> {path ? "Change…" : "Choose…"}
      </Button>
      <span className="truncate max-w-72 text-[11px] text-slate-600" title={path ?? undefined}>
        {path ? fileName(path) : "No file chosen"}
      </span>
    </div>
  );
}

function SubsystemSelect({
  value,
  onChange,
  none,
  label,
  testId,
  onSubsystems,
}: {
  value: string | null;
  onChange: (id: string | null) => void;
  none: string;
  label: string;
  testId: string;
  onSubsystems: () => void;
}) {
  const subsystems = useContingencies((s) => s.choice.subsystems);
  return (
    <div className="flex items-end gap-2">
      <Field label={label}>
        <select className={`${inputClass} min-w-56`} value={value ?? ""} onChange={(e) => onChange(e.target.value || null)} data-testid={testId}>
          <option value="">{none}</option>
          {subsystems.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name || s.id} ({describeSubsystem(s)})
            </option>
          ))}
        </select>
      </Field>
      <Button type="button" variant="ghost" onClick={onSubsystems}>
        {subsystems.length ? "Edit subsystems…" : "Create a subsystem…"}
      </Button>
    </div>
  );
}

function OutagesPanel({
  report,
  scope,
  onSubsystems,
}: {
  report: SolveReport | null;
  scope: { elements: string[]; buses: number } | null;
  onSubsystems: () => void;
}) {
  const s = useContingencies();
  const c = s.choice;
  const planning = c.source === "planning";
  const loadCounts = s.loadCounts;
  useEffect(() => {
    if (planning) void loadCounts();
  }, [planning, report, c.kvMin, loadCounts]);
  const chosenCount = (s.counts ?? []).filter((k) => c.categories.includes(k.category)).reduce((n, k) => n + k.count, 0);
  const within = chosenSubsystem(c, c.outageSubsystem);
  const pairElements = scope
    ? scope.elements.filter((e) => !e.startsWith("bus:")).length
    : (c.branches ? new Set(report?.branches.filter((b) => b.online && !b.element.startsWith("switch:")).map((b) => b.element)).size : 0) +
      (c.generators ? (report?.generators.filter((g) => g.online).length ?? 0) : 0);
  return (
    <>
      <Group title="Source">
        <Field label="Outages">
          <select
            className={inputClass}
            value={c.source}
            onChange={(e) => s.setChoice({ source: e.target.value as ContingencySource })}
            aria-label="Outages"
            data-testid="contingency-source"
          >
            <option value="n1">N-1 or N-2: elements of the case</option>
            <option value="planning">NERC TPL-001 planning events</option>
            <option value="lists">From a lists file (.stnl)</option>
          </select>
        </Field>
        {c.source === "lists" && (
          <Field label="Lists file">
            <ListsFile path={c.listsPath} onPick={(p) => s.setChoice({ listsPath: p })} testId="choose-lists" />
          </Field>
        )}
      </Group>
      {c.source === "n1" && (
        <>
          <Group title="Take out" note={c.pairs ? <span data-testid="pair-count">{within ? "Pairs of branches in the subsystem; generators and buses stay one at a time." : pairNote(pairElements)}</span> : undefined}>
            <div className="flex items-center gap-4 pb-2">
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={c.branches} onChange={(e) => s.setChoice({ branches: e.target.checked })} />
                Branches
              </label>
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={c.generators} onChange={(e) => s.setChoice({ generators: e.target.checked })} />
                Generators
              </label>
              <label className="flex items-center gap-1.5" title="Each bus with every branch and DC line connected to it, cutting off its loads, generators and shunts">
                <input type="checkbox" checked={c.buses} onChange={(e) => s.setChoice({ buses: e.target.checked })} data-testid="outage-buses" />
                Buses
              </label>
            </div>
            <Field label="How many at a time">
              <select
                className={inputClass}
                value={c.pairs ? "pairs" : "single"}
                onChange={(e) => s.setChoice({ pairs: e.target.value === "pairs" })}
                aria-label="How many at a time"
                data-testid="outage-depth"
              >
                <option value="single">One at a time (N-1)</option>
                <option value="pairs">Two at a time (N-2)</option>
              </select>
            </Field>
          </Group>
          <Group
            title="Where"
            note={
              within
                ? `Only elements in ${within.name || within.id}: ${describeSubsystem(within)}. A branch counts when either end is in it.`
                : scope
                  ? `The bus filters narrow the outages to ${count(scope.elements.length)} branches and generators touching ${count(scope.buses)} matching buses.`
                  : undefined
            }
          >
            <SubsystemSelect
              label="Within"
              none="The whole case"
              value={c.outageSubsystem}
              onChange={(id) => s.setChoice({ outageSubsystem: id })}
              testId="outage-subsystem"
              onSubsystems={onSubsystems}
            />
          </Group>
        </>
      )}
      {planning && (
        <Group
          title="Planning events"
          note={
            s.counts && (
              <span data-testid="planning-total">
                {count(chosenCount)} event{chosenCount === 1 ? "" : "s"} in the categories chosen. The base case is always solved in AC; so are the largest shunt outages, as
                many as are checked in AC, since the screen can't see what a shunt does.
              </span>
            )
          }
        >
          <PlanningCategories counts={s.counts} chosen={c.categories} onChange={(categories) => s.setChoice({ categories })} />
          <Field label="At or above (kV)" hint="Branches, shunts and DC lines with a terminal at or above this voltage. Generators are not filtered by voltage.">
            <input
              className={`${inputClass} w-20`}
              type="number"
              min={0}
              step="any"
              value={c.kvMin}
              onChange={(e) => s.setChoice({ kvMin: Math.max(0, Number(e.target.value) || 0) })}
              aria-label="Lowest voltage"
            />
          </Field>
        </Group>
      )}
      <Group title="Zero-impedance lines" note="Lines with no resistance and a reactance at or below the threshold are left out, as PSS/E treats them (THRSHZ). A bus outage still opens them.">
        <label className="flex items-center gap-1.5 pb-1.5">
          <input type="checkbox" checked={c.skipZeroImpedance} onChange={(e) => s.setChoice({ skipZeroImpedance: e.target.checked })} data-testid="skip-zero-impedance" />
          Skip them
        </label>
        <Field label="Threshold |X| (p.u.)">
          <input
            className={`${inputClass} w-24`}
            type="number"
            min={0}
            step="any"
            value={c.zeroImpedancePu}
            disabled={!c.skipZeroImpedance}
            onChange={(e) => s.setChoice({ zeroImpedancePu: Math.max(0, Number(e.target.value) || 0) })}
            aria-label="Zero-impedance threshold, p.u."
          />
        </Field>
      </Group>
    </>
  );
}

/** The planning event categories, each with how many events it gives on the case. */
function PlanningCategories({ counts, chosen, onChange }: { counts: CategoryCount[] | null; chosen: string[]; onChange: (categories: string[]) => void }) {
  const label = (c: string) => counts?.find((k) => k.category === c);
  const toggle = (c: string, on: boolean) => onChange(PLANNING_CATEGORIES.filter((k) => (k === c ? on : chosen.includes(k))));
  return (
    <fieldset className="flex flex-col gap-1" data-testid="planning-categories">
      <legend className="sr-only">Planning events</legend>
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5">
        {PLANNING_CATEGORIES.map((c) => {
          const info = label(c);
          return (
            <label key={c} className="flex items-center gap-1.5 whitespace-nowrap" title={info?.label}>
              <input type="checkbox" checked={chosen.includes(c)} onChange={(e) => toggle(c, e.target.checked)} />
              <span className="font-semibold tabular-nums">{c}</span>
              <span className="text-slate-600">{info?.label ?? ""}</span>
              {info && <span className="tabular-nums text-slate-400">{count(info.count)}</span>}
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}

function MonitorPanel({ onSubsystems }: { onSubsystems: () => void }) {
  const s = useContingencies();
  const c = s.choice;
  const mode = c.monitorFromLists ? "lists" : c.monitorSubsystem ? "subsystem" : "all";
  const pick = (m: string) =>
    s.setChoice(
      m === "lists"
        ? { monitorFromLists: true, monitorsPath: c.monitorsPath ?? c.listsPath }
        : m === "subsystem"
          ? { monitorFromLists: false, monitorSubsystem: c.monitorSubsystem ?? c.subsystems[0]?.id ?? null }
          : { monitorFromLists: false, monitorSubsystem: null }
    );
  const radio = (value: string, label: string, hint: string) => (
    <label className="flex items-start gap-2 py-1">
      <input type="radio" name="monitor-mode" className="mt-0.5" checked={mode === value} onChange={() => pick(value)} data-testid={`monitor-${value}`} />
      <span>
        <span className="block font-medium text-slate-800">{label}</span>
        <span className="block text-[11px] text-slate-500">{hint}</span>
      </span>
    </label>
  );
  return (
    <>
      <Group title="Checked after each outage">
        <div className="flex flex-col">
          {radio("all", "Every branch and bus", "Every rated branch at the limit, and every bus at its own voltage limits.")}
          {radio("subsystem", "A subsystem", "Branches with either end in it, at the limit and ratings set under Limits; and, if chosen, its buses' voltages.")}
          {radio("lists", "Monitors from a lists file", "Only what the file's monitor records name, with their own limits, and its interfaces.")}
        </div>
      </Group>
      {mode === "subsystem" && (
        <Group title="Subsystem">
          <SubsystemSelect
            label="Monitor"
            none="(choose one)"
            value={c.monitorSubsystem}
            onChange={(id) => s.setChoice({ monitorSubsystem: id })}
            testId="monitor-subsystem"
            onSubsystems={onSubsystems}
          />
          <label className="flex items-center gap-1.5 pb-1.5">
            <input type="checkbox" checked={c.monitorVoltages} onChange={(e) => s.setChoice({ monitorVoltages: e.target.checked })} />
            Bus voltages too
          </label>
        </Group>
      )}
      {mode === "lists" && (
        <Group title="Lists file">
          <ListsFile path={c.monitorsPath} onPick={(p) => s.setChoice({ monitorsPath: p })} testId="choose-monitors" />
        </Group>
      )}
    </>
  );
}

function parseNumbers(text: string): number[] {
  return [...new Set(text.split(/[\s,;]+/).filter(Boolean).map(Number).filter((n) => Number.isInteger(n) && n > 0))];
}

function SubsystemsPanel({ groups, areaNames }: { groups: CaseGroups | null; areaNames: Map<number, string> }) {
  const s = useContingencies();
  const list = s.choice.subsystems;
  const [selected, setSelected] = useState<string | null>(list[0]?.id ?? null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const current = list.find((x) => x.id === selected) ?? null;
  const set = (subsystems: SetupSubsystem[]) => s.setChoice({ subsystems });
  const update = (patch: Partial<SetupSubsystem>) => current && set(list.map((x) => (x.id === current.id ? { ...x, ...patch } : x)));
  const add = () => {
    const name = `Subsystem ${list.length + 1}`;
    const sub: SetupSubsystem = { id: subsystemId(name, list.map((x) => x.id)), name, areas: [], zones: [], owners: [], buses: [], kvMin: null, kvMax: null };
    set([...list, sub]);
    setSelected(sub.id);
  };
  const remove = () => {
    if (!current) return;
    const rest = list.filter((x) => x.id !== current.id);
    set(rest);
    s.setChoice({
      outageSubsystem: s.choice.outageSubsystem === current.id ? null : s.choice.outageSubsystem,
      monitorSubsystem: s.choice.monitorSubsystem === current.id ? null : s.choice.monitorSubsystem,
    });
    setSelected(rest[0]?.id ?? null);
  };
  async function load() {
    setError(null);
    setNotice(null);
    const path = await pickFile(LISTS_FILTERS);
    if (!path) return;
    try {
      const read: ListedSubsystem[] = await api.listsSubsystems(path);
      const nested = read.filter((r) => r.subsystems.length).length;
      const incoming: SetupSubsystem[] = read.map((r) => ({
        id: r.id,
        name: r.name,
        areas: r.areas ?? [],
        zones: r.zones ?? [],
        owners: r.owners ?? [],
        buses: r.buses ?? [],
        kvMin: r.kv_min ?? null,
        kvMax: r.kv_max ?? null,
      }));
      const ids = new Set(incoming.map((x) => x.id));
      set([...list.filter((x) => !ids.has(x.id)), ...incoming]);
      setSelected(incoming[0]?.id ?? selected);
      setNotice(
        `Read ${incoming.length} subsystem${incoming.length === 1 ? "" : "s"} from ${fileName(path)}.` +
          (nested ? ` ${nested} include other subsystems; those links aren't kept here.` : "")
      );
    } catch (e) {
      setError((e as Error).message);
    }
  }
  async function save() {
    setError(null);
    await saveTextExport("subsystems.stnl", subsystemsText(list), LISTS_FILTERS[0]).catch((e) => setError((e as Error).message));
  }
  return (
    <div className="flex gap-4 h-full min-h-0">
      <div className="w-56 shrink-0 flex flex-col gap-2 min-h-0">
        <div className="flex items-center gap-1">
          <Button type="button" onClick={add} data-testid="subsystem-add">
            <Plus size={13} /> New
          </Button>
          <Button type="button" variant="ghost" onClick={load} title="Read subsystems from a .stnl lists file" data-testid="subsystems-load">
            <FolderOpen size={13} /> Load…
          </Button>
          <Button type="button" variant="ghost" onClick={save} disabled={!list.length} title="Save these subsystems as a .stnl lists file" data-testid="subsystems-save">
            <Save size={13} /> Save…
          </Button>
        </div>
        <ul className="flex-1 min-h-0 overflow-auto border border-slate-200 rounded" role="listbox" aria-label="Subsystems" data-testid="subsystem-list">
          {list.length === 0 && <li className="p-2 text-[11px] text-slate-500">No subsystems yet. Make one, or load them from a .stnl file.</li>}
          {list.map((x) => (
            <li key={x.id}>
              <button
                type="button"
                role="option"
                aria-selected={x.id === selected}
                onClick={() => setSelected(x.id)}
                className={`w-full text-left px-2 py-1.5 border-b border-slate-100 ${x.id === selected ? "bg-teal-50" : "hover:bg-slate-50"}`}
              >
                <span className="block font-medium text-slate-800 truncate">{x.name || x.id}</span>
                <span className="block text-[11px] text-slate-500 truncate">{describeSubsystem(x)}</span>
              </button>
            </li>
          ))}
        </ul>
        {notice && <p className="text-[11px] text-slate-500">{notice}</p>}
        {error && <ErrorBanner>{error}</ErrorBanner>}
      </div>
      <div className="flex-1 min-w-0 overflow-auto" data-testid="subsystem-editor">
        {!current ? (
          <p className="text-slate-500">Pick a subsystem to edit. Listed areas, zones, owners and buses combine; the kV range then narrows them.</p>
        ) : (
          <>
            <div className="flex items-end gap-4 mb-4">
              <Field label="Name">
                <input
                  className={`${inputClass} w-56`}
                  value={current.name}
                  onChange={(e) => update({ name: e.target.value })}
                  aria-label="Subsystem name"
                  data-testid="subsystem-name"
                />
              </Field>
              <span className="text-[11px] text-slate-500 pb-1.5">
                Id <code>{current.id}</code>
              </span>
              <Button type="button" variant="ghost" className="ml-auto" onClick={remove} data-testid="subsystem-delete">
                <Trash2 size={13} /> Delete
              </Button>
            </div>
            <div className="grid grid-cols-3 gap-4 mb-4">
              <Picks title="Areas" options={groups?.area ?? []} names={areaNames} chosen={current.areas} onChange={(areas) => update({ areas })} testId="subsystem-areas" />
              <Picks title="Zones" options={groups?.zone ?? []} chosen={current.zones} onChange={(zones) => update({ zones })} testId="subsystem-zones" />
              <Picks title="Owners" options={groups?.owner ?? []} chosen={current.owners} onChange={(owners) => update({ owners })} testId="subsystem-owners" />
            </div>
            <div className="flex flex-wrap items-end gap-4">
              <Field label="Buses" hint="Bus numbers, separated by commas or spaces">
                <BusesInput value={current.buses} onChange={(buses) => update({ buses })} />
              </Field>
              <Field label="From (kV)">
                <input
                  className={`${inputClass} w-20`}
                  type="number"
                  min={0}
                  step="any"
                  value={current.kvMin ?? ""}
                  placeholder="any"
                  onChange={(e) => update({ kvMin: e.target.value === "" ? null : Math.max(0, Number(e.target.value)) })}
                  aria-label="Lowest voltage, kV"
                  data-testid="subsystem-kv-min"
                />
              </Field>
              <Field label="To (kV)">
                <input
                  className={`${inputClass} w-20`}
                  type="number"
                  min={0}
                  step="any"
                  value={current.kvMax ?? ""}
                  placeholder="any"
                  onChange={(e) => update({ kvMax: e.target.value === "" ? null : Math.max(0, Number(e.target.value)) })}
                  aria-label="Highest voltage, kV"
                />
              </Field>
            </div>
            <p className="mt-3 text-[11px] text-slate-500">
              {describeSubsystem(current)}. With nothing listed, every bus is in it, narrowed by the kV range.
            </p>
          </>
        )}
      </div>
    </div>
  );
}

function BusesInput({ value, onChange }: { value: number[]; onChange: (buses: number[]) => void }) {
  const [text, setText] = useState(value.join(", "));
  useEffect(() => {
    if (parseNumbers(text).join() !== value.join()) setText(value.join(", "));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);
  return (
    <input
      className={`${inputClass} w-72`}
      value={text}
      placeholder="101, 102, 205"
      onChange={(e) => {
        setText(e.target.value);
        onChange(parseNumbers(e.target.value));
      }}
      aria-label="Bus numbers"
      data-testid="subsystem-buses"
    />
  );
}

function Picks({
  title,
  options,
  names,
  chosen,
  onChange,
  testId,
}: {
  title: string;
  options: [number, number][];
  names?: Map<number, string>;
  chosen: number[];
  onChange: (v: number[]) => void;
  testId: string;
}) {
  const [filter, setFilter] = useState("");
  const shown = options.filter(([v]) => !filter || `${v} ${names?.get(v) ?? ""}`.toLowerCase().includes(filter.toLowerCase()));
  return (
    <fieldset className="flex flex-col gap-1 min-w-0" data-testid={testId}>
      <legend className="text-[11px] font-medium text-slate-600 mb-1">
        {title}
        {chosen.length ? <span className="text-slate-400"> ({chosen.length} chosen)</span> : null}
      </legend>
      {options.length > 8 && (
        <input className={`${inputClass} py-0.5`} placeholder="Filter…" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label={`Filter ${title.toLowerCase()}`} />
      )}
      <div className="h-40 overflow-auto border border-slate-200 rounded p-1">
        {options.length === 0 && <p className="p-1 text-[11px] text-slate-400">None in this case</p>}
        {shown.map(([v, n]) => (
          <label key={v} className="flex items-center gap-1.5 px-1 py-0.5 whitespace-nowrap hover:bg-slate-50 rounded">
            <input type="checkbox" checked={chosen.includes(v)} onChange={(e) => onChange(e.target.checked ? [...chosen, v].sort((a, b) => a - b) : chosen.filter((x) => x !== v))} />
            <span className="tabular-nums font-medium">{v}</span>
            {names?.get(v) && <span className="truncate text-slate-600">{names.get(v)}</span>}
            <span className="ml-auto tabular-nums text-slate-400">{count(n)}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

function RatingSelect({ value, onChange, label, hint, testId }: { value: Rating; onChange: (r: Rating) => void; label: string; hint: string; testId: string }) {
  return (
    <Field label={label} hint={hint}>
      <select className={inputClass} value={value} onChange={(e) => onChange(e.target.value as Rating)} aria-label={label} data-testid={testId}>
        {RATINGS.map((r) => (
          <option key={r.value} value={r.value}>
            {r.label} ({r.hint})
          </option>
        ))}
      </select>
    </Field>
  );
}

function LimitsPanel() {
  const s = useContingencies();
  const c = s.choice;
  return (
    <>
      <Group title="Ratings" note="A common planning choice is the normal rating before the outage and the short-term rating after it. A branch whose chosen rating is zero is unrated and never overloads.">
        <RatingSelect
          label="Base case rating"
          hint="The rating for loadings before an outage, and for the base case (P0)"
          value={c.baseRating}
          onChange={(baseRating) => s.setChoice({ baseRating })}
          testId="base-rating"
        />
        <RatingSelect
          label="Contingency rating"
          hint="The rating for loadings after an outage"
          value={c.rating}
          onChange={(rating) => s.setChoice({ rating })}
          testId="contingency-rating"
        />
        <Field label="Overloaded above (%)">
          <input
            className={`${inputClass} w-20`}
            type="number"
            min={1}
            step="any"
            value={c.limitPct}
            onChange={(e) => s.setChoice({ limitPct: Math.max(1, Number(e.target.value) || 100) })}
            aria-label="Overload limit"
          />
        </Field>
      </Group>
      <Group
        title="What counts as a problem"
        note="A branch over the limit, or a bus outside its voltage limits, counts when it wasn't before the outage. One that already was counts only when it gets worse by more than these amounts, so problems in the base case don't fill every outage's list. Include existing counts them all."
      >
        <Field label="Overloads worse by (points)" hint="An overload already there before the outage is reported only when it grows by more than this many points">
          <input
            className={`${inputClass} w-24`}
            type="number"
            min={0}
            step="any"
            value={c.worseByPct}
            disabled={c.reportExisting}
            onChange={(e) => s.setChoice({ worseByPct: Math.max(0, Number(e.target.value) || 0) })}
            aria-label="Worse by points"
            data-testid="contingency-worse-by"
          />
        </Field>
        <Field label="Voltages worse by (p.u.)" hint="A voltage problem already there before the outage is reported only when it moves further out by more than this">
          <input
            className={`${inputClass} w-24`}
            type="number"
            min={0}
            step="any"
            value={c.worseByPu}
            disabled={c.reportExisting}
            onChange={(e) => s.setChoice({ worseByPu: Math.max(0, Number(e.target.value) || 0) })}
            aria-label="Worse by per unit"
            data-testid="contingency-worse-by-pu"
          />
        </Field>
        <label className="flex items-center gap-1.5 pb-1.5" title="Also report overloads and voltage problems that were there before the outage and got no worse">
          <input type="checkbox" checked={c.reportExisting} onChange={(e) => s.setChoice({ reportExisting: e.target.checked })} data-testid="contingency-report-existing" />
          Include existing problems
        </label>
      </Group>
    </>
  );
}

function SolutionPanel({ solve }: { solve: SolveSettings }) {
  const s = useContingencies();
  const c = s.choice;
  const setOverride = (key: keyof SolveOverrides, v: string) => {
    const next: SolveOverrides = { ...c.solveOverrides };
    if (v === "same") delete next[key];
    else next[key] = v === "on";
    s.setChoice({ solveOverrides: next });
  };
  return (
    <>
      <Group
        title="Method"
        note={
          c.method === "screen"
            ? "Every outage is screened with linear factors from one factorization, then the worst are solved in full AC. The base case and the largest shunt outages are always solved in AC."
            : "Every outage gets a full AC solve: slower, and finds voltage problems everywhere."
        }
      >
        <Field label="Method">
          <select className={inputClass} value={c.method} onChange={(e) => s.setChoice({ method: e.target.value as "screen" | "ac" })} aria-label="Method" data-testid="contingency-method">
            <option value="screen">Linear screen, then AC for the worst</option>
            <option value="ac">AC solve of every outage</option>
          </select>
        </Field>
        {c.method === "screen" && (
          <Field label="Worst checked in AC">
            <input
              className={`${inputClass} w-24`}
              type="number"
              min={0}
              step={1}
              value={c.acCount}
              onChange={(e) => s.setChoice({ acCount: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
              aria-label="Worst checked in AC"
            />
          </Field>
        )}
      </Group>
      <Group title="AC solves" note="Each outage starts from the base case's AC solution. Anything left on Same as Power flow follows the Power flow tab's settings.">
        {SOLVE_OPTIONS.map((o) => {
          const v = c.solveOverrides[o.key];
          const pf = solve[o.key] ? "on" : "off";
          return (
            <Field key={o.key} label={o.label} hint={o.hint}>
              <select
                className={inputClass}
                value={v === undefined ? "same" : v ? "on" : "off"}
                onChange={(e) => setOverride(o.key, e.target.value)}
                data-testid={`solve-${o.key}`}
              >
                <option value="same">Same as Power flow ({pf})</option>
                <option value="on">On</option>
                <option value="off">Off</option>
              </select>
            </Field>
          );
        })}
      </Group>
    </>
  );
}
