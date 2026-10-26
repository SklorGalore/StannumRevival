import { useEffect, useState } from "react";
import { useWorkspace } from "../../../store/workspaceStore";
import { loadTopology, useTopology } from "../../../lib/analysis/topologyStore";
import type { TopologySnapshot } from "../../../lib/analysis/types";

/** The open case's topology snapshot, loaded while `want` is true. */
export function useSnapshot(want: boolean): { snapshot: TopologySnapshot | null; error: string | null } {
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const available = useWorkspace((s) => !!s.caseSummary?.topology.available);
  const snapshot = useTopology((s) => (s.version === caseVersion ? s.snapshot : null));
  const [error, setError] = useState<{ version: number; message: string } | null>(null);
  useEffect(() => {
    if (!want || !available || snapshot) return;
    let live = true;
    loadTopology(caseVersion).catch((e: Error) => live && setError({ version: caseVersion, message: e.message }));
    return () => {
      live = false;
    };
  }, [want, available, snapshot, caseVersion]);
  return { snapshot, error: error?.version === caseVersion ? error.message : null };
}
