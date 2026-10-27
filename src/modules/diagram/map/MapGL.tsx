// The Map style's canvas: MapLibre GL drawing the case's sites and lines over
// the offline Natural Earth outlines. Loaded on first use (see LazyMapGL.tsx);
// MapLibre and its worker are about 1.5 MB.
import { useEffect, useMemo, useRef } from "react";
import { Map as GLMap, Popup, setWorkerUrl, type GeoJSONSource, type MapLayerMouseEvent } from "maplibre-gl";
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?url";
import "maplibre-gl/dist/maplibre-gl.css";
import type { FeatureCollection } from "geojson";
import type { CaseNetwork } from "../../../lib/analysis/types";
import { elementLabel } from "../../../lib/elements";
import { centreBus } from "../../../ui/DrawAround";
import { siteElement, siteLabel, type MapModel } from "../../../lib/map/model";
import type { MapPalette } from "../../../lib/map/palette";
import { ARROW_MIN_MW, arrowSize, type MapOverlay } from "../../../lib/map/overlay";

// The worker's URL is set explicitly: MapLibre only works it out by itself on
// http(s) pages, and the desktop app serves its pages from its own scheme.
setWorkerUrl(workerUrl);

interface Outlines {
  layers: Record<"land" | "lakes" | "countries" | "states", FeatureCollection>;
}

let outlines: Promise<Outlines> | null = null;
function loadOutlines(): Promise<Outlines> {
  outlines ??= fetch(`${import.meta.env.BASE_URL}basemap/outlines.json`).then((r) => {
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    return r.json() as Promise<Outlines>;
  });
  outlines.catch(() => (outlines = null));
  return outlines;
}

const EMPTY: FeatureCollection = { type: "FeatureCollection", features: [] };

/** The flow arrow: a chevron pointing along the line, in ink with a land-coloured edge. */
function arrowImage(fill: string, edge: string): ImageData | null {
  const size = 32;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.beginPath();
  ctx.moveTo(8, 6);
  ctx.lineTo(26, 16);
  ctx.lineTo(8, 26);
  ctx.lineTo(12, 16);
  ctx.closePath();
  ctx.lineJoin = "round";
  ctx.lineWidth = 4;
  ctx.strokeStyle = edge;
  ctx.stroke();
  ctx.fillStyle = fill;
  ctx.fill();
  return ctx.getImageData(0, 0, size, size);
}

function setArrow(map: GLMap, palette: MapPalette) {
  const image = arrowImage(palette.ink.text, palette.land);
  if (!image) return;
  if (map.hasImage("flow-arrow")) map.updateImage("flow-arrow", image);
  else map.addImage("flow-arrow", image, { pixelRatio: 2 });
}

/** Line width in pixels at zoom 6, by voltage. */
function lineWidth(kv: number): number {
  return kv >= 600 ? 3.4 : kv >= 300 ? 2.8 : kv >= 200 ? 2.3 : kv >= 100 ? 1.8 : 1.3;
}
/** Site radius in pixels at zoom 6, by voltage. */
function siteRadius(kv: number): number {
  return kv >= 300 ? 4.5 : kv >= 100 ? 3.6 : 2.8;
}
/** Grow with zoom: a whole interconnection stays light, a city reads clearly. */
// (Zoom may only drive a top-level interpolate, so extra width goes inside.)
const scaled = (prop: string, extra = 0) =>
  ["interpolate", ["linear"], ["zoom"], 3, ["+", ["*", ["get", prop], 0.55], extra], 6, ["+", ["get", prop], extra], 10, ["+", ["*", ["get", prop], 1.8], extra]] as never;

export interface MapGLProps {
  model: MapModel;
  net: CaseNetwork;
  palette: MapPalette;
  /** Layer marks and the shown result's flows (see lib/map/overlay.ts). */
  overlay: MapOverlay;
  /** Buses around the focus; everything else is dimmed. Null: nothing dimmed. */
  around: Set<number> | null;
  /** The bus the neighbourhood is drawn around. */
  focusBus: number | null;
  /** The selected element (a shared id). */
  element: string | null;
  onPick: (element: string | null) => void;
  onRecentre: (bus: number) => void;
  fitSignal: number;
  visible: boolean;
}

export function MapGL({ model, net, palette, overlay, around, focusBus, element, onPick, onRecentre, fitSignal, visible }: MapGLProps) {
  const box = useRef<HTMLDivElement>(null);
  const mapRef = useRef<GLMap | null>(null);
  const ready = useRef(false);
  // Work waiting for the style to load.
  const pending = useRef<((map: GLMap) => void)[]>([]);
  const handlers = useRef({ onPick, onRecentre });
  handlers.current = { onPick, onRecentre };
  const modelRef = useRef(model);
  modelRef.current = model;
  const overlayRef = useRef(overlay);
  overlayRef.current = overlay;

  // A bus, or an element the map has no line for (a generator, a load, a
  // transformer inside a substation): its site is the selection.
  const selectedBuses = useMemo(() => {
    if (!element) return new Set<number>();
    if (element.startsWith("bus:")) return new Set([Number(element.slice(4))]);
    if (model.lines.some((l) => l.element === element)) return new Set<number>();
    const bus = centreBus(element);
    return new Set(bus !== null ? [bus] : []);
  }, [element, model]);

  // Network features, rebuilt when the case, focus, selection or colours change.
  const sites = useMemo<FeatureCollection>(() => {
    const focusSite = focusBus !== null ? model.busSite.get(focusBus) : undefined;
    return {
      type: "FeatureCollection",
      features: model.sites.map((s) => {
        const o = overlay.sites[s.index];
        return {
          type: "Feature",
          id: s.index,
          properties: {
            color: o?.color ?? palette.ink.voltage(s.kv),
            r: siteRadius(s.kv) + (o?.bold ? 1.5 : 0),
            halo: o?.halo ?? "",
            estimated: s.estimated,
            dim: (around ? !s.buses.some((b) => around.has(b)) : false) || !!o?.dim,
            selected: s.buses.some((b) => selectedBuses.has(b)),
            focus: s.index === focusSite,
            // Marked sites are drawn over plain ones.
            sort: o?.halo || o?.bold ? 2 : o?.color ? 1 : 0,
          },
          geometry: { type: "Point", coordinates: [s.lon, s.lat] },
        };
      }),
    };
  }, [model, palette, overlay, around, focusBus, selectedBuses]);

  const lines = useMemo<FeatureCollection>(
    () => ({
      type: "FeatureCollection",
      features: model.lines.map((l) => {
        const a = model.sites[l.a];
        const b = model.sites[l.b];
        const o = overlay.lines[l.index];
        const online = o?.online ?? l.online;
        return {
          type: "Feature",
          id: l.index,
          properties: {
            color: online ? (o?.color ?? palette.ink.voltage(l.kv)) : palette.ink.off,
            w: lineWidth(l.kv) * (o?.bold ? 1.8 : 1),
            halo: o?.halo ?? "",
            dc: l.dc,
            online,
            dim: (around ? !(around.has(l.from) && around.has(l.to)) : false) || !!o?.dim,
            selected: l.element === element,
            sort: o?.halo || o?.bold ? 2 : o?.color ? 1 : 0,
          },
          geometry: { type: "LineString", coordinates: [[a.lon, a.lat], [b.lon, b.lat]] },
        };
      }),
    }),
    [model, palette, overlay, around, element]
  );

  // One arrow per line with a flow, pointing the way the power goes.
  const flows = useMemo<FeatureCollection>(
    () => ({
      type: "FeatureCollection",
      features: model.lines.flatMap((l) => {
        const o = overlay.lines[l.index];
        if (!o || o.mw === null || !o.online || Math.abs(o.mw) < ARROW_MIN_MW) return [];
        const a = model.sites[l.a];
        const b = model.sites[l.b];
        const ends = o.mw >= 0 ? [[a.lon, a.lat], [b.lon, b.lat]] : [[b.lon, b.lat], [a.lon, a.lat]];
        return [
          {
            type: "Feature" as const,
            id: l.index,
            properties: {
              size: arrowSize(Math.abs(o.mw), overlay.maxMw),
              // Heavier flows keep their arrow where arrows would collide.
              sort: -Math.abs(o.mw),
              dim: (around ? !(around.has(l.from) && around.has(l.to)) : false) || o.dim,
            },
            geometry: { type: "LineString" as const, coordinates: ends },
          },
        ];
      }),
    }),
    [model, overlay, around]
  );

  // Create the map once.
  useEffect(() => {
    if (!box.current) return;
    const map = new GLMap({
      container: box.current,
      style: { version: 8, sources: {}, layers: [{ id: "water", type: "background", paint: { "background-color": palette.water } }] },
      attributionControl: false,
      doubleClickZoom: false,
      dragRotate: false,
      pitchWithRotate: false,
      touchPitch: false,
      renderWorldCopies: false,
      maxPitch: 0,
      bounds: model.bounds ?? undefined,
      fitBoundsOptions: { padding: 48, maxZoom: 9 },
    });
    map.touchZoomRotate.disableRotation();
    mapRef.current = map;
    const popup = new Popup({ closeButton: false, closeOnClick: false, offset: 10, className: "stannum-map-tip" });

    map.on("load", () => {
      map.addSource("land", { type: "geojson", data: EMPTY });
      map.addSource("lakes", { type: "geojson", data: EMPTY });
      map.addSource("states", { type: "geojson", data: EMPTY });
      map.addSource("countries", { type: "geojson", data: EMPTY });
      map.addSource("lines", { type: "geojson", data: EMPTY });
      map.addSource("flows", { type: "geojson", data: EMPTY });
      map.addSource("sites", { type: "geojson", data: EMPTY });
      setArrow(map, palette);
      map.addLayer({ id: "land", type: "fill", source: "land", paint: { "fill-color": palette.land } });
      map.addLayer({ id: "lakes", type: "fill", source: "lakes", paint: { "fill-color": palette.water } });
      map.addLayer({ id: "states", type: "line", source: "states", paint: { "line-color": palette.states, "line-width": 0.8 } });
      map.addLayer({ id: "countries", type: "line", source: "countries", paint: { "line-color": palette.countries, "line-width": 1.2 } });
      // A layer's halo (a bridge, the outage, a voltage change) under the line.
      map.addLayer({
        id: "line-halo",
        type: "line",
        source: "lines",
        filter: ["!=", ["get", "halo"], ""],
        layout: { "line-cap": "round" },
        paint: { "line-color": ["get", "halo"], "line-width": scaled("w", 6), "line-opacity": ["case", ["get", "dim"], 0.2, 0.5] },
      });
      // The selected line's teal casing, under the line itself.
      map.addLayer({
        id: "line-selected",
        type: "line",
        source: "lines",
        filter: ["==", ["get", "selected"], true],
        layout: { "line-cap": "round" },
        paint: { "line-color": palette.ink.selected, "line-width": scaled("w", 5), "line-opacity": 0.55 },
      });
      for (const dc of [false, true])
        map.addLayer({
          id: dc ? "lines-dc" : "lines",
          type: "line",
          source: "lines",
          filter: ["==", ["get", "dc"], dc],
          layout: { "line-cap": "round", "line-sort-key": ["get", "sort"] },
          paint: {
            "line-color": ["get", "color"],
            "line-width": scaled("w"),
            "line-opacity": ["case", ["get", "dim"], palette.dimmed, 1],
            ...(dc ? { "line-dasharray": [2, 1.5] } : {}),
          },
        });
      map.addLayer({
        id: "flows",
        type: "symbol",
        source: "flows",
        layout: {
          "symbol-placement": "line-center",
          "symbol-sort-key": ["get", "sort"],
          "icon-image": "flow-arrow",
          "icon-rotation-alignment": "map",
          "icon-size": ["interpolate", ["linear"], ["zoom"], 3, ["*", ["get", "size"], 0.6], 6, ["get", "size"], 10, ["*", ["get", "size"], 1.4]],
        },
        paint: { "icon-opacity": ["case", ["get", "dim"], palette.dimmed, 1] },
      });
      map.addLayer({
        id: "site-halo",
        type: "circle",
        source: "sites",
        filter: ["!=", ["get", "halo"], ""],
        paint: {
          "circle-radius": scaled("r", 4),
          "circle-color": "rgba(0,0,0,0)",
          "circle-stroke-color": ["get", "halo"],
          "circle-stroke-width": 2.5,
          "circle-stroke-opacity": ["case", ["get", "dim"], 0.3, 1],
        },
      });
      map.addLayer({
        id: "site-selected",
        type: "circle",
        source: "sites",
        filter: ["==", ["get", "selected"], true],
        paint: { "circle-radius": scaled("r", 5), "circle-color": palette.ink.selected, "circle-opacity": 0.35, "circle-stroke-color": palette.ink.selected, "circle-stroke-width": 2 },
      });
      map.addLayer({
        id: "site-focus",
        type: "circle",
        source: "sites",
        filter: ["==", ["get", "focus"], true],
        paint: { "circle-radius": scaled("r", 8), "circle-color": "rgba(0,0,0,0)", "circle-stroke-color": palette.ink.halo, "circle-stroke-width": 2 },
      });
      map.addLayer({
        id: "sites",
        type: "circle",
        source: "sites",
        layout: { "circle-sort-key": ["get", "sort"] },
        paint: {
          "circle-radius": scaled("r"),
          // Estimated sites are drawn hollow.
          "circle-color": ["case", ["get", "estimated"], palette.land, ["get", "color"]],
          "circle-stroke-color": ["case", ["get", "estimated"], ["get", "color"], palette.land],
          "circle-stroke-width": ["case", ["get", "estimated"], 1.5, 1],
          "circle-opacity": ["case", ["get", "dim"], palette.dimmed, 1],
          "circle-stroke-opacity": ["case", ["get", "dim"], palette.dimmed, 1],
        },
      });
      ready.current = true;
      for (const fn of pending.current.splice(0)) fn(map);
      void loadOutlines()
        .then((o) => {
          for (const k of ["land", "lakes", "states", "countries"] as const) (map.getSource(k) as GeoJSONSource | undefined)?.setData(o.layers[k]);
        })
        .catch(() => {
          // Without outlines the network still draws, on plain water colour.
        });
    });

    const featureAt = (e: MapLayerMouseEvent) => {
      const hits = map.queryRenderedFeatures(e.point, { layers: ["sites", "lines", "lines-dc"] });
      // Sites win over the lines that meet at them.
      return hits.find((f) => f.layer.id === "sites") ?? hits[0];
    };
    const describe = (f: ReturnType<typeof featureAt>) => {
      const m = modelRef.current;
      if (!f || typeof f.id !== "number") return null;
      const o = overlayRef.current;
      if (f.layer.id === "sites") {
        const s = m.sites[f.id];
        const n = s.buses.length;
        const title = `${siteLabel(s, net)} · ${n} bus${n === 1 ? "" : "es"}${s.estimated ? " · estimated" : ""}`;
        return { element: siteElement(s), title, notes: o.sites[f.id]?.notes ?? [], bus: s.buses[0] };
      }
      const l = m.lines[f.id];
      return { element: l.element, title: elementLabel(l.element), notes: o.lines[f.id]?.notes ?? [], bus: null };
    };
    const tip = (title: string, notes: string[]) => {
      const box = document.createElement("div");
      const head = document.createElement("div");
      head.className = "font-medium";
      head.textContent = title;
      box.append(head);
      for (const note of notes.slice(0, 6)) {
        const line = document.createElement("div");
        line.textContent = note;
        box.append(line);
      }
      if (notes.length > 6) {
        const more = document.createElement("div");
        more.textContent = `+${notes.length - 6} more`;
        box.append(more);
      }
      return box;
    };
    map.on("mousemove", (e) => {
      const hit = describe(featureAt(e as MapLayerMouseEvent));
      map.getCanvas().style.cursor = hit ? "pointer" : "";
      if (hit) popup.setLngLat(e.lngLat).setDOMContent(tip(hit.title, hit.notes)).addTo(map);
      else popup.remove();
    });
    map.on("mouseout", () => popup.remove());
    map.on("click", (e) => handlers.current.onPick(describe(featureAt(e as MapLayerMouseEvent))?.element ?? null));
    map.on("dblclick", (e) => {
      const hit = describe(featureAt(e as MapLayerMouseEvent));
      if (hit?.bus != null) handlers.current.onRecentre(hit.bus);
    });

    const resize = new ResizeObserver(() => map.resize());
    resize.observe(box.current);
    return () => {
      resize.disconnect();
      popup.remove();
      map.remove();
      mapRef.current = null;
      ready.current = false;
      pending.current = [];
    };
    // The map is made once; later changes go through the effects below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Push data and colours once the style is loaded.
  const whenReady = (fn: (map: GLMap) => void) => {
    const map = mapRef.current;
    if (!map) return;
    if (ready.current) fn(map);
    else pending.current.push(fn);
  };
  useEffect(() => whenReady((map) => (map.getSource("sites") as GeoJSONSource).setData(sites)), [sites]);
  useEffect(() => whenReady((map) => (map.getSource("lines") as GeoJSONSource).setData(lines)), [lines]);
  useEffect(() => whenReady((map) => (map.getSource("flows") as GeoJSONSource).setData(flows)), [flows]);
  useEffect(
    () =>
      whenReady((map) => {
        map.setPaintProperty("water", "background-color", palette.water);
        map.setPaintProperty("land", "fill-color", palette.land);
        map.setPaintProperty("lakes", "fill-color", palette.water);
        map.setPaintProperty("states", "line-color", palette.states);
        map.setPaintProperty("countries", "line-color", palette.countries);
        map.setPaintProperty("line-selected", "line-color", palette.ink.selected);
        map.setPaintProperty("site-selected", "circle-color", palette.ink.selected);
        map.setPaintProperty("site-selected", "circle-stroke-color", palette.ink.selected);
        map.setPaintProperty("site-focus", "circle-stroke-color", palette.ink.halo);
        map.setPaintProperty("flows", "icon-opacity", ["case", ["get", "dim"], palette.dimmed, 1]);
        setArrow(map, palette);
        map.setPaintProperty("sites", "circle-color", ["case", ["get", "estimated"], palette.land, ["get", "color"]]);
        map.setPaintProperty("sites", "circle-stroke-color", ["case", ["get", "estimated"], ["get", "color"], palette.land]);
      }),
    [palette]
  );

  // Fit: the focus neighbourhood, or the whole case.
  const fit = () =>
    whenReady((map) => {
      const m = modelRef.current;
      const pts = m.sites.filter((s) => !around || s.buses.some((b) => around.has(b)));
      if (!pts.length) return;
      const w = Math.min(...pts.map((s) => s.lon));
      const e = Math.max(...pts.map((s) => s.lon));
      const so = Math.min(...pts.map((s) => s.lat));
      const n = Math.max(...pts.map((s) => s.lat));
      map.fitBounds([[w, so], [e, n]], { padding: 64, maxZoom: 10, duration: 300 });
    });
  // Fit when asked, and when the highlighted neighbourhood changes (but not
  // on mount: the map opens fitted to the whole case).
  const mounted = useRef(false);
  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    fit();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fitSignal, around]);

  // A selection made elsewhere that is off screen: bring it into view.
  useEffect(() => {
    if (!element) return;
    whenReady((map) => {
      const m = modelRef.current;
      const line = m.lines.find((l) => l.element === element);
      const bus = line ? null : centreBus(element);
      const site = bus !== null ? m.busSite.get(bus) : undefined;
      const pts = site !== undefined ? [m.sites[site]] : line ? [m.sites[line.a], m.sites[line.b]] : [];
      if (!pts.length) return;
      const b = map.getBounds();
      if (pts.every((p) => b.contains([p.lon, p.lat]))) return;
      map.easeTo({ center: [pts.reduce((s, p) => s + p.lon, 0) / pts.length, pts.reduce((s, p) => s + p.lat, 0) / pts.length], duration: 300 });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [element]);

  useEffect(() => {
    if (visible) mapRef.current?.resize();
  }, [visible]);

  // MapLibre makes its container position: relative, so the box fills a
  // positioned wrapper instead of positioning itself.
  return (
    <div className="absolute inset-0">
      <div ref={box} className="h-full w-full" data-testid="map-canvas" />
    </div>
  );
}
