import { useEffect, useState } from "react";
import { invokeCore, toStannumError } from "../../lib/analysis/api";
import type { FileVoltageComparison } from "../../lib/analysis/commands.gen";
import type { SolveReport } from "../../lib/analysis/types";
import { count, fixed } from "../../lib/analysis/format";
import { busElement } from "../../lib/elements";
import { ElementLink } from "../../ui/DrawAround";
import { Button, Panel } from "../../ui/primitives";

/**
 * The latest solve against the voltages saved in the case file: the
 * solution of whatever tool saved it last, such as PSS/E or PowerWorld.
 */
export function FileVoltages({
  report,
  onBus,
}: {
  report: SolveReport;
  onBus: (element: string) => void;
}) {
  const [result, setResult] = useState<FileVoltageComparison | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // A new solve makes the comparison stale.
  useEffect(() => {
    setResult(null);
    setError(null);
  }, [report]);
  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      setResult(await invokeCore("compare_file_voltages", { limit: 20 }));
    } catch (e) {
      setError(toStannumError(e).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Panel
      title="Against the voltages in the file"
      eyebrow="Check"
      actions={
        <Button type="button" onClick={run} disabled={busy || !report.converged} data-testid="compare-file-voltages">
          {busy ? "Comparing…" : result ? "Compare again" : "Compare with the file"}
        </Button>
      }
    >
      {!result && !error && (
        <p className="px-3 py-2 text-xs text-slate-600">
          A case saved solved by PSS/E, PowerWorld or another tool holds that tool's voltage at every bus. Compare it with
          this solve to see how closely the two agree on your own case.
        </p>
      )}
      {error && <p className="px-3 py-2 text-xs text-amber-800">{error}</p>}
      {result && (
        <div data-testid="file-voltages">
          <p className="px-3 py-2 text-xs text-slate-700">
            {count(result.compared)} buses compared: mean difference {fixed(result.mean_dvm, 4)} p.u., largest{" "}
            {fixed(result.max_dvm, 4)} p.u.{result.max_dvm_bus !== null ? ` at bus ${result.max_dvm_bus}` : ""}, angles
            within {fixed(result.max_dva, 2)}°. {count(result.within_0_001)} within 0.001 p.u. and{" "}
            {count(result.within_0_01)} within 0.01 p.u.
            {result.reference_bus !== null ? ` Angles lined up at reference bus ${result.reference_bus}.` : ""}
            {result.with_study_edits ? " The solve includes study edits, so it is not the file's own case." : ""}
          </p>
          <table className="w-full text-xs tabular-nums">
            <thead>
              <tr className="text-left text-slate-500 border-t border-slate-100">
                <th className="px-3 py-1.5 font-medium">Bus</th>
                <th className="px-3 py-1.5 font-medium">Name</th>
                <th className="px-3 py-1.5 font-medium text-right">kV</th>
                <th className="px-3 py-1.5 font-medium text-right">File (p.u.)</th>
                <th className="px-3 py-1.5 font-medium text-right">Solved (p.u.)</th>
                <th className="px-3 py-1.5 font-medium text-right">Difference (p.u.)</th>
                <th className="px-3 py-1.5 font-medium text-right">Angle difference (°)</th>
              </tr>
            </thead>
            <tbody>
              {result.rows.map((r) => (
                <tr key={r.bus} className="border-t border-slate-100">
                  <td className="px-3 py-1.5">
                    <ElementLink element={busElement(r.bus)} from="power" onOpen={() => onBus(busElement(r.bus))}>
                      {r.bus}
                    </ElementLink>
                  </td>
                  <td className="px-3 py-1.5">{r.name}</td>
                  <td className="px-3 py-1.5 text-right">{fixed(r.base_kv, 1)}</td>
                  <td className="px-3 py-1.5 text-right">{fixed(r.file_vm, 4)}</td>
                  <td className="px-3 py-1.5 text-right">{fixed(r.solved_vm, 4)}</td>
                  <td className={`px-3 py-1.5 text-right ${Math.abs(r.dvm) > 0.01 ? "text-amber-700 font-semibold" : ""}`}>
                    {r.dvm >= 0 ? "+" : ""}
                    {fixed(r.dvm, 4)}
                  </td>
                  <td className="px-3 py-1.5 text-right">
                    {r.dva >= 0 ? "+" : ""}
                    {fixed(r.dva, 2)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="px-3 py-2 text-[11px] text-slate-500">The 20 largest differences. Differences come from different models and settings as well as from the solvers; see What isn't modelled.</p>
        </div>
      )}
    </Panel>
  );
}
