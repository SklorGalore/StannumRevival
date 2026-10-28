// The map's picture of the case: buses gathered into sites (substations) at
// their coordinates, and branches as straight lines between sites.
import type { BusCoordinates, CaseNetwork } from "../analysis/types";
import { busElement } from "../elements";

export interface Site {
  /** Index in MapModel.sites; also the map feature id. */
  index: number;
  lon: number;
  lat: number;
  /** Buses here, highest voltage first. */
  buses: number[];
  /** The substation the coordinates file or the case names, if any. */
  substation: string | null;
  /** Highest base kV here. */
  kv: number;
  /** Every bus here was placed from its neighbours, not by the file or the case. */
  estimated: boolean;
}

export interface MapLine {
  index: number;
  element: string;
  /** 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise. */
  winding: number;
  from: number;
  to: number;
  /** Site indexes at each end (never equal: lines inside a site aren't drawn). */
  a: number;
  b: number;
  kv: number;
  online: boolean;
  dc: boolean;
}

/** A branch with both ends at one site (a substation transformer): not drawn as a line. */
export interface InnerBranch {
  element: string;
  winding: number;
  site: number;
}

export interface MapModel {
  sites: Site[];
  lines: MapLine[];
  /** Branches inside a site, so layers that mark them can mark the site. */
  inner: InnerBranch[];
  /** Bus number to site index, for every bus with a place. */
  busSite: Map<number, number>;
  /** Buses (other than star points) with no place. */
  unplaced: number;
  /** [west, south, east, north], or null when nothing is placed. */
  bounds: [number, number, number, number] | null;
}

const key = (lat: number, lon: number) => `${lat},${lon}`;

export function buildMap(net: CaseNetwork, coords: BusCoordinates): MapModel {
  const kv = new Map(net.buses.map((b) => [b.id, b.base_kv]));
  const place = new Map<number, { lat: number; lon: number; file: boolean; substation: string | null }>();
  coords.bus.forEach((bus, i) => {
    const lat = coords.latitude[i];
    const lon = coords.longitude[i];
    if (lat === null || lon === null) return;
    place.set(bus, { lat, lon, file: coords.source[i] !== "estimated", substation: coords.substation[i] });
  });
  // A three-winding transformer's star point sits between its windings' buses.
  for (const star of net.buses.filter((b) => b.star && !place.has(b.id))) {
    const ends = net.branches
      .filter((br) => br.from === star.id || br.to === star.id)
      .map((br) => place.get(br.from === star.id ? br.to : br.from))
      .filter((p) => !!p);
    if (!ends.length) continue;
    place.set(star.id, {
      lat: ends.reduce((s, p) => s + p.lat, 0) / ends.length,
      lon: ends.reduce((s, p) => s + p.lon, 0) / ends.length,
      file: false,
      substation: null,
    });
  }

  // Sites: a named substation, else one point. Two substations at one point
  // stay apart. Buses the file places come first, so an unnamed bus at the
  // same point as a placed one (an estimated transformer low side, usually)
  // joins that site.
  const sites: Site[] = [];
  const byName = new Map<string, number>();
  const byPoint = new Map<string, number>();
  const busSite = new Map<number, number>();
  const entries = [...place.entries()].sort((x, y) => Number(y[1].file) - Number(x[1].file));
  for (const [bus, p] of entries) {
    const point = key(p.lat, p.lon);
    let i = p.substation !== null ? byName.get(p.substation) : byPoint.get(point);
    if (i === undefined) {
      i = sites.length;
      sites.push({ index: i, lon: p.lon, lat: p.lat, buses: [], substation: p.substation, kv: 0, estimated: !p.file });
      if (p.substation !== null) byName.set(p.substation, i);
      if (!byPoint.has(point)) byPoint.set(point, i);
    }
    const site = sites[i];
    site.buses.push(bus);
    site.kv = Math.max(site.kv, kv.get(bus) ?? 0);
    if (p.file) site.estimated = false;
    busSite.set(bus, i);
  }
  for (const s of sites) s.buses.sort((x, y) => (kv.get(y) ?? 0) - (kv.get(x) ?? 0) || x - y);

  const lines: MapLine[] = [];
  const inner: InnerBranch[] = [];
  const add = (element: string, winding: number, from: number, to: number, online: boolean, dc: boolean) => {
    const a = busSite.get(from);
    const b = busSite.get(to);
    if (a === undefined || b === undefined) return;
    if (a === b) {
      inner.push({ element, winding, site: a });
      return;
    }
    const v = Math.max(kv.get(from) ?? 0, kv.get(to) ?? 0);
    lines.push({ index: lines.length, element, winding, from, to, a, b, kv: v, online, dc });
  };
  for (const br of net.branches) add(br.element, br.winding, br.from, br.to, br.online, false);
  for (const dc of net.dc_lines) add(dc.element, 0, dc.from, dc.to, dc.online, true);

  let bounds: MapModel["bounds"] = null;
  for (const s of sites) {
    if (!bounds) bounds = [s.lon, s.lat, s.lon, s.lat];
    else bounds = [Math.min(bounds[0], s.lon), Math.min(bounds[1], s.lat), Math.max(bounds[2], s.lon), Math.max(bounds[3], s.lat)];
  }
  const unplaced = net.buses.filter((b) => !b.star && !place.has(b.id)).length;
  return { sites, lines, inner, busSite, unplaced, bounds };
}

/** Buses within `depth` branches of `bus` (in service or not: the map shows geography). */
export function neighbourhood(net: CaseNetwork, bus: number, depth: number): Set<number> {
  const next = new Map<number, number[]>();
  const link = (a: number, b: number) => {
    if (!next.has(a)) next.set(a, []);
    next.get(a)!.push(b);
  };
  for (const br of net.branches) {
    link(br.from, br.to);
    link(br.to, br.from);
  }
  for (const dc of net.dc_lines) {
    link(dc.from, dc.to);
    link(dc.to, dc.from);
  }
  // Star points don't count as a hop: a three-winding transformer joins its
  // buses in one step, as in the other styles.
  const star = new Set(net.buses.filter((b) => b.star).map((b) => b.id));
  const seen = new Set([bus]);
  let ring = [bus];
  for (let d = 0; d < depth && ring.length; d++) {
    const out: number[] = [];
    const visit = (from: number) => {
      for (const n of next.get(from) ?? []) {
        if (seen.has(n)) continue;
        seen.add(n);
        if (star.has(n)) visit(n);
        else out.push(n);
      }
    };
    ring.forEach(visit);
    ring = out;
  }
  return seen;
}

/** The element a site stands for when picked: its highest-voltage bus. */
export function siteElement(site: Site): string {
  return busElement(site.buses[0]);
}

/** The site name to show: the substation, else its first bus's name and number. */
export function siteLabel(site: Site, net: CaseNetwork): string {
  if (site.substation) return site.substation;
  const bus = net.buses.find((b) => b.id === site.buses[0]);
  const name = bus?.name.trim();
  return name ? `${name} (bus ${site.buses[0]})` : `Bus ${site.buses[0]}`;
}
