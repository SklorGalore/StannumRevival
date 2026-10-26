import type { NodeProps } from "@xyflow/react";
import type { OffPageNode as OffPageNodeType, OffPageLink } from "../../store/types";
import { useInk, STROKE } from "../../lib/diagram/ink";
import { branchFlow, loadingColor, mw, useResults, type Results } from "../../lib/diagram/results";
import { SymbolFrame, type Terminal } from "./shared/SymbolFrame";

const W = 40;
const H = 40;
const CX = W / 2;
const TERMINALS: Terminal[] = [{ id: "op-top", type: "source", x: CX, y: 0, side: "top" }];

const flag = (dx: number) =>
  `${CX - 8 + dx},14 ${CX + 8 + dx},14 ${CX + 8 + dx},30 ${CX + dx},${H - 2} ${CX - 8 + dx},30`;

/** One line of a bundled marker's tooltip: where the branch goes and its flow. */
function describe(l: OffPageLink, results: Results | null): string {
  const where = l.busName && l.busName !== `Bus ${l.bus}` ? `${l.busName} (${l.bus})` : `Bus ${l.bus}`;
  const row = l.caseRef?.kind === "branch" ? results?.branches.get(l.caseRef.id) : undefined;
  if (!row) return `${where}${l.outOfService ? ", out of service" : ""}`;
  const p = branchFlow(row, l.reversed).p;
  return `${where}: ${mw(Math.abs(p))} ${p >= 0 ? "out" : "in"}${row.loading_pct != null ? `, ${row.loading_pct.toFixed(0)}%` : ""}`;
}

/** A branch that continues to a bus outside the drawing, or all of a bus's
 *  branches that do (a bundle, drawn as two flags). A single branch's flow is
 *  shown on the wire leading to it; a bundle lists flows in its tooltip and
 *  takes the colour of its most loaded branch. */
export function OffPageNode({ data, selected }: NodeProps<OffPageNodeType>) {
  const ink = useInk();
  const results = useResults();
  const bundle = data.links && data.links.length > 1 ? data.links : null;
  const worst = bundle
    ? Math.max(
        -1,
        ...bundle.map((l) =>
          l.caseRef?.kind === "branch" ? (results?.branches.get(l.caseRef.id)?.loading_pct ?? -1) : -1,
        ),
      )
    : null;
  const loading = worst != null && worst >= 0 ? loadingColor(worst) : null;
  const stroke = selected ? ink.selected : data.outOfService ? ink.off : loading ? ink.alert(loading) : ink.line;
  const far = bundle ? [...new Set(bundle.map((l) => l.bus))] : [data.bus];
  const title = bundle
    ? `${bundle.length} branches leave the drawing here:\n${bundle.map((l) => `• ${describe(l, results)}`).join("\n")}\n\nDouble-click to bring ${far.length === 1 ? "that bus" : `all ${far.length} buses`} in, or right-click to pick one`
    : `Double-click to bring ${data.busName || `Bus ${data.bus}`} into the drawing`;
  const dash = data.outOfService ? "3 2" : undefined;
  return (
    <SymbolFrame
      w={W}
      h={H}
      rotation={data.rotation}
      labelOffset={data.labelOffset}
      terminals={TERMINALS}
      title={title}
      graphic={
        <>
          <line x1={CX} y1={0} x2={CX} y2={14} stroke={stroke} strokeWidth={STROKE} />
          {bundle && (
            // A second flag behind the first: several branches leave here.
            <polygon
              points={flag(5)}
              transform="translate(0,-4)"
              fill="white"
              stroke={stroke}
              strokeWidth={STROKE}
              strokeLinejoin="round"
              strokeDasharray={dash}
            />
          )}
          {/* Off-page connector: a flag pointing away from the bus. */}
          <polygon
            points={flag(0)}
            fill="white"
            stroke={stroke}
            strokeWidth={STROKE}
            strokeLinejoin="round"
            strokeDasharray={dash}
          />
          {bundle && (
            <text
              x={CX}
              y={27}
              fontSize={8}
              fontWeight={600}
              textAnchor="middle"
              fill={stroke}
              data-testid="off-page-count"
            >
              {bundle.length}
            </text>
          )}
        </>
      }
      labelTop={17}
      label={
        <text
          y={26}
          fontSize={9.5}
          fill={loading ? ink.alert(loading) : ink.text}
          fontWeight={loading ? 600 : undefined}
        >
          {data.label}
        </text>
      }
    />
  );
}
