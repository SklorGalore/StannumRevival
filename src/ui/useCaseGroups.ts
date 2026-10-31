import { useEffect, useMemo, useState } from "react";
import { caseNetwork } from "../lib/diagram/network";
import { useWorkspace } from "../store/workspaceStore";
import type { CaseNetwork } from "../lib/analysis/commands.gen";

export type CaseGroups = Record<"area" | "zone" | "owner", [number, number][]>;

/** Buses of each area, zone and owner, from the case: [number, bus count], sorted. */
export function useCaseGroups(): CaseGroups | null {
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const [net, setNet] = useState<CaseNetwork | null>(null);
  useEffect(() => {
    let live = true;
    caseNetwork()
      .then((n) => live && setNet(n))
      .catch(() => live && setNet(null));
    return () => {
      live = false;
    };
  }, [caseVersion]);
  return useMemo(() => {
    if (!net) return null;
    const out = { area: [], zone: [], owner: [] } as CaseGroups;
    for (const g of ["area", "zone", "owner"] as const) {
      const counts = new Map<number, number>();
      for (const b of net.buses) if (!b.star) counts.set(b[g], (counts.get(b[g]) ?? 0) + 1);
      out[g] = [...counts].sort((a, b) => a[0] - b[0]);
    }
    // MATPOWER cases have no owners: owner 0 on every bus.
    if (!out.owner.some(([o]) => o > 0)) out.owner = [];
    return out;
  }, [net]);
}
