// The application log: lines from the Rust core and its solvers, plus the
// app's own record of each operation it starts (see store/logStore.ts).

export type LogLevel = "error" | "warn" | "info" | "debug";

export interface LogEntry {
  /** Unique within the session. */
  key: number;
  /** Milliseconds since the Unix epoch. */
  time: number;
  level: LogLevel;
  /** Who wrote it: "app" for the window, or "core", "rs-power", "nodal". */
  source: string;
  message: string;
}

/** A line as the Rust core sends it (stannum-core's logbook::Entry). */
export interface CoreEntry {
  seq: number;
  time: number;
  level: LogLevel;
  source: string;
  message: string;
}

export type LevelFilter = "all" | "warn" | "error";

export function shown(entry: LogEntry, filter: LevelFilter, details: boolean, query: string): boolean {
  if (!details && entry.level === "debug") return false;
  if (filter === "warn" && entry.level !== "warn") return false;
  if (filter === "error" && entry.level !== "error") return false;
  if (query) {
    const q = query.toLowerCase();
    return entry.message.toLowerCase().includes(q) || entry.source.toLowerCase().includes(q);
  }
  return true;
}

/** "14:03:07.215" in local time. */
export function clock(time: number): string {
  const d = new Date(time);
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}.${String(d.getMilliseconds()).padStart(3, "0")}`;
}

/** "850 ms", "3.4 s", "2 min 05 s". */
export function duration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`;
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")} s`;
}

const LEVEL_TAG: Record<LogLevel, string> = { error: "ERROR", warn: "WARN ", info: "INFO ", debug: "DEBUG" };

/** Plain text for copying or saving, one line per entry. */
export function asText(entries: LogEntry[]): string {
  return entries.map((e) => `${clock(e.time)} ${LEVEL_TAG[e.level]} [${e.source}] ${e.message}`).join("\n") + "\n";
}

export type Segment = { text: string } | { text: string; element: string };

// "bus 101", "Bus 101", "buses 101 and 102" are linked bus by bus.
// A number followed by a decimal point is a value, not a bus.
const BUS = /\b(bus(?:es)?)\s+(\d+(?!\d|\.\d)(?:(?:\s*,\s*|\s+and\s+|\s*[-–]\s*)\d+(?!\d|\.\d))*)/gi;

/** Split a message so bus numbers in it can be shown as links. */
export function linkify(message: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of message.matchAll(BUS)) {
    const start = m.index ?? 0;
    // Counts such as "118 buses" come after the number, so they never match.
    let at = start + m[1].length;
    out.push({ text: message.slice(last, at) });
    const numbers = m[2];
    const tail = message.slice(at, start + m[0].length);
    const lead = tail.length - tail.trimStart().length;
    out.push({ text: tail.slice(0, lead) });
    at += lead;
    let cursor = 0;
    for (const n of numbers.matchAll(/\d+/g)) {
      const i = n.index ?? 0;
      if (i > cursor) out.push({ text: numbers.slice(cursor, i) });
      out.push({ text: n[0], element: `bus:${n[0]}` });
      cursor = i + n[0].length;
    }
    last = start + m[0].length;
  }
  out.push({ text: message.slice(last) });
  return out.filter((s) => s.text !== "");
}
