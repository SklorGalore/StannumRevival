// Two pictures for the power flow overview: every bus voltage against its
// limits, lowest to highest, and the most heavily loaded branches.
import { useMemo, useState } from "react";
import { useWidth } from "../../ui/useWidth";
import type { BranchRow, BusRow } from "../../lib/analysis/types";
import { fixed } from "../../lib/analysis/format";
import { voltageBand } from "../../lib/diagram/detail";
import { DrawAroundButton } from "../../ui/DrawAround";

/** The most common value, for drawing one limit band across all buses. */
function mode(values: number[]): number | null {
  const counts = new Map<number, number>();
  for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
  let best: number | null = null;
  let n = 0;
  for (const [v, c] of counts) if (c > n) [best, n] = [v, c];
  return best;
}

const H = 180;
const PAD = { left: 44, right: 12, top: 10, bottom: 22 };

/**
 * Every bus, sorted by voltage magnitude, against the usual limit band. A bus
 * past its own limits is red, one close to them amber. Click a bus to list it.
 */
export function VoltageProfile({ buses, onPick }: { buses: BusRow[]; onPick: (bus: number) => void }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);
  const sorted = useMemo(() => buses.filter((b) => b.kind !== "Isolated").sort((a, b) => a.vm - b.vm || a.id - b.id), [buses]);
  const lo = mode(sorted.map((b) => b.vmin).filter((v) => v > 0));
  const hi = mode(sorted.map((b) => b.vmax).filter((v) => v > 0));
  if (!sorted.length) return null;
  const vms = sorted.map((b) => b.vm);
  const yMin = Math.min(...vms, lo ?? Infinity) - 0.01;
  const yMax = Math.max(...vms, hi ?? -Infinity) + 0.01;
  const plotW = Math.max(40, width - PAD.left - PAD.right);
  const plotH = H - PAD.top - PAD.bottom;
  const x = (i: number) => PAD.left + (sorted.length === 1 ? plotW / 2 : (i / (sorted.length - 1)) * plotW);
  const y = (v: number) => PAD.top + (1 - (v - yMin) / (yMax - yMin)) * plotH;
  const ticks = [yMin + 0.01, (yMin + yMax) / 2, yMax - 0.01];
  const r = sorted.length > 400 ? 2 : sorted.length > 120 ? 2.5 : 3.5;
  const h = hover === null ? null : sorted[hover];
  const counts = { past: 0, near: 0 };
  for (const b of sorted) {
    const band = voltageBand(b);
    if (band) counts[band]++;
  }
  return (
    <div ref={ref} className="relative" data-testid="voltage-profile">
      <svg
        width={width}
        height={H}
        role="img"
        aria-label={`Voltage of ${sorted.length} buses, lowest to highest: ${counts.past} outside limits, ${counts.near} close to a limit`}
        onMouseLeave={() => setHover(null)}
        onMouseMove={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          const px = e.clientX - box.left;
          const i = Math.round(((px - PAD.left) / plotW) * (sorted.length - 1));
          setHover(i >= 0 && i < sorted.length ? i : null);
        }}
        onClick={() => h && onPick(h.id)}
        className={h ? "cursor-pointer" : ""}
      >
        {lo !== null && hi !== null && (
          <rect x={PAD.left} y={y(hi)} width={plotW} height={y(lo) - y(hi)} className="fill-teal-50" />
        )}
        {[lo, hi].map(
          (v, i) =>
            v !== null && (
              <line key={i} x1={PAD.left} x2={PAD.left + plotW} y1={y(v)} y2={y(v)} className="stroke-teal-300" strokeDasharray="3 3" />
            )
        )}
        {ticks.map((t) => (
          <text key={t} x={PAD.left - 6} y={y(t) + 3} textAnchor="end" fontSize={10} className="fill-slate-500 tabular-nums">
            {t.toFixed(2)}
          </text>
        ))}
        <text x={PAD.left} y={H - 6} fontSize={10} className="fill-slate-500">
          Lowest
        </text>
        <text x={PAD.left + plotW} y={H - 6} fontSize={10} textAnchor="end" className="fill-slate-500">
          Highest
        </text>
        <polyline
          points={sorted.map((b, i) => `${x(i)},${y(b.vm)}`).join(" ")}
          fill="none"
          className="stroke-slate-300"
          strokeWidth={1.5}
        />
        {sorted.map((b, i) => {
          const band = voltageBand(b);
          if (!band && sorted.length > 60 && i !== hover) return null;
          return (
            <circle
              key={b.id}
              cx={x(i)}
              cy={y(b.vm)}
              r={i === hover ? r + 2 : band ? r + 0.5 : r}
              className={band === "past" ? "fill-red-600" : band === "near" ? "fill-amber-500" : "fill-slate-500"}
              stroke="var(--color-white)"
              strokeWidth={1}
            />
          );
        })}
      </svg>
      {h && (
        <div
          className="pointer-events-none absolute z-10 rounded-md border border-slate-200 bg-white px-2 py-1 text-[11px] shadow-sm whitespace-nowrap"
          style={{ left: Math.min(x(hover!) + 8, width - 180), top: Math.max(0, y(h.vm) - 40) }}
          data-testid="voltage-profile-tip"
        >
          <span className="font-medium text-slate-800">Bus {h.id}</span>
          {h.name.trim() && <span className="text-slate-500"> {h.name.trim()}</span>}
          <span className="block tabular-nums text-slate-700">
            {fixed(h.vm, 4)} p.u. · limits {fixed(h.vmin, 2)} to {fixed(h.vmax, 2)}
          </span>
        </div>
      )}
      <p className="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-slate-500">
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-block w-2.5 h-2.5 rounded-full bg-red-600" /> Outside limits ({counts.past})
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-block w-2.5 h-2.5 rounded-full bg-amber-500" /> Within 0.01 p.u. of a limit ({counts.near})
        </span>
        {lo !== null && hi !== null && (
          <span className="inline-flex items-center gap-1.5">
            <span className="inline-block w-4 h-2.5 bg-teal-50 border-y border-dashed border-teal-300" /> Usual limits {fixed(lo, 2)} to {fixed(hi, 2)}
          </span>
        )}
      </p>
    </div>
  );
}

/**
 * The most heavily loaded branches against rating A. A case without ratings
 * gets its largest flows instead, so the panel is never empty.
 */
export function TopBranches({
  branches,
  names,
  onPick,
  limit = 10,
}: {
  branches: BranchRow[];
  names: Map<number, string>;
  onPick: (element: string) => void;
  limit?: number;
}) {
  const rated = branches.filter((b) => b.online && b.loading_pct !== null);
  const byLoading = rated.length > 0;
  const top = (byLoading ? rated : branches.filter((b) => b.online))
    .map((b) => ({ b, v: byLoading ? b.loading_pct! : Math.max(Math.abs(b.pf), Math.abs(b.pt)) }))
    .sort((a, b) => b.v - a.v)
    .slice(0, limit);
  if (!top.length) return null;
  const scale = byLoading ? Math.max(100, top[0].v) : top[0].v || 1;
  const end = (bus: number) => (names.get(bus) ? `${bus} ${names.get(bus)}` : `${bus}`);
  return (
    <div data-testid="top-branches">
      {!byLoading && <p className="mb-2 text-[11px] text-slate-500">No branch in this case has a rating, so these are the largest flows.</p>}
      <ul className="flex flex-col gap-1">
        {top.map(({ b, v }) => {
          const tone = !byLoading ? "bg-slate-400" : v > 100 ? "bg-red-600" : v > 90 ? "bg-amber-500" : "bg-teal-600";
          return (
            <li key={b.index} className="group flex items-center gap-2 text-xs">
              <button
                type="button"
                className="flex-1 min-w-0 grid grid-cols-[minmax(0,14rem)_1fr_4.5rem] items-center gap-2 rounded px-1 py-0.5 text-left hover:bg-slate-50"
                onClick={() => onPick(b.element)}
                title={`List ${b.is_transformer ? "transformer" : "line"} ${b.from} to ${b.to}, circuit ${b.circuit}`}
              >
                <span className="truncate text-slate-700">
                  {end(b.from)} → {end(b.to)}
                  {b.circuit !== "1" && <span className="text-slate-500"> ({b.circuit})</span>}
                </span>
                <span className="relative h-2.5 rounded-sm bg-slate-100">
                  <span className={`absolute inset-y-0 left-0 rounded-sm ${tone}`} style={{ width: `${Math.min(100, (v / scale) * 100)}%` }} />
                  {byLoading && scale > 100 && (
                    <span className="absolute inset-y-[-2px] w-px bg-slate-500" style={{ left: `${(100 / scale) * 100}%` }} title="Rating A" />
                  )}
                </span>
                <span className="text-right tabular-nums text-slate-800">{byLoading ? `${fixed(v, 1)}%` : `${fixed(v, 1)} MW`}</span>
              </button>
              <DrawAroundButton element={b.element} from="power" compact className="opacity-0 group-hover:opacity-100 focus:opacity-100" />
            </li>
          );
        })}
      </ul>
    </div>
  );
}
