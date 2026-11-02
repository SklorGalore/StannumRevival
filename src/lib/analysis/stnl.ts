/**
 * Writing `.stnl` lists text from the contingency setup: its subsystems, and
 * the contingency sets and monitors a subsystem choice stands for.
 */

/** A subsystem built in the setup, as a `.stnl` subsystem record. */
export interface SetupSubsystem {
  /** The record id: letters, digits and underscores. */
  id: string;
  name: string;
  areas: number[];
  zones: number[];
  owners: number[];
  buses: number[];
  kvMin: number | null;
  kvMax: number | null;
}

/** The header every lists file starts with. */
export const LISTS_HEADER = "stannum-lists 1.0";

/** An id the format accepts, made from any name. */
export function subsystemId(name: string, taken: Iterable<string> = []): string {
  const base =
    name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_]+/g, "_")
      .replace(/^_+|_+$/g, "") || "subsystem";
  const start = /^[a-z_]/.test(base) ? base : `s_${base}`;
  const used = new Set(taken);
  let id = start;
  for (let n = 2; used.has(id); n++) id = `${start}_${n}`;
  return id;
}

function quote(s: string): string {
  return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

function list(v: number[]): string {
  return `[${v.join(", ")}]`;
}

/** One subsystem record line. */
export function subsystemLine(s: SetupSubsystem): string {
  const parts = [`subsystem ${s.id}`];
  if (s.name && s.name !== s.id) parts.push(`name=${quote(s.name)}`);
  if (s.areas.length) parts.push(`areas=${list(s.areas)}`);
  if (s.zones.length) parts.push(`zones=${list(s.zones)}`);
  if (s.owners.length) parts.push(`owners=${list(s.owners)}`);
  if (s.buses.length) parts.push(`buses=${list(s.buses)}`);
  if (s.kvMin !== null) parts.push(`kv_min=${s.kvMin}`);
  if (s.kvMax !== null) parts.push(`kv_max=${s.kvMax}`);
  return parts.join(" ");
}

/** A whole lists file of subsystems, to save. */
export function subsystemsText(subsystems: SetupSubsystem[]): string {
  return [LISTS_HEADER, ...subsystems.map(subsystemLine)].join("\n") + "\n";
}

/** What to take out within a subsystem. */
export interface SubsystemOutages {
  subsystem: string;
  branches: boolean;
  generators: boolean;
  buses: boolean;
  /** N-2: branches two at a time (generators stay one at a time). */
  pairs: boolean;
}

/** Lists text whose contingency sets take out the chosen kinds within one subsystem. */
export function outagesText(subsystems: SetupSubsystem[], o: SubsystemOutages): string {
  const sets: string[] = [];
  if (o.branches)
    sets.push(
      o.pairs
        ? `contingency_set n2_branches name="N-2 branches" kind=n_2_branches subsystem=${o.subsystem}`
        : `contingency_set n1_branches name="N-1 branches" kind=n_1_branches subsystem=${o.subsystem}`
    );
  if (o.generators) sets.push(`contingency_set n1_generators name="N-1 generators" kind=n_1_generators subsystem=${o.subsystem}`);
  if (o.buses) sets.push(`contingency_set bus_outages name="Bus outages" kind=bus_outages subsystem=${o.subsystem}`);
  return [LISTS_HEADER, ...subsystems.map(subsystemLine), ...sets].join("\n") + "\n";
}

/** Lists text whose monitors check one subsystem's branches (at the run's limit and ratings) and, optionally, its bus voltages. */
export function monitorsText(subsystems: SetupSubsystem[], subsystem: string, limitPct: number, voltages: boolean): string {
  const lines = [LISTS_HEADER, ...subsystems.map(subsystemLine), `monitor flows subsystem=${subsystem} limit_pct=${limitPct}`];
  if (voltages) lines.push(`monitor voltages kind=voltage subsystem=${subsystem}`);
  return lines.join("\n") + "\n";
}

/** A short description of a subsystem's members, for lists and summaries. */
export function describeSubsystem(s: SetupSubsystem): string {
  const parts: string[] = [];
  if (s.areas.length) parts.push(`area${s.areas.length > 1 ? "s" : ""} ${s.areas.join(", ")}`);
  if (s.zones.length) parts.push(`zone${s.zones.length > 1 ? "s" : ""} ${s.zones.join(", ")}`);
  if (s.owners.length) parts.push(`owner${s.owners.length > 1 ? "s" : ""} ${s.owners.join(", ")}`);
  if (s.buses.length) parts.push(`${s.buses.length} bus${s.buses.length > 1 ? "es" : ""}`);
  let out = parts.length ? parts.join(" + ") : "every bus";
  if (s.kvMin !== null && s.kvMax !== null) out += `, ${s.kvMin}–${s.kvMax} kV`;
  else if (s.kvMin !== null) out += `, ${s.kvMin} kV and up`;
  else if (s.kvMax !== null) out += `, up to ${s.kvMax} kV`;
  return out;
}
