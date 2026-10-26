import type { Rotation, SLDNode } from "../../store/types";

/** Size of each symbol drawn upright. */
export const SYMBOL_SIZE: Record<string, { w: number; h: number }> = {
  generator: { w: 40, h: 80 },
  load: { w: 40, h: 60 },
  transformer: { w: 40, h: 100 },
  transformer3: { w: 80, h: 100 },
  dcLink: { w: 40, h: 140 },
  offPage: { w: 40, h: 40 },
  shunt: { w: 40, h: 60 },
  circuitBreaker: { w: 40, h: 60 },
  disconnector: { w: 40, h: 60 },
  fuse: { w: 40, h: 60 },
};

/** Size a symbol takes on the canvas: as measured, or its drawn size turned.
 *  `drawn` skips the measurement, which lags a turn by a frame. */
export function symbolSize(n: SLDNode, drawn = false): { w: number; h: number } {
  const base = SYMBOL_SIZE[n.type ?? ""] ?? { w: 40, h: 60 };
  const r = (n.data as { rotation?: Rotation }).rotation ?? 0;
  const turned = r === 90 || r === 270 ? { w: base.h, h: base.w } : base;
  if (drawn) return turned;
  return { w: n.measured?.width ?? n.width ?? turned.w, h: n.measured?.height ?? n.height ?? turned.h };
}
