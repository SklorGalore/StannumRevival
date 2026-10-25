import { useMemo } from "react";
import { Panel } from "@xyflow/react";
import { useDiagramStore } from "../../store/diagramStore";
import { useInk } from "../../lib/diagram/ink";
import { voltageLevel } from "../nodes/shared/voltageColor";
import type { BusData } from "../../store/types";

/** Which colour stands for which base kV, for the voltages in the drawing. */
export function VoltageLegend() {
  const nodes = useDiagramStore((s) => s.nodes);
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

  if (ink.mono || levels.length < 2) return null;
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
      </div>
    </Panel>
  );
}

const fmt = (kv: number) => String(Number(kv.toFixed(2)));
