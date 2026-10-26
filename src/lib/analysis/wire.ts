// The solve report's bus, generator and branch tables arrive as columns: one
// array per field, which keeps a large case's report to about a third of the
// size of one object per row (see `power::wire` in stannum-core). The rest of
// the app works with rows, so turn them back here.
import type { BranchRow, BusRow, GeneratorRow, SolveReport } from "./types";

/** A table sent as one array per field, plus the row count. */
export type Columns<T> = { count: number } & { [K in keyof T]: T[K][] };

/** The report as it comes over the wire. */
export type WireSolveReport = Omit<
  SolveReport,
  "buses" | "generators" | "branches"
> & {
  buses: Columns<BusRow>;
  generators: Columns<GeneratorRow>;
  branches: Columns<BranchRow>;
};

/** Rows from columns. Already-row tables (older callers, tests) pass through. */
export function fromColumns<T>(table: Columns<T> | T[]): T[] {
  if (Array.isArray(table)) return table;
  const { count, ...columns } = table as { count: number } & Record<
    string,
    unknown[]
  >;
  const keys = Object.keys(columns);
  const arrays = keys.map((k) => columns[k]);
  const rows = new Array<T>(count);
  for (let i = 0; i < count; i++) {
    const row: Record<string, unknown> = {};
    for (let k = 0; k < keys.length; k++) row[keys[k]] = arrays[k][i];
    rows[i] = row as T;
  }
  return rows;
}

export function decodeSolveReport(
  wire: WireSolveReport | SolveReport,
): SolveReport {
  return {
    ...wire,
    buses: fromColumns<BusRow>(wire.buses),
    generators: fromColumns<GeneratorRow>(wire.generators),
    branches: fromColumns<BranchRow>(wire.branches),
  };
}
