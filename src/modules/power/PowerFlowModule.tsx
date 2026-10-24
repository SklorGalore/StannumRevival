import { useEffect, useMemo, useState } from "react";
import { Download, FolderOpen, Play, Sigma, Zap } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, DEFAULT_SOLVE_SETTINGS, pickSavePath, saveTextExport } from "../../lib/analysis/api";
import type {
  BranchRow,
  BusRow,
  GeneratorRow,
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

type Tab = "overview" | "buses" | "generators" | "branches" | "sensitivities" | "warnings";

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

function Overview({ report }: { report: SolveReport }) {
  const l = report.limits;
  const rows: [string, number, string, string][] = [
    [
      "Voltage limits",
      l.voltage_violations,
      l.worst_voltage ? `${fixed(l.worst_voltage[1], 4)} p.u. outside` : "—",
      l.worst_voltage ? `Bus ${l.worst_voltage[0]}` : "—",
    ],
    [
      "Generator Q limits",
      l.reactive_violations,
      l.worst_reactive ? `${fixed(l.worst_reactive[1])} MVAr outside` : "—",
      l.worst_reactive ? `Bus ${l.worst_reactive[0]}` : "—",
    ],
    [
      "Branch rating A",
      l.branch_overloads,
      l.worst_branch ? `${fixed(l.worst_branch[2], 1)}% loaded` : "—",
      l.worst_branch ? `${l.worst_branch[0]} → ${l.worst_branch[1]}` : "—",
    ],
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
          detail={`${report.q_limit_passes} Q-limit pass${report.q_limit_passes === 1 ? "" : "es"}`}
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
            {rows.map(([check, n, worst, where]) => (
              <tr key={check} className="border-t border-slate-100">
                <td className="px-3 py-2">{check}</td>
                <td className={`px-3 py-2 text-right tabular-nums font-semibold ${n ? "text-amber-700" : "text-emerald-700"}`}>
                  {n ? count(n) : "None"}
                </td>
                <td className="px-3 py-2 tabular-nums">{worst}</td>
                <td className="px-3 py-2">{where}</td>
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
}: {
  rows: T[];
  columns: Column<T>[];
  rowKey: (r: T) => number;
  placeholder: string;
  exportName: string;
  rowClassName?: (r: T) => string;
}) {
  const [search, setSearch] = useState("");
  async function exportCsv() {
    const filtered = filterRows(rows, columns, search);
    const csv = toCsv(
      filtered.map((r) => Object.fromEntries(columns.map((c) => [c.key, c.value(r)])))
    );
    await saveTextExport(exportName, csv, { name: "CSV", extensions: ["csv"] });
  }
  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-slate-200 bg-white">
        <SearchBox value={search} onChange={setSearch} placeholder={placeholder} />
        <Button variant="ghost" className="ml-auto" onClick={exportCsv}>
          <Download size={13} /> Export CSV
        </Button>
      </div>
      <div className="flex-1 min-h-0 bg-white">
        <DataTable rows={rows} columns={columns} rowKey={rowKey} search={search} rowClassName={rowClassName} />
      </div>
    </div>
  );
}

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

  // A new case invalidates every result.
  useEffect(() => {
    setSensitivity(null);
    setError(null);
    setTab("overview");
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
              {tab === "overview" && report && <Overview report={report} />}
              {tab === "buses" && report && (
                <TableTab
                  rows={report.buses}
                  columns={busColumns}
                  rowKey={(r) => r.id}
                  placeholder="Search buses…"
                  exportName="buses.csv"
                  rowClassName={(r) => (r.violation > 0 ? "bg-amber-50/60" : "")}
                />
              )}
              {tab === "generators" && report && (
                <TableTab
                  rows={report.generators}
                  columns={generatorColumns}
                  rowKey={(r) => r.index}
                  placeholder="Search generators…"
                  exportName="generators.csv"
                  rowClassName={(r) => (r.online ? (r.q_violation > 0 ? "bg-amber-50/60" : "") : "text-slate-400")}
                />
              )}
              {tab === "branches" && report && (
                <TableTab
                  rows={report.branches}
                  columns={branchColumns}
                  rowKey={(r) => r.index}
                  placeholder="Search branches…"
                  exportName="branches.csv"
                  rowClassName={(r) => (r.online ? ((r.loading_pct ?? 0) > 100 ? "bg-red-50/60" : "") : "text-slate-400")}
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
