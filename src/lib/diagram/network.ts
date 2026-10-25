// The open case's network, fetched once per case.
import { api } from "../analysis/api";
import type { CaseNetwork } from "../analysis/types";
import { useWorkspace } from "../../store/workspaceStore";

let cache: { version: number; net: Promise<CaseNetwork> } | null = null;

export function caseNetwork(): Promise<CaseNetwork> {
  const version = useWorkspace.getState().caseVersion;
  if (!cache || cache.version !== version) {
    const net = api.diagramNetwork();
    cache = { version, net };
    // A failed fetch isn't kept, so the next call tries again.
    net.catch(() => {
      if (cache?.net === net) cache = null;
    });
  }
  return cache.net;
}

/** True when the drawing was generated from the case that is open now. */
export function diagramLinked(caseName: string | undefined): boolean {
  return !!caseName && caseName === useWorkspace.getState().caseSummary?.name;
}
