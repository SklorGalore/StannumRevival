// Element search for the command palette (Ctrl+K).
import { busElement } from "./elements";
import type { CaseNetwork } from "./analysis/types";

export interface PaletteElement {
  element: string;
  title: string;
  detail: string;
  /** Lower-case text the query is matched against. */
  haystack: string;
  /** Bus numbers, for exact-number matches. */
  numbers: number[];
}

/** Everything in the case that can be searched for. */
export function paletteElements(net: CaseNetwork): PaletteElement[] {
  const names = new Map(net.buses.map((b) => [b.id, b.name.trim()]));
  const bus = (n: number) => (names.get(n) ? `${n} ${names.get(n)}` : String(n));
  const out: PaletteElement[] = [];
  for (const b of net.buses) {
    if (b.star) continue;
    const name = b.name.trim();
    out.push({
      element: busElement(b.id),
      title: name ? `Bus ${b.id} · ${name}` : `Bus ${b.id}`,
      detail: `${b.kind} bus · ${b.base_kv} kV · area ${b.area}`,
      haystack: `bus ${b.id} ${name}`.toLowerCase(),
      numbers: [b.id],
    });
  }
  for (const br of net.branches) {
    // A three-winding transformer is listed once, by its first winding.
    if (br.winding > 1) continue;
    const kind = br.transformer ? "Transformer" : "Line";
    out.push({
      element: br.element,
      title: `${kind} ${br.from}–${br.to} circuit ${br.circuit}`,
      detail: `${bus(br.from)} to ${bus(br.to)}${br.online ? "" : " · offline"}`,
      haystack: `${kind} ${br.from} ${br.to} ${br.from}-${br.to} ckt ${br.circuit} ${names.get(br.from) ?? ""} ${names.get(br.to) ?? ""}`.toLowerCase(),
      numbers: [br.from, br.to],
    });
  }
  for (const g of net.generators) {
    const id = g.element.split(":").slice(2).join(":");
    out.push({
      element: g.element,
      title: `Generator ${id} at bus ${g.bus}`,
      detail: `${bus(g.bus)} · Pmax ${g.pmax} MW${g.online ? "" : " · offline"}`,
      haystack: `generator gen ${g.bus} ${id} ${names.get(g.bus) ?? ""}`.toLowerCase(),
      numbers: [g.bus],
    });
  }
  for (const d of net.dc_lines) {
    out.push({
      element: d.element,
      title: `DC line ${d.name.trim()}`,
      detail: `${bus(d.from)} to ${bus(d.to)} · ${d.mw} MW`,
      haystack: `dc line hvdc ${d.name} ${d.from} ${d.to}`.toLowerCase(),
      numbers: [d.from, d.to],
    });
  }
  return out;
}

/**
 * Rank elements against a query: every word must appear. A number that is
 * a bus number exactly ranks the bus first, then what connects to it.
 */
export function searchElements(items: PaletteElement[], query: string, limit = 40): PaletteElement[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const exact = words.length === 1 && /^\d+$/.test(words[0]) ? Number(words[0]) : null;
  const scored: { item: PaletteElement; score: number; i: number }[] = [];
  items.forEach((item, i) => {
    if (!words.every((w) => item.haystack.includes(w))) return;
    let score = 3;
    if (exact !== null && item.numbers.includes(exact)) score = item.element === busElement(exact) ? 0 : 1;
    else if (words.some((w) => item.title.toLowerCase().startsWith(w) || item.haystack.split(" ").includes(w))) score = 2;
    scored.push({ item, score, i });
  });
  scored.sort((a, b) => a.score - b.score || a.i - b.i);
  return scored.slice(0, limit).map((s) => s.item);
}

