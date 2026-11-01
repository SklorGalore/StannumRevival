// One channel of a secure transfer study against the transfer: the base
// case in black, then each contingency picked, each ending where it stopped
// solving. The secure limit and the channel's limits are marked. Hover for
// the values at a step; click a name in the legend to hide or show it.
import { useMemo, useState } from "react";
import { useWidth } from "../../ui/useWidth";
import type { ChannelCurves, SecureSummary } from "../../lib/analysis/commands.gen";
import { fixed } from "../../lib/analysis/format";
import { CURVE_COLOURS, DOTS, ticks } from "./PvChart";

const H = 320;
const PAD = { left: 56, right: 16, top: 14, bottom: 34 };

const UNITS: Record<string, string> = { pu: "p.u.", deg: "degrees", MW: "MW", Mvar: "Mvar", "%": "% of rating" };

/** Digits that tell values of this spread apart. */
function digits(span: number): number {
  return span < 0.05 ? 4 : span < 1 ? 3 : span < 20 ? 2 : span < 200 ? 1 : 0;
}

export function SecureChart({ curves, summary }: { curves: ChannelCurves; summary: SecureSummary }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);
  const [hidden, setHidden] = useState<Set<number>>(new Set());
  const xs = curves.transfer_mw;
  const ch = curves.channel;
  const shown = curves.curves.filter((c) => !hidden.has(c.index));
  // Contingencies keep their colour as others are hidden; the base case is black.
  const colour = (index: number) => {
    const k = curves.curves.filter((c) => c.index !== 0).findIndex((c) => c.index === index);
    return k % CURVE_COLOURS.length;
  };
  const xMax = Math.max(...xs, 1);
  const marks = [ch.low, ch.high].filter((v): v is number => v !== null && v !== undefined);
  const [yMin, yMax] = useMemo(() => {
    const vs = shown.flatMap((c) => c.values.filter((v): v is number => v !== null && v !== undefined));
    if (!vs.length) return [0, 1];
    let lo = Math.min(...vs);
    let hi = Math.max(...vs);
    // Show a limit when it is near the curves.
    const span = Math.max(hi - lo, Math.abs(hi) * 0.02, 1e-3);
    for (const m of marks) {
      if (m >= lo - span * 0.5 && m <= hi + span * 0.5) {
        lo = Math.min(lo, m);
        hi = Math.max(hi, m);
      }
    }
    const pad = Math.max((hi - lo) * 0.08, 1e-3);
    return [lo - pad, hi + pad];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shown, ch]);
  const plotW = Math.max(60, width - PAD.left - PAD.right);
  const plotH = H - PAD.top - PAD.bottom;
  const x = (v: number) => PAD.left + (v / xMax) * plotW;
  const y = (v: number) => PAD.top + (1 - (v - yMin) / (yMax - yMin)) * plotH;
  const xTicks = ticks(0, xMax, Math.max(2, Math.floor(plotW / 90)));
  const yTicks = ticks(yMin, yMax, 6);
  const d = digits(yMax - yMin);
  // Lines broken where a curve didn't solve.
  const paths = (values: (number | null | undefined)[]) => {
    const runs: string[] = [];
    let run: string[] = [];
    values.forEach((v, i) => {
      if (v === null || v === undefined) {
        if (run.length) runs.push(run.join(" "));
        run = [];
      } else run.push(`${x(xs[i])},${y(v)}`);
    });
    if (run.length) runs.push(run.join(" "));
    return runs;
  };
  const limit = summary.limit_mw;
  return (
    <div ref={ref} className="relative" data-testid="secure-chart">
      <svg
        width={width}
        height={H}
        role="img"
        aria-label={`${ch.name} against the transfer, for the base case and ${curves.curves.length - 1} contingencies`}
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
              {t.toFixed(d)}
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
          Transfer, MW
        </text>
        <text transform={`translate(11 ${PAD.top + plotH / 2}) rotate(-90)`} textAnchor="middle" fontSize={10} className="fill-slate-600">
          {UNITS[ch.unit] ?? ch.unit}
        </text>
        {marks
          .filter((m) => m > yMin && m < yMax)
          .map((m) => (
            <g key={m}>
              <line x1={PAD.left} x2={PAD.left + plotW} y1={y(m)} y2={y(m)} className="stroke-red-400" strokeDasharray="4 3" />
              <text x={PAD.left + plotW - 4} y={y(m) - 4} textAnchor="end" fontSize={10} className="fill-red-600">
                Limit {fixed(m, d)}
              </text>
            </g>
          ))}
        <line x1={x(limit)} x2={x(limit)} y1={PAD.top} y2={PAD.top + plotH} className="stroke-amber-500" strokeDasharray="5 3" />
        <text x={x(limit) + 4} y={PAD.top + 10} fontSize={10} className="fill-amber-700">
          Secure limit {fixed(limit, 0)} MW
        </text>
        {shown
          .filter((c) => c.index !== 0)
          .map((c) => (
            <g key={c.index} className={CURVE_COLOURS[colour(c.index)]}>
              {paths(c.values).map((p, k) => (
                <polyline key={k} points={p} fill="none" strokeWidth={1.5} strokeLinejoin="round" />
              ))}
            </g>
          ))}
        {shown
          .filter((c) => c.index === 0)
          .map((c) => (
            <g key={0} className="stroke-slate-900">
              {paths(c.values).map((p, k) => (
                <polyline key={k} points={p} fill="none" strokeWidth={2.4} strokeLinejoin="round" />
              ))}
              {xs.length <= 60 &&
                c.values.map((v, i) =>
                  v === null || v === undefined ? null : <circle key={i} cx={x(xs[i])} cy={y(v)} r={2} className="fill-white" strokeWidth={1.2} />
                )}
            </g>
          ))}
        {hover !== null && <line x1={x(xs[hover])} x2={x(xs[hover])} y1={PAD.top} y2={PAD.top + plotH} className="stroke-slate-400" />}
      </svg>
      {hover !== null && (
        <div
          className="pointer-events-none absolute z-10 rounded-md border border-slate-200 bg-white px-2 py-1 text-[11px] shadow-sm whitespace-nowrap"
          style={{ left: Math.min(x(xs[hover]) + 10, Math.max(0, width - 320)), top: PAD.top }}
          data-testid="secure-chart-tip"
        >
          <span className="font-medium text-slate-800">{fixed(xs[hover], 1)} MW</span>
          {shown.map((c) => {
            const v = c.values[hover];
            return (
              <span key={c.index} className="flex items-center gap-1.5 tabular-nums text-slate-700">
                <span className={`inline-block w-2 h-2 rounded-full ${c.index === 0 ? "bg-slate-900" : DOTS[colour(c.index)]}`} />
                {v === null || v === undefined ? <span className="text-red-600">no solution</span> : fixed(v, d)}
                <span className="text-slate-500 max-w-64 truncate">{c.name}</span>
              </span>
            );
          })}
        </div>
      )}
      <ul className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[11px]" aria-label="Curves">
        {curves.curves.map((c) => {
          const off = hidden.has(c.index);
          return (
            <li key={c.index}>
              <button
                type="button"
                className={`inline-flex items-center gap-1.5 rounded px-1 hover:bg-slate-50 ${off ? "text-slate-400 line-through" : "text-slate-700"}`}
                aria-pressed={!off}
                title={off ? "Show this curve" : "Hide this curve"}
                onClick={() =>
                  setHidden((s) => {
                    const next = new Set(s);
                    if (next.has(c.index)) next.delete(c.index);
                    else next.add(c.index);
                    return next;
                  })
                }
              >
                <span className={`inline-block w-3 ${c.index === 0 ? "h-1 bg-slate-900" : `h-0.5 ${DOTS[colour(c.index)]}`}`} />
                {c.name}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
