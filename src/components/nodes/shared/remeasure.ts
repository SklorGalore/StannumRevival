import { useEffect, useRef } from "react";
import { useNodeId, useUpdateNodeInternals } from "@xyflow/react";

type Update = ReturnType<typeof useUpdateNodeInternals>;

// Nodes waiting to be re-measured, per React Flow instance.
const queues = new Map<Update, Set<string>>();

/** Re-measure `id` with the others queued this frame, in one store update. */
export function queue(update: Update, id: string) {
  let ids = queues.get(update);
  if (ids) {
    ids.add(id);
    return;
  }
  ids = new Set([id]);
  queues.set(update, ids);
  requestAnimationFrame(() => {
    queues.delete(update);
    update([...ids]);
  });
}

/**
 * Re-measure this node's handles when `key` changes (a symbol turns, a bus
 * gains a tap), so wires follow. React Flow measures a node itself when it
 * first appears, so nothing happens on mount, and changes to many nodes at
 * once (an undo, a redraw) are measured together: one update per node would
 * pass React's nested-update limit on large drawings.
 */
export function useRemeasure(key: string) {
  const id = useNodeId();
  const update = useUpdateNodeInternals();
  const last = useRef(key);
  useEffect(() => {
    if (last.current === key) return;
    last.current = key;
    if (id) queue(update, id);
  }, [id, key, update]);
}
