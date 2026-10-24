export function getVoltageColor(kv: number): string {
  if (kv >= 765) return "#dc2626";
  if (kv >= 345) return "#ea580c";
  if (kv >= 115) return "#ca8a04";
  if (kv >= 34.5) return "#16a34a";
  return "#2563eb";
}
