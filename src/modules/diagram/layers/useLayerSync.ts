import { useEffect, useMemo } from "react";
import { useShownReport, useWorkspace } from "../../../store/workspaceStore";
import { useLayers } from "../../../store/layerStore";
import { NO_MARKS, resolveLayers } from "../../../lib/diagram/layers";
import { isComplete, loadFieldSettings, saveFieldSettings } from "../../topology/fields/storage";
import { useSnapshot } from "../network/useSnapshot";
import { useContingencies } from "../../../store/contingencyStore";
import { useStudy } from "../../../store/studyStore";

/**
 * Keep the layers' marks in step with the case and the layer settings. The
 * topology snapshot is fetched while `want` is true (a drawing is shown);
 * field rules are kept per case file, as Topology kept them.
 */
export function useLayerSync(want: boolean) {
  const path = useWorkspace((s) => s.caseSummary?.path);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const { snapshot } = useSnapshot(want);
  const on = useLayers((s) => s.on);
  const pathQuery = useLayers((s) => s.path);
  const rules = useLayers((s) => s.rules);
  const colors = useLayers((s) => s.colors);
  const onDiagram = useContingencies((s) => s.onDiagram);
  const detail = useContingencies((s) => s.detail);
  const limitPct = useContingencies((s) => s.summary?.limit_pct ?? 100);
  // A trip shown on its own was measured with the current choice.
  const rating = useContingencies((s) => (s.open !== null && s.summary ? s.summary.rating : s.choice.rating));
  const report = useShownReport();
  const resultMode = useLayers((s) => s.resultMode);
  const compareWith = useLayers((s) => s.compareWith);
  const sensitivity = useLayers((s) => s.sensitivity);
  const saved = useStudy((s) => s.saved.find((x) => x.id === compareWith) ?? null);
  const results = useMemo(
    () => ({ mode: resultMode, report, saved: saved ? { name: saved.name, report: saved.report } : null, sensitivity }),
    [resultMode, report, saved, sensitivity]
  );
  const contingency = useMemo(
    () => (onDiagram && detail ? { detail, limitPct, rating } : null),
    [onDiagram, detail, limitPct, rating]
  );

  // A new case: restore its saved filters and colours.
  useEffect(() => {
    useLayers.getState().reset(loadFieldSettings(path));
  }, [caseVersion, path]);
  useEffect(() => {
    if (snapshot) saveFieldSettings(path, rules, colors);
  }, [snapshot, path, rules, colors]);

  const complete = useMemo(() => rules.filter(isComplete), [rules]);
  const resolved = useMemo(
    () => (snapshot ? resolveLayers(snapshot, { on, path: pathQuery?.result ?? null, rules: complete, colors, contingency, results }) : null),
    [snapshot, on, pathQuery, complete, colors, contingency, results]
  );
  useEffect(() => {
    if (resolved) useLayers.getState().setResolved(resolved.marks, resolved.legends);
    else useLayers.getState().setResolved(NO_MARKS, []);
  }, [resolved]);
  return snapshot;
}
