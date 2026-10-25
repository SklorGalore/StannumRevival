import { useEffect, useMemo, useState } from "react";
import { Download, FolderOpen, Play, Sigma, Zap } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, DEFAULT_SOLVE_SETTINGS, pickSavePath, saveTextExport } from "../../lib/analysis/api";
import type {
  DcLineKind,
  DcLineRow,
  BranchRow,
  BusRow,
  GeneratorRow,
  LccDetail,
  SolveReport,
  SolveSettings,
} from "../../lib/analysis/types";
import { count, fixed, sig, toCsv } from "../../lib/analysis/format";
import {
  Button,
  EmptyState,
  ErrorBanner,
  Field,
  Metric,
  Notice,
  Panel,
  SearchBox,
  Tabs,
  inputClass,
} from "../../ui/primitives";
import { DataTable, filterRows, type Column } from "../../ui/DataTable";
import { SensitivityView } from "./SensitivityView";
import { busElement, elementBus, elementLabel, parseElement } from "../../lib/elements";
import { DrawAroundButton, ElementLink } from "../../ui/DrawAround";

type Tab = "overview" | "buses" | "generators" | "branches" | "dc" | "sensitivities" | "warnings";

const DEFAULT_SETTINGS = DEFAULT_SOLVE_SETTINGS;

const busColumns: Column<BusRow>[] = [
  { key: "id", header: "Bus", value: (r) => r.id, align: "right" },
  { key: "name", header: "Name", value: (r) => r.name },
  { key: "kind", header: "Type", value: (r) => r.kind },
  { key: "base_kv", header: "Base kV", value: (r) => r.base_kv, render: (r) => fixed(r.base_kv, 1), align: "right" },
  { key: "vm", header: "Vm (p.u.)", value: (r) => r.vm, render: (r) => fixed(r.vm, 4), align: "right" },
  { key: "va", header: "Va (°)", value: (r) => r.va, render: (r) => fixed(r.va, 3), align: "right" },
  { key: "vmin", header: "Vmin", value: (r) => r.vmin, render: (r) => fixed(r.vmin, 3), align: "right" },
  { key: "vmax", header: "Vmax", value: (r) => r.vmax, render: (r) => fixed(r.vmax, 3), align: "right" },
  { key: "pd", header: "Pd (MW)", value: (r) => r.pd, render: (r) => fixed(r.pd), align: "right" },
  { key: "qd", header: "Qd (MVAr)", value: (r) => r.qd, render: (r) => fixed(r.qd), align: "right" },
  { key: "area", header: "Area", value: (r) => r.area, align: "right" },
  {
    key: "violation",
    header: "Outside limits",
    title: "Distance outside [Vmin, Vmax] in p.u.",
    value: (r) => r.violation,
    render: (r) => (r.violation > 0 ? <span className="text-amber-700 font-medium">{fixed(r.violation, 4)}</span> : ""),
    align: "right",
  },
];

const generatorColumns: Column<GeneratorRow>[] = [
  { key: "index", header: "#", value: (r) => r.index + 1, align: "right" },
  { key: "bus", header: "Bus", value: (r) => r.bus, align: "right" },
  { key: "bus_name", header: "Bus name", value: (r) => r.bus_name },
  { key: "machine_id", header: "ID", value: (r) => r.machine_id },
  { key: "online", header: "Status", value: (r) => (r.online ? "Online" : "Offline") },
  { key: "pg", header: "Pg (MW)", value: (r) => r.pg, render: (r) => fixed(r.pg), align: "right" },
  { key: "qg", header: "Qg (MVAr)", value: (r) => r.qg, render: (r) => fixed(r.qg), align: "right" },
  { key: "qmin", header: "Qmin", value: (r) => r.qmin, render: (r) => fixed(r.qmin), align: "right" },
  { key: "qmax", header: "Qmax", value: (r) => r.qmax, render: (r) => fixed(r.qmax), align: "right" },
  { key: "vg", header: "Vg (p.u.)", value: (r) => r.vg, render: (r) => fixed(r.vg, 4), align: "right" },
  { key: "pmax", header: "Pmax", value: (r) => r.pmax, render: (r) => fixed(r.pmax), align: "right" },
  {
    key: "q_violation",
    header: "Q outside limits",
    value: (r) => r.q_violation,
    render: (r) => (r.q_violation > 0 ? <span className="text-amber-700 font-medium">{fixed(r.q_violation)}</span> : ""),
    align: "right",
  },
];

const branchColumns: Column<BranchRow>[] = [
  { key: "index", header: "#", value: (r) => r.index + 1, align: "right" },
  { key: "from", header: "From", value: (r) => r.from, align: "right" },
  { key: "to", header: "To", value: (r) => r.to, align: "right" },
  {
    key: "circuit",
    header: "Circuit",
    value: (r) => r.circuit,
    render: (r) => (r.winding ? `${r.circuit} · W${r.winding}` : r.circuit),
  },
  { key: "type", header: "Type", value: (r) => (r.is_transformer ? "Transformer" : "Line") },
  { key: "online", header: "Status", value: (r) => (r.online ? "Online" : "Offline") },
  { key: "pf", header: "PF (MW)", value: (r) => r.pf, render: (r) => fixed(r.pf), align: "right" },
  { key: "qf", header: "QF (MVAr)", value: (r) => r.qf, render: (r) => fixed(r.qf), align: "right" },
  { key: "pt", header: "PT (MW)", value: (r) => r.pt, render: (r) => fixed(r.pt), align: "right" },
  { key: "qt", header: "QT (MVAr)", value: (r) => r.qt, render: (r) => fixed(r.qt), align: "right" },
  { key: "loss", header: "Loss (MW)", value: (r) => r.loss_mw, render: (r) => fixed(r.loss_mw, 3), align: "right" },
  { key: "rate_a", header: "Rate A (MVA)", value: (r) => r.rate_a, render: (r) => (r.rate_a > 0 ? fixed(r.rate_a, 1) : "—"), align: "right" },
  {
    key: "loading",
    header: "Loading (%)",
    value: (r) => r.loading_pct,
    render: (r) =>
      r.loading_pct === null ? (
        "—"
      ) : (
        <span className={r.loading_pct > 100 ? "text-red-700 font-semibold" : r.loading_pct > 90 ? "text-amber-700" : ""}>
          {fixed(r.loading_pct, 1)}
        </span>
      ),
    align: "right",
  },
];

const DC_KIND_LABEL: Record<DcLineKind, string> = {
  two_terminal: "Two-terminal",
  vsc: "VSC",
  multi_terminal: "Multi-terminal",
};

const qRange = (lo: number, hi: number) => (lo === hi ? fixed(lo) : `${fixed(lo)} to ${fixed(hi)}`);

const LCC_CONTROL_LABEL: Record<LccDetail["control"], string> = {
  rectifier: "Rectifier",
  inverter: "Inverter (margin)",
  angle_limited: "Angle limited",
  blocked: "Blocked",
};

/** A detailed-model value, blank for rows solved with the simplified model. */
function detail(r: DcLineRow, show: (d: LccDetail) => string): string {
  return r.detail ? show(r.detail) : "";
}

function num(value: number | null, digits: number): string {
  return value === null ? "" : fixed(value, digits);
}

function dcCurrent(r: DcLineRow): number | null {
  return r.detail?.current_ka ?? r.vsc_detail?.current_ka ?? null;
}

/** DC voltage at the sending (0) or receiving (1) converter. */
function dcVoltage(r: DcLineRow, end: 0 | 1): number | null {
  if (r.detail) return end === 0 ? r.detail.rectifier_kv : r.detail.inverter_kv;
  if (r.vsc_detail) return end === 0 ? r.vsc_detail.from_kv : r.vsc_detail.to_kv;
  return null;
}

function vscControl(r: DcLineRow): string {
  const d = r.vsc_detail;
  if (!d) return "";
  const ends = [d.from_limited && "sending", d.to_limited && "receiving"].filter(Boolean);
  if (!ends.length) return "At order";
  return `Limited at ${ends.join(" and ")} end (${fixed(d.order_scale * 100, 1)}% of order)`;
}

const dcLineColumns: Column<DcLineRow>[] = [
  { key: "index", header: "#", value: (r) => r.index + 1, align: "right" },
  { key: "name", header: "Name", value: (r) => parseElement(r.element)?.circuit ?? "" },
  { key: "kind", header: "Kind", value: (r) => DC_KIND_LABEL[r.kind] ?? r.kind },
  { key: "from", header: "From", value: (r) => r.from, align: "right" },
  { key: "from_name", header: "From name", value: (r) => r.from_name },
  { key: "to", header: "To", value: (r) => r.to, align: "right" },
  { key: "to_name", header: "To name", value: (r) => r.to_name },
  { key: "online", header: "Status", value: (r) => (r.online ? "Online" : "Offline") },
  {
    key: "control",
    header: "Control",
    title: "Detailed model: which converter holds the DC current (LCC), or whether converter limits cut the output (VSC)",
    value: (r) => (r.detail ? LCC_CONTROL_LABEL[r.detail.control] + (r.detail.reduced_voltage ? ", low voltage" : "") : vscControl(r)),
  },
  { key: "id", header: "Id (kA)", value: (r) => dcCurrent(r), render: (r) => num(dcCurrent(r), 4), align: "right" },
  { key: "vdr", header: "Vd send (kV)", title: "DC voltage at the sending converter (LCC rectifier)", value: (r) => dcVoltage(r, 0), render: (r) => num(dcVoltage(r, 0), 1), align: "right" },
  { key: "vdi", header: "Vd recv (kV)", title: "DC voltage at the receiving converter (LCC inverter)", value: (r) => dcVoltage(r, 1), render: (r) => num(dcVoltage(r, 1), 1), align: "right" },
  { key: "alpha", header: "α (°)", title: "Rectifier firing angle", value: (r) => r.detail?.alpha ?? null, render: (r) => detail(r, (d) => fixed(d.alpha, 2)), align: "right" },
  { key: "gamma", header: "γ (°)", title: "Inverter extinction angle", value: (r) => r.detail?.gamma ?? null, render: (r) => detail(r, (d) => fixed(d.gamma, 2)), align: "right" },
  { key: "tap_r", header: "Tap R", title: "Rectifier converter transformer tap", value: (r) => r.detail?.rectifier_tap ?? null, render: (r) => detail(r, (d) => fixed(d.rectifier_tap, 5)), align: "right" },
  { key: "tap_i", header: "Tap I", title: "Inverter converter transformer tap", value: (r) => r.detail?.inverter_tap ?? null, render: (r) => detail(r, (d) => fixed(d.inverter_tap, 5)), align: "right" },
  { key: "pf", header: "P sent (MW)", value: (r) => r.pf, render: (r) => fixed(r.pf), align: "right" },
  { key: "pt", header: "P received (MW)", value: (r) => r.pt, render: (r) => fixed(r.pt), align: "right" },
  { key: "loss", header: "Loss (MW)", value: (r) => r.loss_mw, render: (r) => fixed(r.loss_mw, 3), align: "right" },
  {
    key: "qf",
    header: "QF (MVAr)",
    title: "Reactive power injected at the sending end; negative means absorbed",
    value: (r) => r.qf,
    render: (r) => fixed(r.qf),
    align: "right",
  },
  { key: "qf_range", header: "QF range", title: "Not used by the detailed model", value: (r) => r.qminf, render: (r) => (r.detail ? "—" : qRange(r.qminf, r.qmaxf)), align: "right" },
  {
    key: "qt",
    header: "QT (MVAr)",
    title: "Reactive power injected at the receiving end; negative means absorbed",
    value: (r) => r.qt,
    render: (r) => fixed(r.qt),
    align: "right",
  },
  { key: "qt_range", header: "QT range", title: "Not used by the detailed model", value: (r) => r.qmint, render: (r) => (r.detail ? "—" : qRange(r.qmint, r.qmaxt)), align: "right" },
  { key: "vf", header: "VF (p.u.)", value: (r) => r.vf, render: (r) => fixed(r.vf, 4), align: "right" },
  { key: "vt", header: "VT (p.u.)", value: (r) => r.vt, render: (r) => fixed(r.vt, 4), align: "right" },
  {
    key: "q_violation",
    header: "Q outside range",
    value: (r) => r.q_violation,
    render: (r) => (r.q_violation > 0 ? <span className="text-amber-700 font-medium">{fixed(r.q_violation)}</span> : ""),
    align: "right",
  },
];

function SolvePanel({
  settings,
  setSettings,
  busy,
  onSolve,
  onSensitivities,
  report,
  onExport,
}: {
  settings: SolveSettings;
  setSettings: (s: SolveSettings) => void;
  busy: string | null;
  onSolve: () => void;
  onSensitivities: () => void;
  report: SolveReport | null;
  onExport: (solved: boolean) => void;
}) {
  const ac = settings.model === "ac";
  return (
    <aside className="w-60 shrink-0 border-r border-slate-200 bg-white p-3 flex flex-col gap-3 overflow-y-auto">
      <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">Solve settings</p>
      <Field label="Model">
        <div className="grid grid-cols-2 rounded-md border border-slate-300 overflow-hidden text-xs">
          {(["ac", "dc"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setSettings({ ...settings, model: m })}
              aria-pressed={settings.model === m}
              className={`py-1.5 font-medium ${settings.model === m ? "bg-teal-700 text-white" : "bg-white text-slate-600 hover:bg-slate-50"}`}
            >
              {m.toUpperCase()}
            </button>
          ))}
        </div>
      </Field>
      <Field label="Starting voltages">
        <select
          className={inputClass}
          value={settings.start}
          disabled={!ac}
          onChange={(e) => setSettings({ ...settings, start: e.target.value as SolveSettings["start"] })}
        >
          <option value="case">From case</option>
          <option value="flat">Flat start (1 p.u.)</option>
        </select>
      </Field>
      <Field label="Tolerance (p.u.)">
        <input
          className={inputClass}
          type="number"
          step="any"
          min={0}
          value={settings.tolerance}
          disabled={!ac}
          onChange={(e) => setSettings({ ...settings, tolerance: Number(e.target.value) })}
        />
      </Field>
      <Field label="Newton iterations per pass">
        <input
          className={inputClass}
          type="number"
          min={0}
          step={1}
          value={settings.max_iterations}
          disabled={!ac}
          onChange={(e) => setSettings({ ...settings, max_iterations: Math.max(0, Math.round(Number(e.target.value))) })}
        />
      </Field>
      <label className="flex items-center gap-2 text-xs text-slate-700">
        <input
          type="checkbox"
          checked={settings.enforce_q_limits && ac}
          disabled={!ac}
          onChange={(e) => setSettings({ ...settings, enforce_q_limits: e.target.checked })}
        />
        Enforce generator Q limits
      </label>
      <Field label="Two-terminal and VSC DC lines">
        <select
          className={inputClass}
          value={settings.dc_model}
          disabled={!ac}
          onChange={(e) => setSettings({ ...settings, dc_model: e.target.value as SolveSettings["dc_model"] })}
        >
          <option value="detailed">Detailed converters</option>
          <option value="simplified">Simplified (MATPOWER)</option>
        </select>
      </Field>
      <label className="flex items-center gap-2 text-xs text-slate-700">
        <input
          type="checkbox"
          checked={settings.adjust_dc_taps && ac && settings.dc_model === "detailed"}
          disabled={!ac || settings.dc_model !== "detailed"}
          onChange={(e) => setSettings({ ...settings, adjust_dc_taps: e.target.checked })}
        />
        Adjust DC converter taps
      </label>
      <Button variant="primary" className="py-2 text-sm" onClick={onSolve} disabled={!!busy}>
        <Play size={14} /> {busy === "solve" ? "Solving…" : "Solve"}
      </Button>
      <Button onClick={onSensitivities} disabled={!!busy}>
        <Sigma size={14} /> {busy === "sens" ? "Calculating…" : "Calculate sensitivities"}
      </Button>
      <div className="mt-2 pt-3 border-t border-slate-200 flex flex-col gap-1.5">
        <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">Export MATPOWER</p>
        <Button variant="ghost" className="justify-start" onClick={() => onExport(false)} disabled={!!busy}>
          <Download size={13} /> Input case…
        </Button>
        <Button variant="ghost" className="justify-start" onClick={() => onExport(true)} disabled={!!busy || !report}>
          <Download size={13} /> Solved case…
        </Button>
      </div>
    </aside>
  );
}

/** Where a click on the overview leads: a table, optionally narrowed to its problems. */
type Jump = { tab: "buses" | "generators" | "branches"; problems: boolean; element?: string };

function Overview({ report, onJump }: { report: SolveReport; onJump: (j: Jump) => void }) {
  const l = report.limits;
  // The worst generator and branch are reported by bus numbers; find their rows.
  const worstGenerator = l.worst_reactive
    ? report.generators
        .filter((g) => g.bus === l.worst_reactive![0])
        .reduce<GeneratorRow | null>((a, g) => (!a || g.q_violation > a.q_violation ? g : a), null)
    : null;
  const worstBranch = l.worst_branch
    ? report.branches
        .filter(
          (b) =>
            (b.from === l.worst_branch![0] && b.to === l.worst_branch![1]) ||
            (b.from === l.worst_branch![1] && b.to === l.worst_branch![0])
        )
        .reduce<BranchRow | null>((a, b) => (!a || (b.loading_pct ?? 0) > (a.loading_pct ?? 0) ? b : a), null)
    : null;
  const rows: { check: string; n: number; worst: string; where: string | null; element: string | null; tab: Jump["tab"] }[] = [
    {
      check: "Voltage limits",
      n: l.voltage_violations,
      worst: l.worst_voltage ? `${fixed(l.worst_voltage[1], 4)} p.u. outside` : "—",
      where: l.worst_voltage ? `Bus ${l.worst_voltage[0]}` : null,
      element: l.worst_voltage ? busElement(l.worst_voltage[0]) : null,
      tab: "buses",
    },
    {
      check: "Generator Q limits",
      n: l.reactive_violations,
      worst: l.worst_reactive ? `${fixed(l.worst_reactive[1])} MVAr outside` : "—",
      where: l.worst_reactive ? `Bus ${l.worst_reactive[0]}` : null,
      element: worstGenerator?.element ?? (l.worst_reactive ? busElement(l.worst_reactive[0]) : null),
      tab: "generators",
    },
    {
      check: "Branch rating A",
      n: l.branch_overloads,
      worst: l.worst_branch ? `${fixed(l.worst_branch[2], 1)}% loaded` : "—",
      where: l.worst_branch ? `${l.worst_branch[0]} → ${l.worst_branch[1]}` : null,
      element: worstBranch?.element ?? null,
      tab: "branches",
    },
  ];
  return (
    <div className="p-4 flex flex-col gap-4 overflow-y-auto h-full">
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <Metric
          label="Status"
          value={report.status}
          detail={`${report.model.toUpperCase()} solve`}
          tone={report.converged ? "good" : "bad"}
        />
        <Metric
          label="Iterations"
          value={report.iterations}
          detail={
            `${report.q_limit_passes} Q-limit pass${report.q_limit_passes === 1 ? "" : "es"}` +
            (report.dc_tap_passes ? `, ${report.dc_tap_passes} DC tap pass${report.dc_tap_passes === 1 ? "" : "es"}` : "") +
            (report.vsc_limit_passes ? `, ${report.vsc_limit_passes} VSC limit pass${report.vsc_limit_passes === 1 ? "" : "es"}` : "")
          }
        />
        <Metric label="Final residual" value={`${sig(report.residual_mva)} MVA`} detail={`${sig(report.residual_pu)} p.u.`} />
        <Metric label="Solve time" value={`${fixed(report.elapsed_ms, 1)} ms`} />
        <Metric label="Generation" value={`${fixed(report.total_generation_mw, 1)} MW`} detail="Online units" />
        <Metric label="Load" value={`${fixed(report.total_load_mw, 1)} MW`} />
        <Metric label="Branch losses" value={`${fixed(report.total_losses_mw, 2)} MW`} />
      </div>
      {!report.converged && (
        <Notice>
          The solver did not converge. The tables show the final iterate so you can inspect it; treat these values with care.
        </Notice>
      )}
      <Panel title="Operating limits" eyebrow="Checks">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-slate-500">
              <th className="px-3 py-2 font-medium">Check</th>
              <th className="px-3 py-2 font-medium text-right">Violations</th>
              <th className="px-3 py-2 font-medium">Worst</th>
              <th className="px-3 py-2 font-medium">Location</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.check}
                className={`border-t border-slate-100 ${r.n ? "cursor-pointer hover:bg-slate-50" : ""}`}
                onClick={r.n ? () => onJump({ tab: r.tab, problems: true }) : undefined}
                title={r.n ? `List the ${r.n} violation${r.n === 1 ? "" : "s"}, worst first` : undefined}
                data-testid={`check-${r.tab}`}
              >
                <td className="px-3 py-2">{r.check}</td>
                <td className={`px-3 py-2 text-right tabular-nums font-semibold ${r.n ? "text-amber-700" : "text-emerald-700"}`}>
                  {r.n ? (
                    <button
                      type="button"
                      className="underline decoration-amber-300 underline-offset-2 hover:decoration-amber-700"
                      onClick={(e) => {
                        e.stopPropagation();
                        onJump({ tab: r.tab, problems: true });
                      }}
                    >
                      {count(r.n)}
                    </button>
                  ) : (
                    "None"
                  )}
                </td>
                <td className="px-3 py-2 tabular-nums">{r.worst}</td>
                <td className="px-3 py-2">
                  {r.element ? (
                    <ElementLink element={r.element} from="power" onOpen={() => onJump({ tab: r.tab, problems: false, element: r.element! })}>
                      {r.where}
                    </ElementLink>
                  ) : (
                    (r.where ?? "—")
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Panel>
    </div>
  );
}

function TableTab<T>({
  rows,
  columns,
  rowKey,
  placeholder,
  exportName,
  rowClassName,
  element,
  matches,
  selected,
  onPick,
  problem,
  initialProblemsOnly = false,
}: {
  rows: T[];
  columns: Column<T>[];
  rowKey: (r: T) => number;
  placeholder: string;
  exportName: string;
  rowClassName?: (r: T) => string;
  /** The shared id a row selects. */
  element: (r: T) => string;
  /** Whether a row shows the selection; defaults to an exact id match. */
  matches?: (r: T, selected: string) => boolean;
  /** The element picked here or, while linked, anywhere. */
  selected: string | null;
  onPick: (element: string) => void;
  /** Rows outside their limits, with the column that measures how far, worst first. */
  problem?: { test: (r: T) => boolean; sortKey: string; label: string };
  initialProblemsOnly?: boolean;
}) {
  const [search, setSearch] = useState("");
  const [problemsOnly, setProblemsOnly] = useState(initialProblemsOnly);
  const problemCount = useMemo(() => (problem ? rows.filter(problem.test).length : 0), [rows, problem]);
  const shown = useMemo(
    () => (problem && problemsOnly ? rows.filter(problem.test) : rows),
    [rows, problem, problemsOnly]
  );
  const isSelected = (r: T) => selected !== null && (matches ? matches(r, selected) : element(r) === selected);
  const selectedRow = selected === null ? undefined : rows.find(isSelected);
  async function exportCsv() {
    const filtered = filterRows(shown, columns, search);
    const csv = toCsv(
      filtered.map((r) => Object.fromEntries(columns.map((c) => [c.key, c.value(r)])))
    );
    await saveTextExport(exportName, csv, { name: "CSV", extensions: ["csv"] });
  }
  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center gap-3 px-3 py-2 border-b border-slate-200 bg-white">
        <SearchBox value={search} onChange={setSearch} placeholder={placeholder} />
        {problem && (
          <label className={`flex items-center gap-1.5 text-xs ${problemCount ? "text-slate-700" : "text-slate-400"}`}>
            <input
              type="checkbox"
              checked={problemsOnly}
              disabled={!problemCount && !problemsOnly}
              onChange={(e) => setProblemsOnly(e.target.checked)}
            />
            {problem.label} only ({count(problemCount)})
          </label>
        )}
        <div className="ml-auto flex items-center gap-2 min-w-0">
          {selectedRow && selected && (
            <span className="flex items-center gap-1 min-w-0 text-xs text-slate-600" data-testid="table-selection">
              <span className="truncate font-medium text-slate-800">{elementLabel(selected)}</span>
              <DrawAroundButton element={element(selectedRow)} from="power" />
            </span>
          )}
          <Button variant="ghost" onClick={exportCsv}>
            <Download size={13} /> Export CSV
          </Button>
        </div>
      </div>
      <div className="flex-1 min-h-0 bg-white">
        <DataTable
          rows={shown}
          columns={columns}
          rowKey={rowKey}
          search={search}
          rowClassName={rowClassName}
          isSelected={isSelected}
          onRowClick={(r) => onPick(element(r))}
          initialSort={problem && initialProblemsOnly ? { key: problem.sortKey, dir: "desc" } : null}
        />
      </div>
    </div>
  );
}

const BUS_PROBLEM = { test: (r: BusRow) => r.violation > 0, sortKey: "violation", label: "Voltage violations" };
const GENERATOR_PROBLEM = {
  test: (r: GeneratorRow) => r.online && r.q_violation > 0,
  sortKey: "q_violation",
  label: "Q limit violations",
};
const BRANCH_PROBLEM = {
  test: (r: BranchRow) => r.online && (r.loading_pct ?? 0) > 100,
  sortKey: "loading",
  label: "Overloads",
};

export function PowerFlowModule({ active }: { active: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const openCase = useWorkspace((s) => s.openCase);

  const [settings, setSettings] = useState(DEFAULT_SETTINGS);
  const [tab, setTab] = useState<Tab>("overview");
  // Shared so the diagram can overlay the same results.
  const report = useWorkspace((s) => s.solveReport);
  const setReport = useWorkspace((s) => s.setSolveReport);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sensitivityVersion, setSensitivityVersion] = useState(0);
  const [sensitivity, setSensitivity] = useState<Awaited<ReturnType<typeof api.sensitivities>> | null>(null);

  // Follow a selection made elsewhere by showing the table that lists it.
  const selection = useWorkspace((s) => s.selection);
  const linked = useWorkspace((s) => s.linked);
  const select = useWorkspace((s) => s.select);
  // Picked here; while linked, the shared selection wins.
  const [picked, setPicked] = useState<string | null>(null);
  const selected = linked ? (selection?.element ?? null) : picked;
  const pick = (element: string) => {
    setPicked(element);
    select(element, "power");
  };
  // A click on the overview: which table to open, and whether to list only problems.
  const [jump, setJump] = useState<{ n: number; problems: boolean }>({ n: 0, problems: false });
  const onJump = (j: Jump) => {
    if (j.element) pick(j.element);
    setJump((p) => ({ n: p.n + 1, problems: j.problems }));
    setTab(j.tab);
  };
  useEffect(() => {
    if (!selection || selection.from === "power" || !report) return;
    const kind = parseElement(selection.element)?.kind;
    const next: Tab | null =
      kind === "bus" || kind === "load" ? "buses" : kind === "generator" ? "generators" : kind === "line" || kind === "transformer" ? "branches" : kind === "dcline" ? "dc" : null;
    if (next) setTab(next);
  }, [selection, report]);

  // A new case invalidates every result.
  useEffect(() => {
    setSensitivity(null);
    setError(null);
    setTab("overview");
    setPicked(null);
    setJump({ n: 0, problems: false });
  }, [caseVersion]);

  const power = summary?.power;
  const warnings = power?.info?.warnings ?? [];

  async function run<T>(kind: string, work: () => Promise<T>, done: (v: T) => void) {
    const version = useWorkspace.getState().caseVersion;
    setBusy(kind);
    setError(null);
    try {
      const value = await work();
      if (version === useWorkspace.getState().caseVersion) done(value);
    } catch (e) {
      if (version === useWorkspace.getState().caseVersion) setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  const solve = () =>
    run("solve", () => api.solve(settings), (r) => {
      setReport(r);
      if (tab === "sensitivities" || tab === "warnings") setTab("overview");
    });
  const sensitivities = () =>
    run("sens", () => api.sensitivities(settings), (s) => {
      setSensitivity(s);
      setSensitivityVersion((v) => v + 1);
      setTab("sensitivities");
    });

  async function exportMatpower(solved: boolean) {
    const stem = (summary?.name ?? "case").replace(/\.[^.]+$/, "").replace(/[^A-Za-z0-9_]/g, "_");
    const name = /^[A-Za-z]/.test(stem) ? stem : `case_${stem}`;
    const path = await pickSavePath(`${name}${solved ? "_solved" : ""}.m`, [{ name: "MATPOWER", extensions: ["m"] }]);
    if (!path) return;
    await run("export", () => api.exportMatpower(path, solved), () => undefined);
  }

  const tabs = useMemo(
    () => [
      { id: "overview" as const, label: "Overview", disabled: !report },
      { id: "buses" as const, label: "Buses", count: report?.buses.length, disabled: !report },
      { id: "generators" as const, label: "Generators", count: report?.generators.length, disabled: !report },
      { id: "branches" as const, label: "Branches", count: report?.branches.length, disabled: !report },
      ...(report?.dc_lines?.length
        ? [{ id: "dc" as const, label: "DC lines", count: report.dc_lines.length, disabled: false }]
        : []),
      { id: "sensitivities" as const, label: "Sensitivities", disabled: !sensitivity },
      { id: "warnings" as const, label: "Import warnings", count: warnings.length, disabled: !warnings.length },
    ],
    [report, sensitivity, warnings.length]
  );

  if (!summary) {
    return (
      <EmptyState
        icon={<Zap size={22} />}
        title="Power flow"
        action={
          <Button variant="primary" onClick={() => openCase()}>
            <FolderOpen size={14} /> Open case…
          </Button>
        }
      >
        Open a MATPOWER (.m) or PSS/E RAW (revisions 23–35) case to run AC or DC power flow, check operating limits and
        calculate sensitivities. Powered by rs-power.
      </EmptyState>
    );
  }
  if (!power?.available) {
    return (
      <div className="p-6 max-w-2xl">
        <ErrorBanner>
          <strong>rs-power could not read this case.</strong> {power?.error}
        </ErrorBanner>
      </div>
    );
  }

  return (
    <div className="h-full flex" aria-hidden={!active}>
      <SolvePanel
        settings={settings}
        setSettings={setSettings}
        busy={busy}
        onSolve={solve}
        onSensitivities={sensitivities}
        report={report}
        onExport={exportMatpower}
      />
      <div className="flex-1 min-w-0 flex flex-col">
        {error && (
          <div className="p-3 pb-0">
            <ErrorBanner>{error}</ErrorBanner>
          </div>
        )}
        {!report && !sensitivity && tab !== "warnings" ? (
          <EmptyState icon={<Play size={20} />} title="Ready to solve">
            {count(power.info!.counts.buses)} buses, {count(power.info!.counts.generators)} generators and{" "}
            {count(power.info!.counts.branches)} branches loaded.
            {warnings.length > 0 && (
              <>
                {" "}
                The RAW import produced{" "}
                <button className="text-teal-700 underline" onClick={() => setTab("warnings")}>
                  {count(warnings.length)} warning{warnings.length === 1 ? "" : "s"}
                </button>
                .
              </>
            )}
          </EmptyState>
        ) : (
          <>
            <Tabs tabs={tabs} value={tab} onChange={setTab} />
            <div className="flex-1 min-h-0">
              {tab === "overview" && report && <Overview report={report} onJump={onJump} />}
              {tab === "buses" && report && (
                <TableTab
                  key={`buses-${jump.n}`}
                  selected={selected}
                  onPick={pick}
                  problem={BUS_PROBLEM}
                  initialProblemsOnly={jump.problems}
                  rows={report.buses}
                  columns={busColumns}
                  rowKey={(r) => r.id}
                  placeholder="Search buses…"
                  exportName="buses.csv"
                  element={(r) => busElement(r.id)}
                  matches={(r, sel) => elementBus(sel) === r.id}
                  rowClassName={(r) => (r.violation > 0 ? "bg-amber-50/60" : "")}
                />
              )}
              {tab === "generators" && report && (
                <TableTab
                  key={`generators-${jump.n}`}
                  selected={selected}
                  onPick={pick}
                  problem={GENERATOR_PROBLEM}
                  initialProblemsOnly={jump.problems}
                  rows={report.generators}
                  columns={generatorColumns}
                  rowKey={(r) => r.index}
                  placeholder="Search generators…"
                  exportName="generators.csv"
                  element={(r) => r.element}
                  rowClassName={(r) => (r.online ? (r.q_violation > 0 ? "bg-amber-50/60" : "") : "text-slate-400")}
                />
              )}
              {tab === "branches" && report && (
                <TableTab
                  key={`branches-${jump.n}`}
                  selected={selected}
                  onPick={pick}
                  problem={BRANCH_PROBLEM}
                  initialProblemsOnly={jump.problems}
                  rows={report.branches}
                  columns={branchColumns}
                  rowKey={(r) => r.index}
                  placeholder="Search branches…"
                  exportName="branches.csv"
                  element={(r) => r.element}
                  rowClassName={(r) => (r.online ? ((r.loading_pct ?? 0) > 100 ? "bg-red-50/60" : "") : "text-slate-400")}
                />
              )}
              {tab === "dc" && report?.dc_lines && (
                <TableTab
                  selected={selected}
                  onPick={pick}
                  rows={report.dc_lines}
                  columns={dcLineColumns}
                  rowKey={(r) => r.index}
                  placeholder="Search DC lines…"
                  exportName="dc-lines.csv"
                  element={(r) => r.element}
                  rowClassName={(r) => (r.online ? (r.q_violation > 0 ? "bg-amber-50/60" : "") : "text-slate-400")}
                />
              )}
              {tab === "sensitivities" && sensitivity && (
                <SensitivityView key={sensitivityVersion} summary={sensitivity} />
              )}
              {tab === "warnings" && (
                <div className="h-full bg-white">
                  <DataTable
                    rows={warnings.map((w, i) => ({ ...w, i }))}
                    rowKey={(w) => w.i}
                    columns={[
                      { key: "line", header: "Line", value: (w) => w.line, align: "right" },
                      { key: "message", header: "Warning", value: (w) => w.message },
                    ]}
                  />
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
