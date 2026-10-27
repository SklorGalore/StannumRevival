import { useState, type FormEvent, type ReactNode } from "react";
import { ChevronRight, Layers, Route, X } from "lucide-react";
import { useShownReport, useWorkspace } from "../../../store/workspaceStore";
import { useLayers } from "../../../store/layerStore";
import { useTopology, shareTopology } from "../../../lib/analysis/topologyStore";
import { api } from "../../../lib/analysis/api";
import type { TopologySnapshot } from "../../../lib/analysis/types";
import { LAYER_COLORS, LAYER_TITLES, LOADING_BANDS, type LayerId, type LayerLegend, type ResultMode } from "../../../lib/diagram/layers";
import { useStudy } from "../../../store/studyStore";
import { Button, inputClass } from "../../../ui/primitives";
import { count } from "../../../lib/analysis/format";
import { FieldPanel } from "../../topology/FieldPanel";
import { createColorPresentation } from "../../topology/fields/field-colors";
import { matchesBus, matchesEquipment } from "../../topology/fields/field-view";
import { isComplete } from "../../topology/fields/storage";
import { useDrawingStyle } from "../../../store/drawingStyleStore";
import { useContingencies } from "../../../store/contingencyStore";
import { elementLabel } from "../../../lib/elements";

/** The "Layers" button in either style's toolbar. */
export function LayersButton({ showLabel = true }: { showLabel?: boolean }) {
  const open = useLayers((s) => s.panel);
  const setPanel = useLayers((s) => s.setPanel);
  const legends = useLayers((s) => s.legends);
  return (
    <button
      type="button"
      onClick={() => setPanel(!open)}
      aria-pressed={open}
      aria-label="Layers"
      title="Layers: results, contingencies, bridges and cut buses, islands, paths, filters and colours"
      className={`shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium whitespace-nowrap ${
        open ? "bg-teal-50 text-teal-800 ring-1 ring-inset ring-teal-200" : "text-slate-600 hover:bg-slate-100"
      }`}
    >
      <Layers size={14} />
      {showLabel && <span>Layers</span>}
      {showLabel && legends.length > 0 && <span className="rounded-full bg-slate-100 px-1.5 text-[10px] text-slate-600">{legends.length}</span>}
    </button>
  );
}

function Swatch({ color, kind }: { color: string; kind: "fill" | "halo" | "line" }) {
  if (kind === "line") return <span className="inline-block h-1 w-4 rounded" style={{ background: color }} />;
  if (kind === "halo") return <span className="inline-block h-3 w-3 rounded-full border-2" style={{ borderColor: color }} />;
  return <span className="inline-block h-3 w-3 rounded-sm" style={{ background: color }} />;
}

/** One layer: a switch, what it shows, and its own controls. */
function LayerRow({ id, swatch, detail, children }: { id: LayerId; swatch: ReactNode; detail: ReactNode; children?: ReactNode }) {
  const on = useLayers((s) => s.on[id]);
  const toggle = useLayers((s) => s.toggle);
  return (
    <li className="px-3 py-2" data-layer={id}>
      <label className="flex items-center gap-2 text-xs font-medium text-slate-800">
        <input type="checkbox" checked={on} onChange={() => toggle(id)} />
        <span className="w-4 flex justify-center">{swatch}</span>
        <span className="flex-1">{LAYER_TITLES[id]}</span>
      </label>
      <div className="mt-0.5 pl-[3.25rem] text-[11px] text-slate-500">{detail}</div>
      {children && <div className="mt-1.5 pl-[3.25rem]">{children}</div>}
    </li>
  );
}

/** Find the fewest equipment hops between two buses, and draw them. */
function PathForm({ onShow }: { onShow: () => void }) {
  const path = useLayers((s) => s.path);
  const setPath = useLayers((s) => s.setPath);
  const [from, setFrom] = useState(path ? String(path.from) : "");
  const [to, setTo] = useState(path ? String(path.to) : "");
  const [busy, setBusy] = useState(false);
  async function find(ev: FormEvent) {
    ev.preventDefault();
    const a = Number(from);
    const b = Number(to);
    setBusy(true);
    try {
      const result = await api.shortestPath(a, b);
      setPath({ from: a, to: b, result, message: result ? null : `No active AC path connects bus ${a} to bus ${b}.` });
      if (result) onShow();
    } catch (e) {
      setPath({ from: a, to: b, result: null, message: (e as Error).message });
    } finally {
      setBusy(false);
    }
  }
  return (
    <form onSubmit={find} className="flex flex-col gap-1.5" aria-label="Shortest path">
      <div className="flex items-center gap-1">
        <input className={`${inputClass} w-0 flex-1 min-w-0`} type="number" min={1} required placeholder="From" aria-label="From bus" value={from} onChange={(e) => setFrom(e.target.value)} />
        <span className="text-slate-400">→</span>
        <input className={`${inputClass} w-0 flex-1 min-w-0`} type="number" min={1} required placeholder="To" aria-label="To bus" value={to} onChange={(e) => setTo(e.target.value)} />
        <Button type="submit" disabled={busy}>
          <Route size={12} /> Find
        </Button>
      </div>
      {path && (
        <p className="text-[11px] text-slate-600" data-testid="path-result">
          {path.result ? (
            <>
              {path.result.hops} equipment hop{path.result.hops === 1 ? "" : "s"}: {path.result.buses.join(" → ")}{" "}
              <button type="button" className="text-teal-700 underline" onClick={onShow}>
                Show
              </button>{" "}
              <button type="button" className="text-slate-500 underline" onClick={() => setPath(null)}>
                Clear
              </button>
            </>
          ) : (
            path.message
          )}
        </p>
      )}
    </form>
  );
}

/** What the contingency layer shows, or how to give it something to show. */
function ContingencyLayerDetail() {
  const detail = useContingencies((s) => s.detail);
  const onDiagram = useContingencies((s) => s.onDiagram);
  const setOnDiagram = useContingencies((s) => s.setOnDiagram);
  if (!detail || !onDiagram)
    return <>Open an outage in Power flow's Contingencies tab and choose Show on diagram.</>;
  return (
    <>
      After {elementLabel(detail.row.element)}: overloads in red, heavier flows in orange, lighter in blue.{" "}
      <button type="button" className="text-slate-500 underline" onClick={() => setOnDiagram(false)}>
        Clear
      </button>
    </>
  );
}

const RESULT_MODES: { mode: ResultMode; label: string }[] = [
  { mode: "loading", label: "Branch loading" },
  { mode: "voltage", label: "Bus voltage" },
  { mode: "change", label: "Change from a saved solve" },
  { mode: "sensitivity", label: "Sensitivity" },
];

/** Pick what the Results layer colours by. */
function ResultsLayerControls() {
  const report = useShownReport();
  const mode = useLayers((s) => s.resultMode);
  const compareWith = useLayers((s) => s.compareWith);
  const sensitivity = useLayers((s) => s.sensitivity);
  const setResults = useLayers((s) => s.setResults);
  const saved = useStudy((s) => s.saved);
  const savedOk = saved.some((x) => x.id === compareWith);
  let hint: string | null = null;
  if (!report) hint = "Solve the case in Power flow to colour by its results.";
  else if (mode === "change" && !saved.length) hint = "Save a solve in Power flow's Study tab, change something and solve again.";
  else if (mode === "change" && !savedOk) hint = "Choose the saved solve to compare with.";
  else if (mode === "sensitivity" && !sensitivity) hint = "Open Power flow's Sensitivities tab and choose Show on diagram.";
  return (
    <div className="flex flex-col gap-1.5">
      <select
        className={`${inputClass} w-full`}
        aria-label="Result to colour by"
        value={mode}
        onChange={(e) => setResults(e.target.value as ResultMode)}
      >
        {RESULT_MODES.map((m) => (
          <option key={m.mode} value={m.mode}>
            {m.label}
          </option>
        ))}
      </select>
      {mode === "change" && saved.length > 0 && (
        <select
          className={`${inputClass} w-full`}
          aria-label="Compare with"
          value={savedOk ? String(compareWith) : ""}
          onChange={(e) => setResults("change", e.target.value ? Number(e.target.value) : null)}
        >
          <option value="">Compare the latest solve with…</option>
          {saved.map((x) => (
            <option key={x.id} value={x.id}>
              {x.name}
            </option>
          ))}
        </select>
      )}
      {mode === "sensitivity" && sensitivity && <p className="text-[11px] text-slate-600">{sensitivity.title}</p>}
      {hint && <p className="text-[11px] text-slate-500">{hint}</p>}
    </div>
  );
}

/** Merge zero-impedance branches in the network graph (the core's setting). */
function MergeSwitch({ snapshot }: { snapshot: TopologySnapshot }) {
  const version = useTopology((s) => s.version);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function toggle(enabled: boolean) {
    setBusy(true);
    setError(null);
    try {
      const view = await api.topologySetMerge(enabled, snapshot.case);
      shareTopology(version, { ...snapshot, analysis: view.analysis, graph: view.graph });
      // A path found on the other graph no longer matches it.
      useLayers.getState().setPath(null);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <label className="flex items-center gap-2 text-[11px] text-slate-700">
        <input type="checkbox" checked={snapshot.graph.merge_zero_impedance} disabled={busy} onChange={(e) => toggle(e.target.checked)} />
        Merge them into one node in the network
      </label>
      {error && <p className="text-[11px] text-red-700">{error}</p>}
    </>
  );
}

/**
 * The Layers panel beside the canvas: topology findings and field rules,
 * drawn on whichever style is showing.
 */
export function LayersPanel({ floating = false }: { floating?: boolean }) {
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const summary = useWorkspace((s) => s.caseSummary);
  const snapshot = useTopology((s) => (s.version === caseVersion ? s.snapshot : null));
  const setPanel = useLayers((s) => s.setPanel);
  const rules = useLayers((s) => s.rules);
  const setRules = useLayers((s) => s.setRules);
  const colors = useLayers((s) => s.colors);
  const setColors = useLayers((s) => s.setColors);
  const fieldsOn = useLayers((s) => s.on.fields);
  const toggle = useLayers((s) => s.toggle);
  const [fieldsOpen, setFieldsOpen] = useState(rules.length > 0);
  const showPath = () => useDrawingStyle.getState().showPath();

  let body: ReactNode;
  if (!summary) body = <p className="p-3 text-xs text-slate-500">Open a case to add layers.</p>;
  else if (!summary.topology.available)
    body = <p className="p-3 text-xs text-slate-500">Layers need the case's topology, which couldn't be built: {summary.topology.error}</p>;
  else if (!snapshot) body = <p className="p-3 text-xs text-slate-500">Building the network…</p>;
  else {
    const a = snapshot.analysis;
    const groups = snapshot.graph.zero_impedance_groups.filter((g) => g.length > 1).length;
    const complete = rules.filter(isComplete);
    const shownBuses = complete.length ? snapshot.case.buses.filter((b) => matchesBus(snapshot, b.number, complete)).length : snapshot.case.buses.length;
    const shownEquipment = complete.length ? snapshot.case.equipment.filter((e) => matchesEquipment(snapshot, e.id, complete)).length : snapshot.case.equipment.length;
    body = (
      <ul className="divide-y divide-slate-100">
        <LayerRow id="critical" swatch={<Swatch color={LAYER_COLORS.critical} kind="halo" />} detail={`${count(a.bridges.length)} bridges and ${count(a.articulation_buses.length)} cut buses: losing one splits its island.`} />
        <LayerRow id="islands" swatch={<Swatch color="#e69f00" kind="fill" />} detail={a.islands.length === 1 ? "One island: every active bus is connected." : `${count(a.islands.length)} islands, each in its own colour.`} />
        <LayerRow id="merged" swatch={<Swatch color={LAYER_COLORS.merged} kind="halo" />} detail={`${count(groups)} groups of buses tied by zero-impedance branches.`}>
          <MergeSwitch snapshot={snapshot} />
        </LayerRow>
        <LayerRow id="path" swatch={<Swatch color={LAYER_COLORS.path} kind="line" />} detail="The fewest equipment hops between two buses.">
          <PathForm onShow={showPath} />
        </LayerRow>
        <LayerRow
          id="results"
          swatch={<span className="inline-block h-1 w-4 rounded" style={{ background: `linear-gradient(to right, ${LOADING_BANDS[0][1]}, ${LOADING_BANDS[4][1]})` }} />}
          detail="Loading, voltage, the change since a saved solve, or a sensitivity, from Power flow."
        >
          <ResultsLayerControls />
        </LayerRow>
        <LayerRow id="contingency" swatch={<Swatch color={LAYER_COLORS.tripped} kind="halo" />} detail={<ContingencyLayerDetail />} />
        <li className="py-2" data-layer="fields">
          <div className="flex items-center gap-2 px-3 text-xs font-medium text-slate-800">
            <input
              type="checkbox"
              aria-label={LAYER_TITLES.fields}
              checked={fieldsOn}
              onChange={() => toggle("fields")}
            />
            <button type="button" className="flex-1 flex items-center gap-1 text-left" onClick={() => setFieldsOpen(!fieldsOpen)} aria-expanded={fieldsOpen}>
              <ChevronRight size={13} className={`text-slate-400 transition-transform ${fieldsOpen ? "rotate-90" : ""}`} />
              {LAYER_TITLES.fields}
              {complete.length > 0 && <span className="rounded-full bg-slate-100 px-1.5 text-[10px] text-slate-600">{complete.length}</span>}
            </button>
          </div>
          {fieldsOpen && (
            <FieldPanel
              snapshot={snapshot}
              rules={rules}
              onRules={setRules}
              colors={colors}
              onColors={setColors}
              presentation={createColorPresentation(snapshot, colors)}
              counts={{ buses: shownBuses, totalBuses: snapshot.case.buses.length, equipment: shownEquipment, totalEquipment: snapshot.case.equipment.length }}
            />
          )}
        </li>
      </ul>
    );
  }
  return (
    <aside
      className={
        floating
          ? "absolute left-2 top-2 bottom-2 z-40 w-80 max-w-[calc(100%-1rem)] rounded-md border border-slate-200 bg-white shadow-lg overflow-y-auto overflow-x-hidden"
          : "w-80 shrink-0 border-r border-slate-200 bg-white overflow-y-auto overflow-x-hidden"
      }
      data-testid="layers-panel"
      aria-label="Layers"
    >
      <header className="flex items-center gap-2 px-3 py-2 border-b border-slate-200">
        <Layers size={14} className="text-slate-500" />
        <h2 className="flex-1 text-xs font-semibold text-slate-700">Layers</h2>
        <button className="p-1 rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700" onClick={() => setPanel(false)} aria-label="Hide layers" title="Hide layers">
          <X size={14} />
        </button>
      </header>
      <p className="px-3 pt-2 text-[11px] text-slate-500">Drawn on the network, the one-line and the map alike.</p>
      {body}
    </aside>
  );
}

/** A key to the layers that are on, over the canvas. */
export function LayerLegend({ legends }: { legends: LayerLegend[] }) {
  if (!legends.length) return null;
  return (
    <div
      className="pointer-events-none absolute left-3 top-3 z-30 max-w-[240px] max-h-[40%] overflow-hidden rounded-md border border-slate-200 bg-white/95 px-2 py-1.5 text-[11px] text-slate-700 shadow-sm"
      data-testid="layer-legend"
    >
      {legends.map((l) => (
        <div key={l.id} className="mb-1 last:mb-0">
          <p className="font-semibold text-slate-600">{l.title}</p>
          <ul>
            {l.entries.map((e, i) => (
              <li key={i} className="flex items-center gap-1.5">
                <span className="w-4 flex justify-center shrink-0">
                  {e.kind === "gradient" ? (
                    <span className="inline-block h-2 w-4 rounded" style={{ background: `linear-gradient(to right, ${e.color}, ${e.to})` }} />
                  ) : (
                    <Swatch color={e.color} kind={e.kind} />
                  )}
                </span>
                <span className="truncate">{e.label}</span>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}
