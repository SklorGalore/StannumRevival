// The QV curve picture: the condenser's reactive output against the voltage
// it holds, one line per bus, with zero, each curve's bottom (the margin)
// and each bus's solved voltage marked. Hover for the values at a voltage;
// click a bus's name in the legend to hide or show it.
import { useMemo, useState } from "react";
import { useWidth } from "../../ui/useWidth";
import type { QvCurve, QvResults } from "../../lib/analysis/commands.gen";
import { fixed } from "../../lib/analysis/format";
import { CURVE_COLOURS, DOTS, curveLabel, ticks } from "./PvChart";

const H = 300;
const PAD = { left: 56, right: 16, top: 14, bottom: 34 };

/** The curve's point nearest voltage `v`, if one lies within `gap` p.u. */
function nearest(c: QvCurve, v: number, gap: number): number | null {
  let best: number | null = null;
  let d = gap;
  c.v_pu.forEach((x, i) => {
    if (Math.abs(x - v) <= d) [best, d] = [i, Math.abs(x - v)];
  });
  return best;
}

export function QvChart({ results, onPick }: { results: QvResults; onPick: (bus: number) => void }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);
  const [hidden, setHidden] = useState<Set<number>>(new Set());
  const curves = results.curves.filter((c) => c.v_pu.length > 0);
  const shown = curves.filter((c) => !hidden.has(c.bus));
  const colour = (bus: number) => results.curves.findIndex((c) => c.bus === bus) % CURVE_COLOURS.length;
  const [xMin, xMax, yMin, yMax] = useMemo(() => {
    const vs = curves.flatMap((c) => c.v_pu);
    const qs = shown.flatMap((c) => c.q_mvar);
    if (!vs.length) return [0.5, 1.1, -100, 100];
    let lo = Math.min(...qs, 0);
    let hi = Math.max(...qs, 0);
    const pad = Math.max(1, (hi - lo) * 0.06);
    lo -= pad;
    hi += pad;
    return [Math.min(...vs), Math.max(...vs), lo, hi];
  }, [curves, shown]);
  const plotW = Math.max(60, width - PAD.left - PAD.right);
  const plotH = H - PAD.top - PAD.bottom;
  const span = xMax - xMin || 1;
  const x = (v: number) => PAD.left + ((v - xMin) / span) * plotW;
  const y = (q: number) => PAD.top + (1 - (q - yMin) / (yMax - yMin)) * plotH;
  const xTicks = ticks(xMin, xMax, Math.max(2, Math.floor(plotW / 70)));
  const yTicks = ticks(yMin, yMax, 7);
  const step = results.summary.step_pu;
  const at = hover === null ? [] : shown.map((c) => ({ c, i: nearest(c, hover, step * 0.75) }));
  return (
    <div ref={ref} className="relative" data-testid="qv-chart">
      <svg
        width={width}
        height={H}
        role="img"
        aria-label={`QV curves of ${curves.length} buses: reactive output of a condenser against the voltage it holds`}
        onMouseLeave={() => setHover(null)}
        onMouseMove={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          const v = xMin + ((e.clientX - box.left - PAD.left) / plotW) * span;
          setHover(v >= xMin - span * 0.02 && v <= xMax + span * 0.02 ? v : null);
        }}
      >
        {yTicks.map((t) => (
          <g key={`y${t}`}>
            <line x1={PAD.left} x2={PAD.left + plotW} y1={y(t)} y2={y(t)} className="stroke-slate-100" />
            <text x={PAD.left - 6} y={y(t) + 3} textAnchor="end" fontSize={10} className="fill-slate-500 tabular-nums">
              {fixed(t, 0)}
            </text>
          </g>
        ))}
        {xTicks.map((t) => (
          <g key={`x${t}`}>
            <line x1={x(t)} x2={x(t)} y1={PAD.top} y2={PAD.top + plotH} className="stroke-slate-100" />
            <text x={x(t)} y={PAD.top + plotH + 14} textAnchor="middle" fontSize={10} className="fill-slate-500 tabular-nums">
              {t.toFixed(2)}
            </text>
          </g>
        ))}
        <text x={PAD.left + plotW / 2} y={H - 4} textAnchor="middle" fontSize={10} className="fill-slate-600">
          Voltage held by the condenser, p.u.
        </text>
        <text transform={`translate(11 ${PAD.top + plotH / 2}) rotate(-90)`} textAnchor="middle" fontSize={10} className="fill-slate-600">
          Condenser output, Mvar
        </text>
        <line x1={PAD.left} x2={PAD.left + plotW} y1={y(0)} y2={y(0)} className="stroke-slate-400" />
        <text x={PAD.left + 4} y={y(0) - 4} fontSize={10} className="fill-slate-500">
          injects ↑ · absorbs ↓
        </text>
        {shown.map((c) => (
          <g key={c.bus} className={CURVE_COLOURS[colour(c.bus)]}>
            <polyline
              points={c.v_pu.map((v, i) => `${x(v)},${y(c.q_mvar[i])}`).join(" ")}
              fill="none"
              strokeWidth={1.8}
              strokeLinejoin="round"
            />
            <line
              x1={x(c.v_at_q_min_pu)}
              x2={x(c.v_at_q_min_pu)}
              y1={y(0)}
              y2={y(c.q_min_mvar)}
              strokeWidth={1}
              strokeDasharray="3 3"
            />
            <circle cx={x(c.v_at_q_min_pu)} cy={y(c.q_min_mvar)} r={3.5} strokeWidth={1.4} className="fill-white" />
            <rect x={x(c.base_vm_pu) - 3} y={y(0) - 3} width={6} height={6} strokeWidth={1.4} className="fill-white" />
          </g>
        ))}
        {hover !== null && <line x1={x(hover)} x2={x(hover)} y1={PAD.top} y2={PAD.top + plotH} className="stroke-slate-400" />}
      </svg>
      {hover !== null && at.some((a) => a.i !== null) && (
        <div
          className="pointer-events-none absolute z-10 rounded-md border border-slate-200 bg-white px-2 py-1 text-[11px] shadow-sm whitespace-nowrap"
          style={{ left: Math.min(x(hover) + 10, width - 240), top: PAD.top }}
          data-testid="qv-chart-tip"
        >
          {at.map(({ c, i }) =>
            i === null ? null : (
              <span key={c.bus} className="flex items-center gap-1.5 tabular-nums text-slate-700">
                <span className={`inline-block w-2 h-2 rounded-full ${DOTS[colour(c.bus)]}`} />
                {fixed(c.v_pu[i], 3)} p.u.: {fixed(c.q_mvar[i], 1)} Mvar
                <span className="text-slate-500">
                  {curveLabel(c)} · {c.q_limited[i]} at a Q limit
                </span>
              </span>
            ),
          )}
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
                {curveLabel(c)}: {fixed(c.margin_mvar, 0)} Mvar
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
      <p className="mt-1 text-[11px] text-slate-500">
        Circle: the bottom of each curve, its margin below zero. Square: the bus's solved voltage, where the condenser gives nothing.
      </p>
    </div>
  );
}
