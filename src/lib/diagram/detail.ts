// Level of detail: zoomed out, results are drawn as marks (wire weight and
// colour by loading, flow arrows, a band on buses near or past their limits)
// instead of numbers, which can't be read at that size. Hovering shows the
// numbers, and exports always get the full text.
import { useStore } from "@xyflow/react";
import { useTheme } from "../../store/themeStore";
import type { BusRow } from "../analysis/types";

/** Below this zoom, results are drawn as marks instead of text. */
export const MARKS_BELOW_ZOOM = 0.6;

/** Whether results should be drawn as marks at the current zoom. */
export function useResultMarks(): boolean {
  const far = useStore((s) => s.transform[2] < MARKS_BELOW_ZOOM);
  const exporting = useTheme((s) => s.exportLight);
  return far && !exporting;
}

/** Margin to a voltage limit, in per unit, that counts as close. */
const NEAR_LIMIT_PU = 0.01;

/** How a bus voltage sits against its limits. */
export function voltageBand(row: Pick<BusRow, "vm" | "vmin" | "vmax" | "violation">): "past" | "near" | null {
  if (row.violation > 0) return "past";
  if (row.vmax > row.vmin && (row.vm - row.vmin < NEAR_LIMIT_PU || row.vmax - row.vm < NEAR_LIMIT_PU)) return "near";
  return null;
}

/**
 * Wire weight from loading, so heavy branches read from far away. A branch
 * without a rating is weighed by its flow against the largest in the case.
 */
export function flowWidth(loadingPct: number | null | undefined, mw: number, maxFlow: number, base: number): number {
  const share = loadingPct ?? (maxFlow > 0 ? (100 * Math.abs(mw)) / maxFlow : 0);
  return base + (4 * Math.min(Math.max(share, 0), 120)) / 100;
}
