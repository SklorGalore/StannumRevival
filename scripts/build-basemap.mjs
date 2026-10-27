// Builds the map's offline outline file, public/basemap/outlines.json, from
// Natural Earth 1:50m data (public domain, https://www.naturalearthdata.com/).
//
//   node scripts/build-basemap.mjs [folder with the four .geojson files]
//
// Without a folder it downloads them from the natural-earth-vector repository.
// Coordinates are rounded to 0.01° (about 1 km, finer than 1:50m data holds)
// after a 0.02° simplification, and properties are dropped, which takes
// ~4 MB of GeoJSON to well under 1 MB. Islands and lakes under ~15 km
// across are dropped.
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const SOURCE = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/";
const LAYERS = {
  land: "ne_50m_land",
  lakes: "ne_50m_lakes",
  countries: "ne_50m_admin_0_boundary_lines_land",
  states: "ne_50m_admin_1_states_provinces_lines",
};
const DIGITS = 2;
// Douglas-Peucker tolerance in degrees: about 2 km, below what the map shows
// at the zoom where outlines matter (a state or a region).
const TOLERANCE = 0.02;

async function load(name, folder) {
  if (folder) return JSON.parse(await readFile(join(folder, `${name}.geojson`), "utf8"));
  const res = await fetch(`${SOURCE}${name}.geojson`);
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}`);
  return res.json();
}

const round = (v) => Math.round(v * 10 ** DIGITS) / 10 ** DIGITS;

function simplify(points) {
  if (points.length < 3) return points;
  const keep = new Uint8Array(points.length);
  keep[0] = keep[points.length - 1] = 1;
  const stack = [[0, points.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop();
    const [ax, ay] = points[a];
    const [bx, by] = points[b];
    const dx = bx - ax;
    const dy = by - ay;
    const len = Math.hypot(dx, dy);
    let worst = -1;
    let at = -1;
    for (let i = a + 1; i < b; i++) {
      const [px, py] = points[i];
      const d = len === 0 ? Math.hypot(px - ax, py - ay) : Math.abs(dy * px - dx * py + bx * ay - by * ax) / len;
      if (d > worst) [worst, at] = [d, i];
    }
    if (worst > TOLERANCE) {
      keep[at] = 1;
      stack.push([a, at], [at, b]);
    }
  }
  return points.filter((_, i) => keep[i]);
}

/** Simplify and round a ring or line, dropping points that rounding made repeat. */
function line(points, closed) {
  const out = [];
  for (const [x, y] of simplify(points)) {
    const p = [round(x), round(y)];
    const last = out[out.length - 1];
    if (!last || last[0] !== p[0] || last[1] !== p[1]) out.push(p);
  }
  if (closed && out.length < 4) return null;
  if (!closed && out.length < 2) return null;
  return out;
}

// Islands and lakes smaller than this (square degrees, about 15 km across)
// are left out: they're specks at the zooms the map is read at.
const MIN_AREA = 0.02;
const area = (ring) => Math.abs(ring.reduce((a, [x, y], i) => {
  const [nx, ny] = ring[(i + 1) % ring.length];
  return a + x * ny - nx * y;
}, 0)) / 2;
const bigEnough = (rings) => rings[0] && area(rings[0]) >= MIN_AREA;

function geometry(g) {
  switch (g.type) {
    case "Polygon": {
      const rings = g.coordinates.map((r) => line(r, true));
      return bigEnough(rings) ? { type: "Polygon", coordinates: rings.filter(Boolean) } : null;
    }
    case "MultiPolygon": {
      const polys = g.coordinates.map((p) => p.map((r) => line(r, true))).filter(bigEnough).map((p) => p.filter(Boolean));
      return polys.length ? { type: "MultiPolygon", coordinates: polys } : null;
    }
    case "LineString": {
      const l = line(g.coordinates, false);
      return l ? { type: "LineString", coordinates: l } : null;
    }
    case "MultiLineString": {
      const ls = g.coordinates.map((l) => line(l, false)).filter(Boolean);
      return ls.length ? { type: "MultiLineString", coordinates: ls } : null;
    }
    default:
      return null;
  }
}

const folder = process.argv[2];
const out = { source: "Natural Earth 1:50m (public domain)", layers: {} };
for (const [key, name] of Object.entries(LAYERS)) {
  const fc = await load(name, folder);
  const features = fc.features
    .map((f) => geometry(f.geometry))
    .filter(Boolean)
    .map((g) => ({ type: "Feature", properties: {}, geometry: g }));
  out.layers[key] = { type: "FeatureCollection", features };
}
await mkdir("public/basemap", { recursive: true });
const text = JSON.stringify(out);
await writeFile("public/basemap/outlines.json", text);
console.log(`public/basemap/outlines.json: ${(text.length / 1024).toFixed(0)} KB`);
