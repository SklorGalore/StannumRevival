import { useMemo } from "react";
import { Panel } from "@xyflow/react";
import { useDiagramStore } from "../../store/diagramStore";
import { useInk } from "../../lib/diagram/ink";
import { voltageLevel } from "../nodes/shared/voltageColor";
import type { BusData } from "../../store/types";

/** Which colour stands for which base kV, for the voltages in the drawing. */
export function VoltageLegend() {
  const nodes = useDiagramStore((s) => s.nodes);
  const hasDc = useDiagramStore(
    (s) => s.nodes.some((n) => n.type === "dcLink") || s.edges.some((e) => e.data?.caseRef?.kind === "dcline")
  );
  const ink = useInk();
  const levels = useMemo(() => {
    const kvs = new Set<number>();
    for (const n of nodes) {
      const kv = n.type === "bus" ? (n.data as BusData).voltageKV : 0;
      if (kv > 0) kvs.add(kv);
    }
    // Group the case's voltages by the level that colours them.
    const byLevel = new Map<number, number[]>();
    for (const kv of [...kvs].sort((a, b) => b - a)) {
      const level = voltageLevel(kv).kv;
      byLevel.set(level, [...(byLevel.get(level) ?? []), kv]);
    }
    return [...byLevel.values()];
  }, [nodes]);

  if (ink.mono || levels.length + Number(hasDc) < 2) return null;
  return (
    <Panel position="top-right">
      <div
        data-testid="voltage-legend"
        aria-label="Voltage colours"
        className="rounded-md border border-slate-200 bg-white/90 px-2.5 py-1.5 text-[11px] text-slate-600 shadow-sm"
      >
        {levels.map((kvs) => (
          <div key={kvs[0]} className="flex items-center gap-2 leading-5">
            <span className="inline-block h-1.5 w-5 rounded-sm" style={{ background: ink.voltage(kvs[0]) }} />
            <span className="tabular-nums">{kvs.map(fmt).join(", ")} kV</span>
          </div>
        ))}
        {hasDc && (
          <div className="flex items-center gap-2 leading-5">
            <svg width={20} height={6} className="shrink-0">
              <line x1={0} y1={3} x2={20} y2={3} stroke={ink.line} strokeWidth={2} strokeDasharray="6 4" />
            </svg>
            <span>DC line</span>
          </div>
        )}
      </div>
    </Panel>
  );
}

const fmt = (kv: number) => String(Number(kv.toFixed(2)));
