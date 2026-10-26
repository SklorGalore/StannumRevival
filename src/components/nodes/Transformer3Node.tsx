import type { NodeProps } from "@xyflow/react";
import type { Transformer3Node as Transformer3NodeType, Transformer3Leg } from "../../store/types";
import { loadingColor, useResults } from "../../lib/diagram/results";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";
import { T3, T3_TERMINALS } from "../../lib/diagram/transformer3";

const TERMINALS: Terminal[] = T3_TERMINALS.map((t) => ({ ...t, type: "source" }));

const kvText = (kv: number) => (kv >= 100 ? kv.toFixed(0) : String(+kv.toFixed(2)));

/**
 * A three-winding transformer as one symbol: three overlapping circles, the
 * IEC symbol, with a terminal for each winding. It stands in for the case's
 * internal star bus and the three windings that meet there. Each winding's
 * flow is drawn on its own wire, so the symbol only carries the worst
 * loading.
 */
export function Transformer3Node({ data, selected }: NodeProps<Transformer3NodeType>) {
  const ink = useInk();
  const results = useResults();
  const legs: Transformer3Leg[] = data.legs ?? [];
  const rows = legs.map((l) => (l.caseRef?.kind === "branch" ? results?.branches.get(l.caseRef.id) : undefined));
  const worst = rows.reduce<number | null>((m, r) => (r?.loading_pct != null && (m == null || r.loading_pct > m) ? r.loading_pct : m), null);
  const overload = loadingColor(worst);
  const offline = data.outOfService || (rows.length > 0 && rows.every((r) => r && !r.online));
  const outline = selected ? ink.selected : offline ? ink.off : overload ? ink.alert(overload) : ink.line;
  const dash = offline ? "4 3" : undefined;
  const legOff = (handle: string) => {
    const i = legs.findIndex((l) => l.handle === handle);
    return i >= 0 && (legs[i].outOfService || (rows[i] && !rows[i]!.online));
  };
  const stem = (handle: string) => (legOff(handle) ? ink.off : ink.line);
  // Windings in their case order (W1, W2, W3), for the text.
  const ordered = [...legs].sort((a, b) => a.winding - b.winding);
  const kvs = ordered.map((l) => kvText(l.kv)).join("/");
  const mvas = ordered.map((l) => (l.ratingMVA > 0 ? l.ratingMVA.toFixed(0) : "–")).join("/");
  const { top, left, right } = T3.circles;

  return (
    <SymbolFrame
      element={data.caseRef?.element}
      w={T3.w}
      h={T3.h}
      focus={data.focus}
      rotation={data.rotation}
      labelOffset={data.labelOffset}
      terminals={TERMINALS}
      title={data.label}
      graphic={
        <>
          <line x1={T3.w / 2} y1={0} x2={T3.w / 2} y2={top.y - T3.r} stroke={stem("x3-a")} strokeWidth={STROKE} />
          <line x1={20} y1={T3.legY} x2={20} y2={T3.h} stroke={stem("x3-b")} strokeWidth={STROKE} />
          <line x1={T3.w - 20} y1={T3.legY} x2={T3.w - 20} y2={T3.h} stroke={stem("x3-c")} strokeWidth={STROKE} />
          {[top, left, right].map((c, i) => (
            <circle
              key={i}
              cx={c.x}
              cy={c.y}
              r={T3.r}
              fill="transparent"
              stroke={outline}
              strokeWidth={STROKE}
              strokeDasharray={dash}
            />
          ))}
        </>
      }
      labelTop={22}
      label={
        <>
          <text y={34} fontSize={10} fill={ink.text}>
            {data.label}
          </text>
          {legs.length > 0 && (
            <>
              <text y={46} fontSize={9} fill={ink.muted}>
                {kvs} kV
              </text>
              <text y={58} fontSize={9} fill={ink.muted}>
                {mvas} MVA
              </text>
            </>
          )}
          {worst != null && (
            <text y={72} fontSize={9} fill={overload ? ink.alert(overload) : ink.result} fontWeight={overload ? 600 : 400}>
              {worst.toFixed(0)}% worst winding
            </text>
          )}
        </>
      }
    />
  );
}
