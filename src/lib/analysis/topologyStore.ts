// The open case's topology snapshot, shared by Topology and the Diagram's
// Network style so the (possibly large) snapshot is fetched once per case.
import { create } from "zustand";
import { api } from "./api";
import type { TopologySnapshot } from "./types";

interface TopologyState {
  /** The case version the snapshot belongs to (see workspaceStore.caseVersion). */
  version: number;
  snapshot: TopologySnapshot | null;
}

export const useTopology = create<TopologyState>(() => ({ version: -1, snapshot: null }));

let pending: { version: number; promise: Promise<TopologySnapshot> } | null = null;

/** The snapshot for case version `version`, fetched once however many views ask. */
export function loadTopology(version: number): Promise<TopologySnapshot> {
  const s = useTopology.getState();
  if (s.version === version && s.snapshot) return Promise.resolve(s.snapshot);
  if (pending?.version === version) return pending.promise;
  const promise = api.topologySnapshot().then(
    (snapshot) => {
      if (pending?.promise === promise) pending = null;
      // A newer case may have opened while this one was loading.
      if (useTopology.getState().version <= version) useTopology.setState({ version, snapshot });
      return snapshot;
    },
    (e) => {
      if (pending?.promise === promise) pending = null;
      throw e;
    }
  );
  pending = { version, promise };
  return promise;
}

/** Share a snapshot changed in place, such as after merging zero-impedance branches. */
export function shareTopology(version: number, snapshot: TopologySnapshot) {
  useTopology.setState({ version, snapshot });
}
