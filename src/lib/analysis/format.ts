/** Fixed decimals for tables, with an em dash for missing values. */
export function fixed(value: number | null | undefined, digits = 2): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  if (!Number.isFinite(value)) return value > 0 ? "∞" : "−∞";
  return value.toFixed(digits);
}

/**
 * Significant digits (six by default). Values below 10^minExp or at or above
 * 10^6 use scientific notation.
 */
export function sig(value: number, digits = 6, minExp = -3): string {
  if (value === 0) return "0";
  if (!Number.isFinite(value)) return String(value);
  const exp = Math.floor(Math.log10(Math.abs(value)));
  if (exp < minExp || exp >= 6) return value.toExponential(digits - 1);
  return value.toFixed(Math.max(0, digits - 1 - exp));
}

export function count(n: number): string {
  return n.toLocaleString("en-US");
}

/** CSV with spreadsheet-formula neutralization (as in nodal). */
export function toCsv(rows: Record<string, unknown>[]): string {
  if (!rows.length) return "";
  const columns = Object.keys(rows[0]);
  const cell = (value: unknown) => {
    let text =
      value == null ? "" : Array.isArray(value) ? value.join(" → ") : String(value);
    if (typeof value === "string" && /^\s*[=+@-]/.test(text)) text = "'" + text;
    return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
  };
  return (
    [columns.map(cell).join(","), ...rows.map((r) => columns.map((k) => cell(r[k])).join(","))].join(
      "\r\n"
    ) + "\r\n"
  );
}
