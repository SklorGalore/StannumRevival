// The Diagram's Map style: the whole case at its bus coordinates, over
// offline outlines, with the focus bus's neighbourhood highlighted. It shares
// the focus bus, hop count and selection with the Network and One-line styles.
import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { MapPin, Maximize2, Minus, Plus, Spline } from "lucide-react";
import { useWorkspace } from "../../../store/workspaceStore";
import { MAX_HOPS, useDrawingStyle } from "../../../store/drawingStyleStore";
import { api } from "../../../lib/analysis/api";
import type { BusCoordinates, CaseNetwork } from "../../../lib/analysis/types";
import { caseNetwork } from "../../../lib/diagram/network";
import { useDarkCanvas, useInk } from "../../../lib/diagram/ink";
import { useTheme } from "../../../store/themeStore";
import { busElement, elementBus } from "../../../lib/elements";
import { count } from "../../../lib/analysis/format";
import { buildMap, neighbourhood, siteLabel, type MapModel } from "../../../lib/map/model";
import { mapPalette } from "../../../lib/map/palette";
import { centreBus } from "../../../ui/DrawAround";
import { ElementInspector, InspectorSection, inspectorClass } from "../../../ui/ElementInspector";
import { useWidth } from "../../../ui/useWidth";
import { NARROW_LEFT } from "../../../components/canvas/CaseActions";
import { StyleSwitch } from "../network/StyleSwitch";
import { showOneLine } from "../network/switchStyle";

const MapGL = lazy(() => import("./MapGL").then((m) => ({ default: m.MapGL })));

/** Draw the one-line around a bus with the map's hop count, and switch to it. */
function drawOneLineAround(bus: number) {
  const { focus, setFocus } = useDrawingStyle.getState();
  setFocus({ bus, depth: focus?.depth ?? 2 });
  void showOneLine();
}

interface MapData {
  net: CaseNetwork;
  coords: BusCoordinates;
  model: MapModel;
}

let cache: { key: string; data: Promise<MapData> } | null = null;

/** The case network and bus places, fetched once per case and coordinates file. */
function useMapData(enabled: boolean): { data: MapData | null; error: string | null } {
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const file = useWorkspace((s) => s.caseSummary?.coordinates?.path ?? null);
  // A Records edit can move buses between sites, so it counts too.
  const edits = useWorkspace((s) => s.caseSummary?.record_edits ?? 0);
  const key = `${caseVersion}:${file}:${edits}`;
  const [state, setState] = useState<{ key: string; data: MapData | null; error: string | null }>({ key: "", data: null, error: null });
  useEffect(() => {
    if (!enabled || !file) return;
    if (!cache || cache.key !== key) {
      const data = Promise.all([caseNetwork(), api.busCoordinates()]).then(([net, coords]) => ({ net, coords, model: buildMap(net, coords) }));
      cache = { key, data };
      data.catch(() => {
        if (cache?.data === data) cache = null;
      });
    }
    let live = true;
    cache.data
      .then((data) => live && setState({ key, data, error: null }))
      .catch((e: Error) => live && setState({ key, data: null, error: e.message }));
    return () => {
      live = false;
    };
  }, [enabled, file, key]);
  return state.key === key ? state : { data: null, error: null };
}

/** The Diagram toolbar while the Map style is shown. */
export function MapToolbar() {
  const focus = useDrawingStyle((s) => s.focus);
  const element = useDrawingStyle((s) => s.element);
  const busy = useDrawingStyle((s) => s.busy);
  const requestFit = useDrawingStyle((s) => s.requestFit);
  const coordinates = useWorkspace((s) => s.caseSummary?.coordinates ?? null);
  const attach = useWorkspace((s) => s.attachCoordinates);
  const bus = (element ? centreBus(element) : null) ?? focus?.bus ?? null;
  const [ref, width] = useWidth<HTMLDivElement>(1440);
  return (
    <div ref={ref} className="h-12 flex items-center px-3 gap-1 border-b border-slate-200 bg-white select-none overflow-hidden whitespace-nowrap shrink-0">
      {width >= 600 && <span className="shrink-0 font-semibold text-slate-700 text-sm mr-3">Diagram</span>}
      <StyleSwitch compact={width < 520} />
      <div className="w-px h-5 bg-slate-200 mx-1" />
      <button
        className="shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium text-slate-600 hover:bg-slate-100 disabled:opacity-40"
        onClick={requestFit}
        disabled={!coordinates}
        title="Fit the highlighted neighbourhood, or the whole case, to the view"
      >
        <Maximize2 size={14} />
        {width >= 520 && <span>Fit</span>}
      </button>
      <button
        className="shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium text-slate-600 hover:bg-slate-100"
        onClick={() => void attach()}
        title={coordinates ? `Bus places from ${coordinates.path}. Attach another file` : "Attach a CSV of bus latitudes and longitudes"}
      >
        <MapPin size={14} />
        {width >= 720 && <span className="max-w-48 truncate">{coordinates ? coordinates.name : "Attach coordinates…"}</span>}
      </button>
      <button
        className="shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium text-teal-700 hover:bg-teal-50 disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={bus === null || busy}
        onClick={() => bus !== null && drawOneLineAround(bus)}
        title={bus !== null ? `Draw ${focus?.depth ?? 2} hops around bus ${bus} as a one-line` : "Pick a bus first"}
      >
        <Spline size={14} />
        <span>{busy ? "Drawing…" : "Draw as one-line"}</span>
      </button>
    </div>
  );
}

/** The map, or what it needs before it can draw. */
export function MapCanvas({ visible }: { visible: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const attach = useWorkspace((s) => s.attachCoordinates);
  const { data, error } = useMapData(visible);
  const focus = useDrawingStyle((s) => s.focus);
  const setFocus = useDrawingStyle((s) => s.setFocus);
  const element = useDrawingStyle((s) => s.element);
  const setElement = useDrawingStyle((s) => s.setElement);
  const fit = useDrawingStyle((s) => s.fit);
  const [whole, setWhole] = useState(false);
  const ink = useInk();
  const dark = useDarkCanvas();
  // Tokens change with the theme, so the palette is read again when it does.
  const theme = useTheme((s) => s.theme);
  const palette = useMemo(() => mapPalette(ink, dark), [ink, dark, theme]);
  const [ref, width] = useWidth<HTMLDivElement>(1200);

  const around = useMemo(
    () => (data && focus && !whole ? neighbourhood(data.net, focus.bus, focus.depth) : null),
    [data, focus, whole]
  );

  function pick(next: string | null) {
    setElement(next);
    if (next) useWorkspace.getState().select(next, "diagram");
  }

  let message: React.ReactNode = null;
  if (!summary) message = "Open a case to draw it on a map.";
  else if (!summary.power.available) message = `This case can't be read for power flow, so it has no buses to place. ${summary.power.error ?? ""}`;
  else if (!summary.coordinates)
    message = (
      <div className="max-w-sm flex flex-col items-center gap-3" data-testid="map-prompt">
        <MapPin size={22} className="text-teal-700" />
        <p className="text-sm font-medium text-slate-800">Attach bus coordinates to draw the map</p>
        <p>
          A CSV with <code className="rounded bg-slate-100 px-1">bus, latitude, longitude</code> and, if you have it,{" "}
          <code className="rounded bg-slate-100 px-1">substation</code>. Buses it leaves out are placed next to their neighbours.
        </p>
        <button className="rounded-md bg-teal-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-800" onClick={() => void attach()}>
          Attach bus coordinates…
        </button>
      </div>
    );
  else if (error) message = `Couldn't place the buses: ${error}`;
  else if (!data) message = "Placing buses…";
  else if (!data.model.sites.length) message = `None of the case's buses are in ${summary.coordinates.name}.`;

  return (
    <div ref={ref} className="@container absolute inset-0 bg-white" data-testid="map-view">
      {message ? (
        <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-slate-500">{message}</div>
      ) : (
        <Suspense fallback={<div className="absolute inset-0 flex items-center justify-center text-xs text-slate-500">Loading the map…</div>}>
          <MapGL
            model={data!.model}
            net={data!.net}
            palette={palette}
            around={around}
            focusBus={whole ? null : (focus?.bus ?? null)}
            element={element}
            onPick={pick}
            onRecentre={(bus) => {
              setWhole(false);
              setFocus({ bus, depth: focus?.depth ?? 2 });
            }}
            fitSignal={fit}
            visible={visible}
          />
        </Suspense>
      )}
      {data && !message && (
        <MapBar model={data.model} net={data.net} around={around} whole={whole} setWhole={setWhole} hint={width >= 820} />
      )}
    </div>
  );
}

/** The bus the map highlights around and how many hops, or the whole case. */
function MapBar({
  model,
  net,
  around,
  whole,
  setWhole,
  hint,
}: {
  model: MapModel;
  net: CaseNetwork;
  around: Set<number> | null;
  whole: boolean;
  setWhole: (whole: boolean) => void;
  hint: boolean;
}) {
  const focus = useDrawingStyle((s) => s.focus);
  const setFocus = useDrawingStyle((s) => s.setFocus);
  const element = useDrawingStyle((s) => s.element);
  const btn = "p-1 rounded text-slate-600 hover:bg-slate-100 disabled:opacity-30 disabled:hover:bg-transparent";
  const bar = `${NARROW_LEFT} bottom-3 z-10 flex items-center gap-1 px-2 py-1 rounded-lg border border-slate-200 bg-white shadow-sm text-[11px] text-slate-600 whitespace-nowrap`;
  const placed = model.busSite.size;
  const sites = count(model.sites.length);
  const unplaced = model.unplaced ? `, ${count(model.unplaced)} without a place` : "";
  const pickedBus = element ? elementBus(element) : null;
  if (!focus || whole) {
    const start = pickedBus ?? focus?.bus ?? null;
    return (
      <div className={bar} data-testid="map-hops">
        <span className="px-1 font-semibold text-slate-800">Whole case</span>
        <span className="px-1 tabular-nums">
          {sites} sites, {count(placed)} buses placed{unplaced}
        </span>
        {start !== null && (
          <>
            <span className="w-px h-4 bg-slate-200 mx-1" />
            <button
              className="px-1.5 py-0.5 rounded text-teal-700 hover:bg-teal-50"
              onClick={() => {
                setWhole(false);
                setFocus({ bus: start, depth: focus?.depth ?? 2 });
              }}
            >
              Highlight around bus {start}
            </button>
          </>
        )}
        {hint && start === null && <span className="px-1 text-slate-400">Double-click a site to highlight around it</span>}
      </div>
    );
  }
  const site = model.busSite.get(focus.bus);
  const name = site !== undefined ? siteLabel(model.sites[site], net) : null;
  const step = (d: number) => setFocus({ bus: focus.bus, depth: focus.depth + d });
  const shown = around ? [...around].filter((b) => model.busSite.has(b)).length : 0;
  return (
    <div className={bar} data-testid="map-hops">
      <span className="px-1">
        Around <span className="font-semibold text-slate-800">Bus {focus.bus}</span>
        {name && hint && <span className="text-slate-500"> {name}</span>}
      </span>
      <span className="w-px h-4 bg-slate-200 mx-1" />
      <button className={btn} title="One hop fewer" aria-label="One hop fewer" disabled={focus.depth <= 1} onClick={() => step(-1)}>
        <Minus size={13} />
      </button>
      <span className="tabular-nums w-12 text-center" aria-label="Hops">
        {focus.depth} hop{focus.depth === 1 ? "" : "s"}
      </span>
      <button className={btn} title="One hop more" aria-label="One hop more" disabled={focus.depth >= MAX_HOPS} onClick={() => step(1)}>
        <Plus size={13} />
      </button>
      <span className="w-px h-4 bg-slate-200 mx-1" />
      <span className="px-1 tabular-nums">{count(shown)} buses</span>
      {site === undefined && <span className="px-1 text-amber-700">Bus {focus.bus} has no place</span>}
      <span className="w-px h-4 bg-slate-200 mx-1" />
      <button className="px-1.5 py-0.5 rounded text-teal-700 hover:bg-teal-50" onClick={() => setWhole(true)}>
        Whole case
      </button>
    </div>
  );
}

/** The shared element inspector, with a Map section for where the element is. */
export function MapInspector({ floating = false }: { floating?: boolean }) {
  const element = useDrawingStyle((s) => s.element);
  const setElement = useDrawingStyle((s) => s.setElement);
  const { data } = useMapData(true);
  if (!element) return null;
  const pick = (next: string) => {
    setElement(next);
    useWorkspace.getState().select(next, "diagram");
  };
  return (
    <aside className={inspectorClass(floating)} data-testid="map-selection">
      <ElementInspector element={element} from="diagram" onPick={pick} onClose={() => setElement(null)}>
        {data && <MapSection element={element} data={data} onPick={pick} />}
      </ElementInspector>
    </aside>
  );
}

function MapSection({ element, data, onPick }: { element: string; data: MapData; onPick: (element: string) => void }) {
  const focus = useDrawingStyle((s) => s.focus);
  const setFocus = useDrawingStyle((s) => s.setFocus);
  const busy = useDrawingStyle((s) => s.busy);
  const bus = centreBus(element);
  const siteIndex = element.startsWith("bus:") ? data.model.busSite.get(Number(element.slice(4))) : undefined;
  const site = siteIndex !== undefined ? data.model.sites[siteIndex] : null;
  const kv = new Map(data.net.buses.map((b) => [b.id, b.base_kv]));
  return (
    <InspectorSection id="map" title="Map">
      {site ? (
        <>
          <p className="mb-1.5 font-medium text-slate-800">{siteLabel(site, data.net)}</p>
          {site.estimated && <p className="mb-1.5 rounded bg-amber-50 px-2 py-1 text-amber-900">Placed from its neighbours: the coordinates file doesn't list it.</p>}
          {site.buses.length > 1 && (
            <div className="mb-1.5">
              <p className="text-slate-500 mb-1">{site.buses.length} buses here</p>
              <div className="flex flex-wrap gap-1">
                {site.buses.map((b) => (
                  <button
                    key={b}
                    className={`rounded border px-1.5 py-0.5 tabular-nums ${busElement(b) === element ? "border-teal-300 bg-teal-50 text-teal-800" : "border-slate-200 text-slate-700 hover:bg-slate-50"}`}
                    onClick={() => onPick(busElement(b))}
                  >
                    {b} · {kv.get(b) ?? 0} kV
                  </button>
                ))}
              </div>
            </div>
          )}
        </>
      ) : element.startsWith("bus:") ? (
        <p className="text-slate-500">No place on the map: nothing in its island is in the coordinates file.</p>
      ) : null}
      {bus !== null && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {focus?.bus !== bus && data.model.busSite.has(bus) && (
            <button
              className="rounded-md border border-slate-300 px-2 py-1 font-medium text-slate-700 hover:bg-slate-50"
              onClick={() => setFocus({ bus, depth: focus?.depth ?? 2 })}
            >
              Highlight around here
            </button>
          )}
          <button
            className="inline-flex items-center gap-1 rounded-md bg-teal-700 px-2 py-1 font-medium text-white hover:bg-teal-800 disabled:opacity-40"
            disabled={busy}
            onClick={() => drawOneLineAround(bus)}
            title={`Draw ${focus?.depth ?? 2} hops around bus ${bus} as a one-line`}
          >
            <Spline size={12} /> Draw as one-line
          </button>
        </div>
      )}
    </InspectorSection>
  );
}
