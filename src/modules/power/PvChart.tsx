// The PV curve picture: voltage of each bus with a curve against the
// transfer, with the nose, the low voltage limit and where a bus first
// crosses it marked. Hover for the values at a point; click a line's name in
// the legend to hide or show it.
import { useMemo, useState } from "react";
import { useWidth } from "../../ui/useWidth";
import type { PvResults } from "../../lib/analysis/commands.gen";
import { fixed } from "../../lib/analysis/format";

/** Line colours, in the app's palette (they follow the dark theme). */
export const CURVE_COLOURS = [
  "stroke-teal-600",
  "stroke-blue-600",
  "stroke-amber-600",
  "stroke-red-600",
  "stroke-emerald-600",
  "stroke-sky-600",
  "stroke-orange-600",
  "stroke-slate-500",
];
/** The same colours as backgrounds, spelled out so Tailwind keeps them. */
const DOTS = ["bg-teal-600", "bg-blue-600", "bg-amber-600", "bg-red-600", "bg-emerald-600", "bg-sky-600", "bg-orange-600", "bg-slate-500"];

const H = 300;
const PAD = { left: 48, right: 16, top: 14, bottom: 34 };

/** Round axis steps: 1, 2 or 5 times a power of ten. */
function ticks(lo: number, hi: number, n: number): number[] {
  const span = hi - lo || 1;
  const raw = span / n;
  const p = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((m) => m * p).find((s) => s >= raw) ?? raw;
  const out: number[] = [];
  for (let t = Math.ceil(lo / step) * step; t <= hi + step * 1e-9; t += step) out.push(t);
  return out;
}

export function curveLabel(c: { bus: number; name: string; base_kv: number }): string {
  const name = c.name.trim();
  return `${c.bus}${name ? ` ${name}` : ""}${c.base_kv > 0 ? ` (${fixed(c.base_kv, 0)} kV)` : ""}`;
}

export function PvChart({ results, onPick }: { results: PvResults; onPick: (bus: number) => void }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);
  const [hidden, setHidden] = useState<Set<number>>(new Set());
  const { points, curves, summary } = results;
  const shown = curves.filter((c) => !hidden.has(c.bus));
  const xs = points.map((p) => p.transfer_mw);
  const xMax = Math.max(...xs, 1);
  const limits = [...new Set(curves.map((c) => c.vmin_pu))];
  const oneLimit = limits.length === 1 ? limits[0] : null;
  const [yMin, yMax] = useMemo(() => {
    const vs = shown.flatMap((c) => c.vm_pu);
    if (!vs.length) return [0.9, 1.05];
    let lo = Math.min(...vs);
    let hi = Math.max(...vs);
    if (oneLimit !== null && oneLimit >= lo - 0.05) lo = Math.min(lo, oneLimit);
    const pad = Math.max(0.005, (hi - lo) * 0.08);
    lo -= pad;
    hi += pad;
    return [lo, hi];
  }, [shown, oneLimit]);
  const plotW = Math.max(60, width - PAD.left - PAD.right);
  const plotH = H - PAD.top - PAD.bottom;
  const x = (v: number) => PAD.left + (v / xMax) * plotW;
  const y = (v: number) => PAD.top + (1 - (v - yMin) / (yMax - yMin)) * plotH;
  const xTicks = ticks(0, xMax, Math.max(2, Math.floor(plotW / 90)));
  const yTicks = ticks(yMin, yMax, 5);
  const nose = summary.limited_by === "collapse";
  const crossing = summary.voltage_limit_mw;
  const colour = (bus: number) => curves.findIndex((c) => c.bus === bus) % CURVE_COLOURS.length;
  const h = hover === null ? null : points[hover];
  return (
    <div ref={ref} className="relative" data-testid="pv-chart">
      <svg
        width={width}
        height={H}
        role="img"
        aria-label={`PV curve: voltage of ${curves.length} buses against the transfer, up to ${fixed(summary.nose_mw, 0)} MW`}
        onMouseLeave={() => setHover(null)}
        onMouseMove={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          const mw = ((e.clientX - box.left - PAD.left) / plotW) * xMax;
          let best = -1;
          let gap = Infinity;
          xs.forEach((v, i) => {
            if (Math.abs(v - mw) < gap) [best, gap] = [i, Math.abs(v - mw)];
          });
          setHover(best >= 0 && mw >= -xMax * 0.02 && mw <= xMax * 1.02 ? best : null);
        }}
      >
        {yTicks.map((t) => (
          <g key={`y${t}`}>
            <line x1={PAD.left} x2={PAD.left + plotW} y1={y(t)} y2={y(t)} className="stroke-slate-100" />
            <text x={PAD.left - 6} y={y(t) + 3} textAnchor="end" fontSize={10} className="fill-slate-500 tabular-nums">
              {t.toFixed(yMax - yMin < 0.1 ? 3 : 2)}
            </text>
          </g>
        ))}
        {xTicks.map((t) => (
          <g key={`x${t}`}>
            <line x1={x(t)} x2={x(t)} y1={PAD.top} y2={PAD.top + plotH} className="stroke-slate-100" />
            <text x={x(t)} y={PAD.top + plotH + 14} textAnchor="middle" fontSize={10} className="fill-slate-500 tabular-nums">
              {fixed(t, 0)}
            </text>
          </g>
        ))}
        <text x={PAD.left + plotW / 2} y={H - 4} textAnchor="middle" fontSize={10} className="fill-slate-600">
          Transfer added, MW
        </text>
        <text
          transform={`translate(11 ${PAD.top + plotH / 2}) rotate(-90)`}
          textAnchor="middle"
          fontSize={10}
          className="fill-slate-600"
        >
          Voltage, p.u.
        </text>
        {oneLimit !== null && oneLimit > yMin && oneLimit < yMax && (
          <g>
            <line x1={PAD.left} x2={PAD.left + plotW} y1={y(oneLimit)} y2={y(oneLimit)} className="stroke-red-400" strokeDasharray="4 3" />
            <text x={PAD.left + 4} y={y(oneLimit) - 4} fontSize={10} className="fill-red-600">
              Low limit {fixed(oneLimit, 2)} p.u.
            </text>
          </g>
        )}
        {crossing !== null && crossing !== undefined && (
          <line x1={x(crossing)} x2={x(crossing)} y1={PAD.top} y2={PAD.top + plotH} className="stroke-amber-500" strokeDasharray="2 3" />
        )}
        <line
          x1={x(summary.nose_mw)}
          x2={x(summary.nose_mw)}
          y1={PAD.top}
          y2={PAD.top + plotH}
          className="stroke-slate-500"
          strokeDasharray="5 3"
        />
        <text x={x(summary.nose_mw) - 4} y={PAD.top + 10} textAnchor="end" fontSize={10} className="fill-slate-700">
          {nose ? "Nose" : "Stopped"} {fixed(summary.nose_mw, 0)} MW
        </text>
        {shown.map((c) => (
          <g key={c.bus} className={CURVE_COLOURS[colour(c.bus)]}>
            <polyline
              points={c.vm_pu.map((v, i) => `${x(xs[i])},${y(v)}`).join(" ")}
              fill="none"
              strokeWidth={1.8}
              strokeLinejoin="round"
            />
            {points.length <= 60 &&
              c.vm_pu.map((v, i) => <circle key={i} cx={x(xs[i])} cy={y(v)} r={2} className="fill-white" strokeWidth={1.4} />)}
          </g>
        ))}
        {h && (
          <line x1={x(h.transfer_mw)} x2={x(h.transfer_mw)} y1={PAD.top} y2={PAD.top + plotH} className="stroke-slate-400" />
        )}
      </svg>
      {h && hover !== null && (
        <div
          className="pointer-events-none absolute z-10 rounded-md border border-slate-200 bg-white px-2 py-1 text-[11px] shadow-sm whitespace-nowrap"
          style={{ left: Math.min(x(h.transfer_mw) + 10, width - 230), top: PAD.top }}
          data-testid="pv-chart-tip"
        >
          <span className="font-medium text-slate-800">+{fixed(h.transfer_mw, 1)} MW</span>
          <span className="text-slate-500"> · sink load {fixed(h.sink_load_mw, 0)} MW</span>
          {shown.map((c) => (
            <span key={c.bus} className="flex items-center gap-1.5 tabular-nums text-slate-700">
              <span className={`inline-block w-2 h-2 rounded-full ${DOTS[colour(c.bus)]}`} />
              {fixed(c.vm_pu[hover], 4)} <span className="text-slate-500">{curveLabel(c)}</span>
            </span>
          ))}
          <span className="block text-slate-500">
            {h.q_limited} generators at a Q limit · {fixed(h.q_reserve_mvar, 0)} Mvar reserve
          </span>
        </div>
      )}
      <ul className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[11px]" aria-label="Curves">
        {curves.map((c) => {
          const off = hidden.has(c.bus);
          return (
            <li key={c.bus} className="inline-flex items-center gap-1">
              <button
                type="button"
                className={`inline-flex items-center gap-1.5 rounded px-1 hover:bg-slate-50 ${off ? "text-slate-400 line-through" : "text-slate-700"}`}
                aria-pressed={!off}
                title={off ? "Show this curve" : "Hide this curve"}
                onClick={() =>
                  setHidden((s) => {
                    const next = new Set(s);
                    if (next.has(c.bus)) next.delete(c.bus);
                    else next.add(c.bus);
                    return next;
                  })
                }
              >
                <span className={`inline-block w-3 h-0.5 ${DOTS[colour(c.bus)]}`} />
                {curveLabel(c)}
              </button>
              <button
                type="button"
                className="text-slate-400 hover:text-teal-700"
                onClick={() => onPick(c.bus)}
                title={`List bus ${c.bus}`}
                aria-label={`List bus ${c.bus}`}
              >
                ↗
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
