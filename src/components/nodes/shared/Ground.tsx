import { STROKE } from "../../../lib/diagram/ink";

/** Earth symbol hanging below (x, y). */
export function Ground({ x, y, color }: { x: number; y: number; color: string }) {
  return (
    <path
      d={`M${x - 8},${y} h16 M${x - 5},${y + 4} h10 M${x - 2},${y + 8} h4`}
      stroke={color}
      strokeWidth={STROKE}
      fill="none"
    />
  );
}
