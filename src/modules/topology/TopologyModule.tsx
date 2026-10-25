import { useEffect, useMemo, useState, type FormEvent } from "react";
import { Download, FolderOpen, Maximize2, Network, Route } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { api, saveTextExport } from "../../lib/analysis/api";
import type { Equipment, GraphData, SourceRecord, TopoPath, TopologySnapshot } from "../../lib/analysis/types";
import { count, fixed, toCsv } from "../../lib/analysis/format";
import {
  Button,
  EmptyState,
  ErrorBanner,
  Metric,
  Notice,
  Panel,
  SearchBox,
  Tabs,
  inputClass,
} from "../../ui/primitives";
import { DataTable, filterRows, type Column } from "../../ui/DataTable";
import { NetworkGraph, type GraphSelection } from "./NetworkGraph";
import { FieldPanel, GraphLegend } from "./FieldPanel";
import { filterGraph, matchesBus, matchesEquipment, type FilterRule } from "./fields/field-view";
import { createColorPresentation, defaultColorSettings, loadColorSettings, saveColorSettings, type ColorSettings } from "./fields/field-colors";
import { DISPLAY_LIMIT, busRows, focusedGraph, islandGraph, neighborhood, nodeForBus, type TopoBusRow } from "./viewModel";

type Tab = "buses" | "islands" | "critical" | "equipment" | "diagnostics";
type Scope = { kind: "neighborhood" | "island" | "full" | "path"; title: string; data: GraphData };

interface IslandRow { island: number; buses: number; junctions: number; bus_numbers: number[] }
interface CriticalRow { type: string; id: string; from: string; to: string; winding: number | string; bus: number | null }
interface EquipmentRow { id: string; kind: string; circuit: string; buses: number[]; name: string; status: number; topology: string; line: number }
interface DiagRow { i: number; severity: string; section: string; line: number; message: string }

const busColumns: Column<TopoBusRow>[] = [
  { key: "bus", header: "Bus", value: (r) => r.bus, align: "right" },
  { key: "name", header: "Name", value: (r) => r.name },
  { key: "members", header: "Members", value: (r) => r.members.join(" "), render: (r) => (r.members.length > 1 ? r.members.length : "") , align: "right" },
  { key: "base_kv", header: "Base kV", value: (r) => r.base_kv, render: (r) => fixed(r.base_kv, 1), align: "right" },
  { key: "degree", header: "Degree", value: (r) => r.degree, align: "right" },
  { key: "island", header: "Island", value: (r) => r.island, align: "right" },
  { key: "status", header: "Status", value: (r) => r.status },
  { key: "articulation", header: "Cut bus", value: (r) => (r.articulation ? "Yes" : ""), render: (r) => (r.articulation ? <span className="text-amber-700 font-medium">Yes</span> : "") },
];
const islandColumns: Column<IslandRow>[] = [
  { key: "island", header: "Island", value: (r) => r.island, align: "right" },
  { key: "buses", header: "Buses", value: (r) => r.buses, align: "right" },
  { key: "junctions", header: "Junctions", value: (r) => r.junctions, align: "right" },
  { key: "bus_numbers", header: "Bus numbers", value: (r) => r.bus_numbers.join(", "), render: (r) => r.bus_numbers.slice(0, 10).join(", ") + (r.bus_numbers.length > 10 ? ` … +${r.bus_numbers.length - 10}` : "") },
];
const criticalColumns: Column<CriticalRow>[] = [
  { key: "type", header: "Type", value: (r) => r.type },
  { key: "id", header: "Equipment / bus", value: (r) => r.id },
  { key: "from", header: "From", value: (r) => r.from },
  { key: "to", header: "To", value: (r) => r.to },
  { key: "winding", header: "Winding", value: (r) => r.winding },
];
const equipmentColumns: Column<EquipmentRow>[] = [
  { key: "id", header: "ID", value: (r) => r.id },
  { key: "kind", header: "Type", value: (r) => r.kind },
  { key: "circuit", header: "Circuit", value: (r) => r.circuit },
  { key: "buses", header: "Buses", value: (r) => r.buses.join(" ↔ ") },
  { key: "name", header: "Name", value: (r) => r.name },
  { key: "status", header: "Status", value: (r) => r.status, align: "right" },
  { key: "topology", header: "Topology", value: (r) => r.topology },
  { key: "line", header: "RAW line", value: (r) => r.line, align: "right" },
];
const diagnosticColumns: Column<DiagRow>[] = [
  { key: "severity", header: "Severity", value: (r) => r.severity },
  { key: "section", header: "Section", value: (r) => r.section },
  { key: "line", header: "Line", value: (r) => r.line, align: "right" },
  { key: "message", header: "Message", value: (r) => r.message },
];

function SourceFields({ source }: { source: SourceRecord }) {
  return (
    <details className="mt-2 text-[11px]">
      <summary className="cursor-pointer font-medium text-slate-600">RAW fields · line {source.line}</summary>
      <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
        {Object.entries(source.fields).map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-slate-500">{k}</dt>
            <dd className="font-mono text-slate-800 truncate">{v || "—"}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}

function Inspector({
  snapshot,
  selection,
  onSelect,
  onNeighborhood,
}: {
  snapshot: TopologySnapshot;
  selection: GraphSelection;
  onSelect: (s: GraphSelection) => void;
  onNeighborhood: (bus: number, hops: number) => void;
}) {
  const [hops, setHops] = useState(2);
  const { case: c, analysis: a, graph } = snapshot;

  if (selection.equipment) {
    const e = c.equipment.find((x) => x.id === selection.equipment);
    if (!e) return null;
    const active = graph.edges.some((g) => g.equipment_id === e.id);
    const absorbed = graph.absorbed_equipment_ids.includes(e.id);
    return (
      <div className="p-3 text-xs">
        <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">Equipment</p>
        <h3 className="text-sm font-semibold capitalize">{e.kind} · circuit {e.circuit.trim()}</h3>
        <p className="text-slate-500 break-all">{e.name.trim() || e.id}</p>
        <p className="mt-1.5">
          <span className={`rounded px-1.5 py-px text-[10px] font-semibold ${active ? "bg-emerald-100 text-emerald-800" : "bg-slate-100 text-slate-600"}`}>
            {active ? "In active graph" : absorbed ? "Absorbed inside merged node" : "Excluded from graph"}
          </span>
        </p>
        <p className="mt-2 text-slate-500">Terminals</p>
        <div className="flex flex-wrap gap-1 mt-0.5">
          {e.buses.map((b, i) => (
            <button key={`${b}-${i}`} className="rounded border border-slate-300 px-1.5 py-px hover:bg-slate-50" onClick={() => onSelect({ bus: b, equipment: null })}>
              Bus {b}
              {e.active_terminals[i] === false && " (open)"}
            </button>
          ))}
        </div>
        <SourceFields source={e.source} />
      </div>
    );
  }

  const node = selection.bus === null ? undefined : nodeForBus(graph, selection.bus);
  const representative = node?.bus ?? selection.bus;
  const bus = c.buses.find((b) => b.number === representative);
  if (!bus)
    return (
      <div className="p-6 text-center text-xs text-slate-500">
        Select a bus or connection in the graph or a table to inspect it.
      </div>
    );
  const members = node?.member_buses?.length ? node.member_buses : [bus.number];
  const memberSet = new Set(members);
  const degree = a.degrees.find((d) => d.bus === bus.number);
  const connected: Equipment[] = c.equipment.filter((e) => e.buses.some((n) => memberSet.has(n)));
  const loads = c.loads.filter((i) => memberSet.has(i.bus));
  const generators = c.generators.filter((i) => memberSet.has(i.bus));
  return (
    <div className="p-3 text-xs">
      <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">Bus</p>
      <h3 className="text-sm font-semibold">
        {members.length > 1 ? `Merged node ${bus.number} · ${members.length} buses` : `Bus ${bus.number}`}
      </h3>
      <p className="text-slate-600">{bus.name.trim()}</p>
      <dl className="mt-2 grid grid-cols-2 gap-1.5">
        <div><dt className="text-slate-500">Base kV</dt><dd className="font-medium">{fixed(bus.base_kv, 1)}</dd></div>
        <div><dt className="text-slate-500">Type</dt><dd className="font-medium">{bus.bus_type === 4 ? "Out of service" : `IDE ${bus.bus_type}`}</dd></div>
        <div><dt className="text-slate-500">Degree</dt><dd className="font-medium">{degree?.degree ?? "—"}</dd></div>
        <div><dt className="text-slate-500">Island</dt><dd className="font-medium">{degree?.island ?? "—"}</dd></div>
      </dl>
      {a.articulation_buses.includes(bus.number) && (
        <p className="mt-2 rounded bg-amber-50 px-2 py-1 text-amber-900">Articulation bus: removing it splits its island.</p>
      )}
      {node && (
        <div className="mt-2 flex items-center gap-1.5">
          <input
            type="number"
            min={1}
            max={6}
            value={hops}
            onChange={(e) => setHops(Math.min(6, Math.max(1, Number(e.target.value) || 1)))}
            className={`${inputClass} w-14`}
            aria-label="Neighborhood hops"
          />
          <Button onClick={() => onNeighborhood(bus.number, hops)}>Show {hops}-hop neighborhood</Button>
        </div>
      )}
      <p className="mt-3 font-medium text-slate-700">Connected equipment ({connected.length})</p>
      <ul className="mt-1 flex flex-col gap-0.5 max-h-48 overflow-y-auto">
        {connected.map((e) => (
          <li key={e.id}>
            <button className="w-full text-left rounded px-1.5 py-1 hover:bg-slate-50" onClick={() => onSelect({ bus: null, equipment: e.id })}>
              <span className="capitalize">{e.kind}</span> {e.circuit.trim()}{" "}
              <span className="text-slate-500">{e.buses.join(" ↔ ")}</span>
            </button>
          </li>
        ))}
      </ul>
      <p className="mt-3 font-medium text-slate-700">
        Loads ({loads.length}) / generators ({generators.length})
      </p>
      <ul className="mt-1 text-slate-600">
        {[...loads.map((l) => ["Load", l] as const), ...generators.map((g) => ["Gen", g] as const)].map(([k, i]) => (
          <li key={`${k}-${i.id}`}>
            {k} {i.id} · {fixed(i.p_mw, 1)} MW · {fixed(i.q_mvar, 1)} MVAr {i.status === 1 ? "" : "(off)"}
          </li>
        ))}
      </ul>
      <SourceFields source={bus.source} />
    </div>
  );
}

export function TopologyModule({ active }: { active: boolean }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const openCase = useWorkspace((s) => s.openCase);

  const [snapshot, setSnapshot] = useState<TopologySnapshot | null>(null);
  const [loadedVersion, setLoadedVersion] = useState(-1);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScope] = useState<Scope | null>(null);
  const [selection, setSelection] = useState<GraphSelection>({ bus: null, equipment: null });
  const [path, setPath] = useState<TopoPath | null>(null);
  const [pathText, setPathText] = useState("Choose two bus numbers to find the fewest equipment hops.");
  const [fromBus, setFromBus] = useState("");
  const [toBus, setToBus] = useState("");
  const [tab, setTab] = useState<Tab>("buses");
  const [search, setSearch] = useState("");
  const [fitSignal, setFitSignal] = useState(0);
  const [side, setSide] = useState<"selection" | "fields">("selection");
  const [rules, setRules] = useState<FilterRule[]>([]);
  const [colors, setColors] = useState<ColorSettings>(defaultColorSettings);

  const available = !!summary?.topology.available;

  function defaultScope(s: TopologySnapshot): Scope | null {
    const origin = s.analysis.degrees[0]?.bus;
    if (origin === undefined) return null;
    return { kind: "neighborhood", title: `Bus ${origin} · 2-hop neighborhood`, data: neighborhood(s.graph, origin, 2) };
  }

  // Fetch lazily: the snapshot can be large, so only when this module is shown.
  useEffect(() => {
    if (!active || !available || loadedVersion === caseVersion) return;
    const version = caseVersion;
    setBusy(true);
    setError(null);
    api
      .topologySnapshot()
      .then((s) => {
        if (version !== useWorkspace.getState().caseVersion) return;
        setSnapshot(s);
        setLoadedVersion(version);
        const saved = loadFieldSettings(useWorkspace.getState().caseSummary?.path);
        setRules(saved.rules);
        setColors(saved.colors);
        setScope(defaultScope(s));
        setSelection({ bus: s.analysis.degrees[0]?.bus ?? null, equipment: null });
        setPath(null);
        setPathText("Choose two bus numbers to find the fewest equipment hops.");
        setFromBus("");
        setToBus("");
        setSearch("");
      })
      .catch((e) => setError((e as Error).message))
      .finally(() => setBusy(false));
  }, [active, available, caseVersion, loadedVersion]);

  useEffect(() => {
    if (!available) {
      setSnapshot(null);
      setScope(null);
    }
  }, [available, caseVersion]);

  async function toggleMerge(enabled: boolean) {
    if (!snapshot) return;
    setBusy(true);
    setError(null);
    try {
      const view = await api.topologySetMerge(enabled);
      const next = { ...snapshot, analysis: view.analysis, graph: view.graph };
      setSnapshot(next);
      setPath(null);
      setPathText("Choose two bus numbers to find the fewest equipment hops.");
      setScope(defaultScope(next));
      setSelection((s) => ({ bus: s.bus === null ? null : nodeForBus(next.graph, s.bus)?.bus ?? s.bus, equipment: null }));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function findPath(ev: FormEvent) {
    ev.preventDefault();
    if (!snapshot) return;
    const from = Number(fromBus);
    const to = Number(toBus);
    setPathText("Finding the shortest equipment path…");
    try {
      const result = await api.shortestPath(from, to);
      setPath(result);
      if (!result) {
        setPathText(`No active AC path connects bus ${from} to bus ${to}.`);
        return;
      }
      setPathText(`${result.hops} equipment hop${result.hops === 1 ? "" : "s"} · ${result.buses.join(" → ")}`);
      setScope({ kind: "path", title: `Path · ${from} → ${to}`, data: focusedGraph(snapshot.graph, new Set(result.node_ids)) });
    } catch (e) {
      setPath(null);
      setPathText((e as Error).message);
    }
  }

  const rows = useMemo(() => {
    if (!snapshot) return null;
    const { case: c, analysis: a, graph } = snapshot;
    const inGraph = new Set(graph.edges.map((e) => e.equipment_id));
    const absorbed = new Set(graph.absorbed_equipment_ids);
    return {
      buses: busRows(snapshot),
      islands: a.islands.map((i) => ({ island: i.id, buses: i.buses.length, junctions: i.synthetic_nodes.length, bus_numbers: i.buses })),
      critical: [
        ...a.bridges.map((e) => ({ type: "Bridge", id: e.equipment_id, from: e.source, to: e.target, winding: e.winding ?? "", bus: null })),
        ...a.articulation_buses.map((b) => ({ type: "Articulation bus", id: String(b), from: `bus:${b}`, to: "", winding: "", bus: b })),
      ] as CriticalRow[],
      equipment: c.equipment.map((e) => ({
        id: e.id,
        kind: e.kind,
        circuit: e.circuit.trim(),
        buses: e.buses,
        name: e.name.trim(),
        status: e.status,
        topology: inGraph.has(e.id) ? "Active" : absorbed.has(e.id) ? "Absorbed" : "Excluded",
        line: e.source.line,
      })),
      diagnostics: c.diagnostics.map((d, i) => ({ i, ...d })),
    };
  }, [snapshot]);

  // Rules still being filled in (no value yet) are ignored rather than hiding everything.
  const activeRules = useMemo(() => rules.filter(isComplete), [rules]);
  const shownRows = useMemo(() => {
    if (!rows || !snapshot || !activeRules.length) return rows;
    return {
      ...rows,
      buses: rows.buses.filter((r) => matchesBus(snapshot, r.bus, activeRules)),
      equipment: rows.equipment.filter((r) => matchesEquipment(snapshot, r.id, activeRules)),
    };
  }, [rows, snapshot, activeRules]);
  const shownGraph = useMemo(
    () => (scope && snapshot ? filterGraph(snapshot, scope.data, activeRules) : null),
    [scope, snapshot, activeRules]
  );
  const presentation = useMemo(
    () => (snapshot ? createColorPresentation(snapshot, colors) : { nodeColors: {}, edgeColors: {}, legends: [] }),
    [snapshot, colors]
  );
  useEffect(() => {
    if (snapshot) saveFieldSettings(summary?.path, rules, colors);
  }, [snapshot, summary?.path, rules, colors]);

  if (!summary) {
    return (
      <EmptyState
        icon={<Network size={22} />}
        title="Topology"
        action={
          <Button variant="primary" onClick={() => openCase()}>
            <FolderOpen size={14} /> Open case…
          </Button>
        }
      >
        Open a PSS/E RAW v35 case to find islands, bridges and articulation buses, trace shortest paths and explore the
        network graph. Powered by nodal.
      </EmptyState>
    );
  }
  if (!available) {
    return (
      <div className="p-6 max-w-2xl">
        <ErrorBanner>
          <strong>Topology is unavailable for this case.</strong> {summary.topology.error}
        </ErrorBanner>
      </div>
    );
  }
  if (!snapshot || !rows || !shownRows) {
    return error ? (
      <div className="p-6 max-w-2xl">
        <ErrorBanner>{error}</ErrorBanner>
      </div>
    ) : (
      <EmptyState title={busy ? "Building the topology…" : "Topology"} />
    );
  }

  const { case: c, analysis: a } = snapshot;
  const lines = c.equipment.filter((e) => e.kind === "line").length;
  const transformers = c.equipment.filter((e) => e.kind === "transformer").length;
  const warnings = c.diagnostics.filter((d) => d.severity === "warning");
  const tooLarge = shownGraph && shownGraph.nodes.length > DISPLAY_LIMIT;

  const tabDefs = [
    { id: "buses" as const, label: "Buses", count: shownRows.buses.length },
    { id: "islands" as const, label: "Islands", count: rows.islands.length },
    { id: "critical" as const, label: "Critical connections", count: rows.critical.length },
    { id: "equipment" as const, label: "Equipment", count: shownRows.equipment.length },
    { id: "diagnostics" as const, label: "Diagnostics", count: rows.diagnostics.length },
  ];

  function onRowClick(row: unknown) {
    if (tab === "buses") setSelection({ bus: (row as TopoBusRow).bus, equipment: null });
    else if (tab === "equipment") setSelection({ bus: null, equipment: (row as EquipmentRow).id });
    else if (tab === "critical") {
      const r = row as CriticalRow;
      setSelection(r.bus !== null ? { bus: r.bus, equipment: null } : { bus: null, equipment: r.id });
    } else if (tab === "islands") {
      const r = row as IslandRow;
      const data = islandGraph(snapshot!, r.island);
      if (data) {
        setPath(null);
        setScope({ kind: "island", title: `Island ${r.island}`, data });
      }
    }
  }

  async function exportCsv() {
    const table = { buses: [shownRows!.buses, busColumns], islands: [rows!.islands, islandColumns], critical: [rows!.critical, criticalColumns], equipment: [shownRows!.equipment, equipmentColumns], diagnostics: [rows!.diagnostics, diagnosticColumns] }[tab] as [unknown[], Column<unknown>[]];
    const [data, columns] = table;
    const filtered = filterRows(data, columns, search);
    const csv = toCsv(filtered.map((r) => Object.fromEntries(columns.map((col) => [col.key, col.value(r)]))));
    await saveTextExport(`${summary!.name.replace(/\.raw$/i, "")}-${tab}.csv`, csv, { name: "CSV", extensions: ["csv"] });
  }

  async function exportJson() {
    const json = JSON.stringify(
      { format_version: 1, filename: summary!.name, case: c, graph: snapshot!.graph, analysis: a, path, view: scope && { scope: scope.kind, title: scope.title } },
      null,
      2
    );
    await saveTextExport(`${summary!.name.replace(/\.raw$/i, "")}-topology.json`, json, { name: "JSON", extensions: ["json"] });
  }

  const selectedKey =
    tab === "buses" ? selection.bus : tab === "equipment" ? selection.equipment : undefined;

  return (
    <div className="h-full flex flex-col gap-3 p-3 overflow-hidden" aria-busy={busy}>
      <div className="flex flex-wrap items-center gap-3">
        <div className="grid grid-cols-3 lg:grid-cols-6 gap-2 flex-1 min-w-0">
          <Metric label="RAW buses" value={count(c.buses.length)} detail={`${count(a.original_active_buses)} active · ${count(a.excluded_buses)} excluded`} />
          <Metric label="Lines" value={count(lines)} detail="Parallel circuits kept" />
          <Metric label="Transformers" value={count(transformers)} detail={`${count(a.synthetic_junctions)} 3-winding junctions`} />
          <Metric label="Islands" value={count(a.islands.length)} tone={a.islands.length > 1 ? "warn" : "neutral"} />
          <Metric label="Bridges" value={count(a.bridges.length)} />
          <Metric label="Articulation buses" value={count(a.articulation_buses.length)} />
        </div>
        <div className="flex flex-col gap-1.5">
          <label className="flex items-center gap-2 text-xs text-slate-700">
            <input
              type="checkbox"
              checked={snapshot.graph.merge_zero_impedance}
              disabled={busy}
              onChange={(e) => toggleMerge(e.target.checked)}
            />
            Merge zero-impedance branches
          </label>
          <Button variant="ghost" onClick={exportJson}>
            <Download size={13} /> Export JSON
          </Button>
        </div>
      </div>
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {warnings.length > 0 && (
        <Notice>
          <strong>Topology coverage is limited.</strong> {warnings.map((d) => `${d.message} (${d.section})`).join(" · ")}
        </Notice>
      )}
      <div className="flex-1 min-h-0 grid grid-cols-[1fr_300px] grid-rows-[minmax(0,1.6fr)_minmax(0,1fr)] gap-3">
        <Panel
          eyebrow="Network view"
          title={scope?.title ?? "Network"}
          actions={
            <>
              <form onSubmit={findPath} className="flex items-center gap-1">
                <Route size={13} className="text-slate-400" />
                <input className={`${inputClass} w-20`} type="number" min={1} required placeholder="From" aria-label="From bus" value={fromBus} onChange={(e) => setFromBus(e.target.value)} />
                <span className="text-slate-400">→</span>
                <input className={`${inputClass} w-20`} type="number" min={1} required placeholder="To" aria-label="To bus" value={toBus} onChange={(e) => setToBus(e.target.value)} />
                <Button type="submit">Path</Button>
              </form>
              <Button
                onClick={() => {
                  setPath(null);
                  setScope({ kind: "full", title: "Full network", data: snapshot.graph });
                }}
              >
                Full network
              </Button>
              <Button variant="ghost" onClick={() => setFitSignal((n) => n + 1)} aria-label="Fit view" title="Fit view">
                <Maximize2 size={13} />
              </Button>
            </>
          }
          bodyClassName="relative"
        >
          {shownGraph && shownGraph.nodes.length > 0 && !tooLarge ? (
            <NetworkGraph
              graph={shownGraph}
              colors={presentation}
              snapshot={snapshot}
              path={path}
              selection={selection}
              onInspect={setSelection}
              visible={active}
              fitSignal={fitSignal}
            />
          ) : (
            <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-slate-500">
              {tooLarge
                ? `This selection has ${count(shownGraph!.nodes.length)} nodes. Pick a bus neighborhood, island or path to stay within the ${DISPLAY_LIMIT}-node display limit.`
                : activeRules.length && scope?.data.nodes.length
                  ? "Nothing in this view matches the filters."
                  : "No active buses to display."}
            </div>
          )}
          <GraphLegend presentation={presentation} />
          <p className="absolute left-2 bottom-1.5 right-2 text-[11px] text-slate-600 bg-white/80 rounded px-1.5 py-0.5 truncate" data-testid="path-result">
            {pathText}
          </p>
        </Panel>
        <section className="row-span-2 bg-white border border-slate-200 rounded-lg flex flex-col min-h-0 overflow-hidden">
          <Tabs
            tabs={[
              { id: "selection" as const, label: "Selection" },
              { id: "fields" as const, label: "Filters & colours", count: activeRules.length || undefined },
            ]}
            value={side}
            onChange={setSide}
          />
          <div className="flex-1 min-h-0 overflow-y-auto">
          {side === "fields" ? (
            <FieldPanel
              snapshot={snapshot}
              rules={rules}
              onRules={setRules}
              colors={colors}
              onColors={setColors}
              presentation={presentation}
              counts={{
                buses: shownRows.buses.length,
                totalBuses: rows.buses.length,
                equipment: shownRows.equipment.length,
                totalEquipment: rows.equipment.length,
              }}
            />
          ) : (
          <Inspector
            snapshot={snapshot}
            selection={selection}
            onSelect={setSelection}
            onNeighborhood={(bus, hops) => {
              setPath(null);
              setScope({ kind: "neighborhood", title: `Bus ${bus} · ${hops}-hop neighborhood`, data: neighborhood(snapshot.graph, bus, hops) });
            }}
          />
          )}
          </div>
        </section>
        <section className="bg-white border border-slate-200 rounded-lg flex flex-col min-h-0 overflow-hidden">
          <Tabs tabs={tabDefs} value={tab} onChange={(t) => { setTab(t); setSearch(""); }} />
          <div className="flex items-center gap-2 px-3 py-1.5 border-b border-slate-200">
            <SearchBox value={search} onChange={setSearch} placeholder={tab === "buses" ? "Search bus number or name…" : "Search results…"} />
            {tab === "islands" && <span className="text-[11px] text-slate-500">Click an island to show it in the graph.</span>}
            <Button variant="ghost" className="ml-auto" onClick={exportCsv}>
              <Download size={13} /> Export CSV
            </Button>
          </div>
          <div className="flex-1 min-h-0">
            {tab === "buses" && <DataTable key="b" rows={shownRows.buses} columns={busColumns} rowKey={(r) => r.bus} search={search} onRowClick={onRowClick} selectedKey={selectedKey} />}
            {tab === "islands" && <DataTable key="i" rows={rows.islands} columns={islandColumns} rowKey={(r) => r.island} search={search} onRowClick={onRowClick} />}
            {tab === "critical" && <DataTable key="c" rows={rows.critical} columns={criticalColumns} rowKey={(r) => `${r.type}:${r.id}:${r.winding}`} search={search} onRowClick={onRowClick} />}
            {tab === "equipment" && <DataTable key="e" rows={shownRows.equipment} columns={equipmentColumns} rowKey={(r) => r.id} search={search} onRowClick={onRowClick} selectedKey={selectedKey} />}
            {tab === "diagnostics" && <DataTable key="d" rows={rows.diagnostics} columns={diagnosticColumns} rowKey={(r) => r.i} search={search} />}
          </div>
        </section>
      </div>
    </div>
  );
}

// Filters and colour rules are kept per case file, so reopening restores them.
const FIELD_STORAGE = "stannum-topology-fields:";

function isComplete(rule: FilterRule): boolean {
  if (rule.operator === "missing" || rule.operator === "present") return true;
  if (rule.value.trim() === "") return false;
  return rule.operator !== "between" || (rule.upper ?? "").trim() !== "";
}

function loadFieldSettings(path: string | undefined): { rules: FilterRule[]; colors: ColorSettings } {
  try {
    const stored = path ? localStorage.getItem(FIELD_STORAGE + path) : null;
    if (!stored) return { rules: [], colors: defaultColorSettings() };
    const parsed = JSON.parse(stored) as { rules?: unknown; colors?: unknown };
    const rules = Array.isArray(parsed.rules)
      ? (parsed.rules as FilterRule[]).filter((r) => r && typeof r.id === "string" && typeof r.field === "string")
      : [];
    const colors = loadColorSettings({ getItem: () => JSON.stringify(parsed.colors ?? null) });
    return { rules, colors };
  } catch {
    return { rules: [], colors: defaultColorSettings() };
  }
}

function saveFieldSettings(path: string | undefined, rules: FilterRule[], colors: ColorSettings) {
  if (!path) return;
  let safeColors: unknown = null;
  saveColorSettings(colors, { setItem: (_k: string, v: string) => (safeColors = JSON.parse(v)) });
  try {
    localStorage.setItem(FIELD_STORAGE + path, JSON.stringify({ rules, colors: safeColors }));
  } catch {
    // Storage may be unavailable; the rules still apply for this session.
  }
}
