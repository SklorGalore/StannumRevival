import {
  BaseEdge,
  EdgeLabelRenderer,
  getStraightPath,
  type EdgeProps,
} from "@xyflow/react";
import type { ConductorData } from "../../store/types";
import { getVoltageColor } from "../nodes/shared/voltageColor";

export function ConductorEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  data,
  selected,
  label,
}: EdgeProps & { data?: ConductorData }) {
  const [edgePath, labelX, labelY] = getStraightPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
  });

  const kv = data?.voltageKV ?? 0;
  const color = kv > 0 ? getVoltageColor(kv) : "#475569";
  const strokeW = data?.conductorType === "transmission" ? 2.5 : 1.5;

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        style={{
          stroke: selected ? "#3b82f6" : color,
          strokeWidth: strokeW,
        }}
      />
      {label && (
        <EdgeLabelRenderer>
          <div
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px,${labelY}px)`,
              fontSize: 9,
              color: "#475569",
              background: "white",
              padding: "1px 3px",
              borderRadius: 2,
              pointerEvents: "all",
            }}
            className="nodrag nopan"
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}
