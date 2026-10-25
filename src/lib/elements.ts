// Shared element ids, the same in every module (see stannum-core's
// elements.rs): bus:101, line:101:102:1, transformer:101:102:103:1,
// generator:101:1, load:101:1, dcline:301:3021:DC1. Branch bus numbers are sorted, and the last
// part is the circuit or machine id.

export type ElementKind = "bus" | "line" | "transformer" | "switch" | "generator" | "load" | "dcline";

export interface ParsedElement {
  kind: ElementKind;
  buses: number[];
  /** Circuit or machine id; empty for buses. */
  circuit: string;
}

const KINDS = new Set<ElementKind>(["bus", "line", "transformer", "switch", "generator", "load", "dcline"]);

export function busElement(bus: number): string {
  return `bus:${bus}`;
}

export function parseElement(id: string): ParsedElement | null {
  const parts = id.split(":");
  const kind = parts[0] as ElementKind;
  if (!KINDS.has(kind)) return null;
  if (kind === "bus") {
    const bus = Number(parts[1]);
    return parts.length === 2 && Number.isInteger(bus) ? { kind, buses: [bus], circuit: "" } : null;
  }
  // The circuit may itself contain ":", so only the leading numbers are buses.
  const count = kind === "generator" || kind === "load" ? 1 : parts.length >= 5 && kind === "transformer" && /^\d+$/.test(parts[3]) ? 3 : 2;
  const buses = parts.slice(1, 1 + count).map(Number);
  if (buses.some((b) => !Number.isInteger(b))) return null;
  const circuit = parts.slice(1 + count).join(":");
  return circuit ? { kind, buses, circuit } : null;
}

const NAMES: Record<ElementKind, string> = {
  bus: "Bus",
  line: "Line",
  transformer: "Transformer",
  switch: "Switch",
  generator: "Generator",
  load: "Load",
  dcline: "DC line",
};

/** A readable name, such as "Line 101–102 circuit 2". */
export function elementLabel(id: string): string {
  const e = parseElement(id);
  if (!e) return id;
  if (e.kind === "bus") return `Bus ${e.buses[0]}`;
  if (e.kind === "generator" || e.kind === "load") return `${NAMES[e.kind]} ${e.circuit} at bus ${e.buses[0]}`;
  if (e.kind === "dcline") return `DC line ${e.circuit} (${e.buses.join("–")})`;
  return `${NAMES[e.kind]} ${e.buses.join("–")} circuit ${e.circuit}`;
}

/** The bus an element sits at: the bus itself, or a generator's or load's bus. */
export function elementBus(id: string): number | null {
  const e = parseElement(id);
  return e && (e.kind === "bus" || e.kind === "generator" || e.kind === "load") ? e.buses[0] : null;
}

/** The RAW section that holds an element, as Case diff names it. */
export function elementSection(id: string): string | null {
  const kind = parseElement(id)?.kind;
  if (!kind) return null;
  return {
    bus: "BUS",
    line: "BRANCH",
    transformer: "TRANSFORMER",
    switch: "SYSTEM SWITCHING DEVICE",
    generator: "GENERATOR",
    load: "LOAD",
    dcline: "TWO-TERMINAL DC",
  }[kind];
}
