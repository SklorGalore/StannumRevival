import { lazy, Suspense, type ComponentProps } from "react";
import type { NetworkGraph as Graph } from "./NetworkGraph";

// Cytoscape and its layouts are about a megabyte, so they load the first
// time a network is drawn rather than at startup.
const Inner = lazy(() => import("./NetworkGraph").then((m) => ({ default: m.NetworkGraph })));

/** `NetworkGraph`, loaded on first use. */
export function NetworkGraph(props: ComponentProps<typeof Graph>) {
  return (
    <Suspense fallback={<div className="absolute inset-0 flex items-center justify-center text-xs text-slate-500">Loading the network view…</div>}>
      <Inner {...props} />
    </Suspense>
  );
}
