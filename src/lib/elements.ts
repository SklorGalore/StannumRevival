// Shared element ids, the same in every module. The kinds, their id formats
// and where each comes from are generated from stannum-core's
// elements::KINDS: bus:101, line:101:102:1, transformer:101:102:103:1,
// generator:101:1, load:101:1, shunt:101:1, switched-shunt:101:1,
// machine:101:1, dcline:301:3021:DC1, facts:SVC_1. Branch bus numbers are
// sorted, and the last part is the circuit, machine id or name.
import { ELEMENT_KINDS } from "./analysis/commands.gen";

export type ElementKind = (typeof ELEMENT_KINDS)[number]["kind"];
export type ElementKindInfo = (typeof ELEMENT_KINDS)[number];

export interface ParsedElement {
  kind: ElementKind;
  buses: number[];
  /** Circuit, machine id or name; empty for buses. */
  circuit: string;
}

const KINDS = new Map<string, ElementKindInfo>(ELEMENT_KINDS.map((k) => [k.kind, k]));

/** What the core says about a kind of element, or undefined for an unknown id. */
export function elementKind(id: string): ElementKindInfo | undefined {
  return KINDS.get(id.split(":")[0]);
}

export function busElement(bus: number): string {
  return `bus:${bus}`;
}

export function parseElement(id: string): ParsedElement | null {
  const parts = id.split(":");
  const info = KINDS.get(parts[0]);
  if (!info) return null;
  const kind = info.kind;
  if (kind === "bus") {
    const bus = Number(parts[1]);
    return parts.length === 2 && Number.isInteger(bus) ? { kind, buses: [bus], circuit: "" } : null;
  }
  // The circuit may itself contain ":", so only the leading numbers are buses.
  // A three-winding transformer has a third bus.
  const count = kind === "transformer" && parts.length >= 5 && /^\d+$/.test(parts[3]) ? 3 : info.buses;
  const buses = parts.slice(1, 1 + count).map(Number);
  if (buses.some((b) => !Number.isInteger(b))) return null;
  const circuit = parts.slice(1 + count).join(":");
  return circuit ? { kind, buses, circuit } : null;
}

/** A readable name, such as "Line 101–102 circuit 2". */
export function elementLabel(id: string): string {
  // Elements out together, such as an N-2 pair.
  if (id.includes("&")) return id.split("&").map(elementLabel).join(" and ");
  const e = parseElement(id);
  if (!e) return id;
  const label = KINDS.get(e.kind)!.label;
  if (e.kind === "bus") return `Bus ${e.buses[0]}`;
  if (e.buses.length === 0) return `${label} ${e.circuit}`;
  if (e.buses.length === 1) return `${label} ${e.circuit} at bus ${e.buses[0]}`;
  if (e.kind === "dcline") return `DC line ${e.circuit} (${e.buses.join("–")})`;
  return `${label} ${e.buses.join("–")} circuit ${e.circuit}`;
}

/** The kind's name, such as "Switched shunt". */
export function kindLabel(kind: string): string {
  return KINDS.get(kind)?.label ?? kind;
}

/** The bus an element sits at: the bus itself, or the bus of a generator,
 *  load, shunt or other one-bus element. */
export function elementBus(id: string): number | null {
  const e = parseElement(id);
  return e && e.buses.length === 1 ? e.buses[0] : null;
}

/** The RAW sections that can hold an element, as Records and Case diff name them. */
export function elementSections(id: string): readonly string[] {
  return elementKind(id)?.raw_sections ?? [];
}

/** The first RAW section that can hold an element. */
export function elementSection(id: string): string | null {
  return elementSections(id)[0] ?? null;
}

/** The MATPOWER table that holds an element, if MATPOWER has one. */
export function elementTable(id: string): string | null {
  return elementKind(id)?.matpower_table ?? null;
}
