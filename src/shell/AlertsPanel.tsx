// The bottom panel's Alerts page: limit problems in the shown result, critical
// (past a limit) and marginal (close to one), each a link to its element.
import { useMemo, useState } from "react";
import { useWorkspace } from "../store/workspaceStore";
import { useAlerts } from "../store/alertStore";
import { alertCounts, alertsFor, type Alert, type AlertLevel } from "../lib/analysis/alerts";
import { elementLabel } from "../lib/elements";

/** Rows drawn at once; a whole case past its limits is not read row by row. */
const DRAWN = 500;

/** The alerts for the result every view shows, or why there are none to show. */
export function useShownAlerts(): { alerts: Alert[]; note: string | null } {
  const latest = useWorkspace((s) => s.solveReport);
  const shown = useWorkspace((s) => s.shown);
  const hasCase = useWorkspace((s) => !!s.caseSummary);
  const thresholds = useAlerts((s) => s.thresholds);
  const report = shown ? shown.report : latest;
  const alerts = useMemo(() => (report ? alertsFor(report, thresholds) : []), [report, thresholds]);
  const note = !hasCase
    ? "Open a case and solve it to see alerts."
    : shown && shown.error
      ? `The shown result failed to solve: ${shown.error}`
      : shown && !shown.report
        ? "Solving the shown result…"
        : !report
          ? "Solve the case to see alerts."
          : null;
  return { alerts, note };
}

const LEVEL_STYLE: Record<AlertLevel, string> = {
  critical: "bg-red-700 text-white",
  marginal: "bg-amber-200 text-amber-950",
};

export function AlertsHeader() {
  const thresholds = useAlerts((s) => s.thresholds);
  const setThresholds = useAlerts((s) => s.setThresholds);
  const input = "w-14 rounded border border-slate-300 px-1 py-0.5 text-[11px] tabular-nums";
  return (
    <div className="flex items-center gap-2 text-[11px] text-slate-600">
      <span>Marginal from</span>
      <label className="flex items-center gap-1">
        <input
          type="number"
          className={input}
          min={50}
          max={100}
          step={1}
          value={thresholds.loadingPct}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (v > 0 && v <= 100) setThresholds({ loadingPct: v });
          }}
          aria-label="Marginal branch loading, percent of rate A"
        />
        % loading
      </label>
      <label className="flex items-center gap-1">
        <input
          type="number"
          className={input}
          min={0}
          max={0.1}
          step={0.005}
          value={thresholds.voltageBandPu}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (v >= 0 && v < 0.5) setThresholds({ voltageBandPu: v });
          }}
          aria-label="Marginal voltage band, per unit inside the bus limits"
        />
        pu from a voltage limit
      </label>
    </div>
  );
}

export function AlertsPanel() {
  const { alerts, note } = useShownAlerts();
  const focus = useWorkspace((s) => s.focus);
  const [level, setLevel] = useState<AlertLevel | "all">("all");
  const counts = alertCounts(alerts);
  const listed = level === "all" ? alerts : alerts.filter((a) => a.level === level);
  const drawn = listed.slice(0, DRAWN);
  const filters: { id: AlertLevel | "all"; label: string; n: number }[] = [
    { id: "all", label: "All", n: alerts.length },
    { id: "critical", label: "Critical", n: counts.critical },
    { id: "marginal", label: "Marginal", n: counts.marginal },
  ];
  return (
    <div className="flex flex-col flex-1 min-h-0" data-testid="alerts">
      <div className="flex items-center gap-3 px-3 py-1.5 border-b border-slate-200">
        <div role="radiogroup" aria-label="Show alerts" className="flex rounded-md border border-slate-300 overflow-hidden">
          {filters.map((f) => (
            <button
              key={f.id}
              role="radio"
              aria-checked={level === f.id}
              onClick={() => setLevel(f.id)}
              className={`px-2.5 py-1 text-[11px] font-medium border-l first:border-l-0 border-slate-300 ${
                level === f.id ? "bg-teal-700 text-white" : "bg-white text-slate-600 hover:bg-slate-50"
              }`}
            >
              {f.label} <span className="tabular-nums opacity-70">{f.n}</span>
            </button>
          ))}
        </div>
        <AlertsHeader />
        <span className="ml-auto text-[11px] text-slate-500">Critical: past a limit. Click a row to show the element everywhere.</span>
      </div>
      <div className="flex-1 min-h-0 overflow-auto">
        {note ? (
          <p className="px-3 py-2 text-xs text-slate-500">{note}</p>
        ) : !listed.length ? (
          <p className="px-3 py-2 text-xs text-slate-500">
            {alerts.length ? "None at this level." : "No alerts: every bus, branch and generator is inside its limits and away from them."}
          </p>
        ) : (
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-slate-50 text-left text-slate-500">
              <tr>
                <th className="px-3 py-1 font-medium w-20">Level</th>
                <th className="px-3 py-1 font-medium">Element</th>
                <th className="px-3 py-1 font-medium">Condition</th>
                <th className="px-3 py-1 font-medium text-right">Value</th>
                <th className="px-3 py-1 font-medium text-right">Limit</th>
              </tr>
            </thead>
            <tbody>
              {drawn.map((a) => (
                <tr
                  key={`${a.kind}:${a.element}`}
                  className={`border-t border-slate-100 cursor-pointer hover:bg-slate-50 ${a.level === "critical" ? "bg-red-50/50" : ""}`}
                  onClick={() => focus(a.element)}
                  data-level={a.level}
                >
                  <td className="px-3 py-1">
                    <span className={`inline-block rounded px-1.5 text-[10.5px] font-semibold ${LEVEL_STYLE[a.level]}`}>
                      {a.level === "critical" ? "Critical" : "Marginal"}
                    </span>
                  </td>
                  <td className="px-3 py-1">
                    <button
                      type="button"
                      className="text-left hover:text-teal-800 hover:underline"
                      onClick={(e) => {
                        e.stopPropagation();
                        focus(a.element);
                      }}
                      title="Show in every module"
                    >
                      <span className="font-medium">{elementLabel(a.element)}</span>
                      {a.names && <span className="text-slate-500"> · {a.names}</span>}
                    </button>
                  </td>
                  <td className="px-3 py-1">{a.condition}</td>
                  <td className={`px-3 py-1 text-right tabular-nums ${a.level === "critical" ? "text-red-800 font-semibold" : ""}`}>{a.value}</td>
                  <td className="px-3 py-1 text-right tabular-nums text-slate-600">{a.limit}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {listed.length > DRAWN && (
          <p className="px-3 py-1 text-[11px] text-slate-500">
            Showing the worst {DRAWN.toLocaleString()} of {listed.length.toLocaleString()}. The Power flow tables list them all.
          </p>
        )}
      </div>
    </div>
  );
}
