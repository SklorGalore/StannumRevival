// The Diagram's tables drawer: the case's topology as tables (buses,
// islands, critical connections, equipment, diagnostics) under the canvas,
// in either drawing style. Picking a row selects the element everywhere.
import { useMemo, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { Download, Table2, X } from "lucide-react";
import { useWorkspace } from "../../../store/workspaceStore";
import { useLayers } from "../../../store/layerStore";
import { useDrawingStyle } from "../../../store/drawingStyleStore";
import { saveTextExport } from "../../../lib/analysis/api";
import type { TopologySnapshot } from "../../../lib/analysis/types";
import { count, fixed, toCsv } from "../../../lib/analysis/format";
import { busElement } from "../../../lib/elements";
import { Button, Notice, SearchBox, Tabs } from "../../../ui/primitives";
import { DataTable, filterRows, type Column } from "../../../ui/DataTable";
import { busRows, type TopoBusRow } from "../../topology/viewModel";
import { matchesBus, matchesEquipment } from "../../topology/fields/field-view";
import { isComplete } from "../../topology/fields/storage";
import { useSnapshot } from "../network/useSnapshot";

type Tab = "buses" | "islands" | "critical" | "equipment" | "diagnostics";

interface IslandRow { island: number; buses: number; junctions: number; bus_numbers: number[] }
interface CriticalRow { type: string; id: string; from: string; to: string; winding: number | string; bus: number | null }
interface EquipmentRow { id: string; kind: string; circuit: string; buses: number[]; name: string; status: number; topology: string; line: number }
interface DiagRow { i: number; severity: string; section: string; line: number; message: string }

const busColumns: Column<TopoBusRow>[] = [
  { key: "bus", header: "Bus", value: (r) => r.bus, align: "right" },
  { key: "name", header: "Name", value: (r) => r.name },
  { key: "members", header: "Members", value: (r) => r.members.join(" "), render: (r) => (r.members.length > 1 ? r.members.length : ""), align: "right" },
  { key: "base_kv", header: "Base kV", value: (r) => r.base_kv, render: (r) => fixed(r.base_kv, 1), align: "right" },
  { key: "degree", header: "Connections", value: (r) => r.degree, align: "right" },
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
  { key: "line", header: "RAW line", value: (r) => r.line, render: (r) => (r.line > 0 ? r.line : "—"), align: "right" },
];
const diagnosticColumns: Column<DiagRow>[] = [
  { key: "severity", header: "Severity", value: (r) => r.severity },
  { key: "section", header: "Section", value: (r) => r.section },
  { key: "line", header: "Line", value: (r) => r.line, render: (r) => (r.line > 0 ? r.line : "—"), align: "right" },
  { key: "message", header: "Message", value: (r) => r.message },
];

function tableRows(snapshot: TopologySnapshot) {
  const { case: c, analysis: a, graph } = snapshot;
  const inGraph = new Set(graph.edges.map((e) => e.equipment_id));
  const absorbed = new Set(graph.absorbed_equipment_ids);
  return {
    buses: busRows(snapshot),
    islands: a.islands.map((i) => ({ island: i.id, buses: i.buses.length, junctions: i.synthetic_nodes.length, bus_numbers: i.buses })) as IslandRow[],
    critical: [
      ...a.bridges.map((e) => ({ type: "Bridge", id: e.equipment_id, from: e.source, to: e.target, winding: e.winding ?? "", bus: null })),
      ...a.articulation_buses.map((b) => ({ type: "Cut bus", id: String(b), from: `bus:${b}`, to: "", winding: "", bus: b })),
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
    })) as EquipmentRow[],
    diagnostics: c.diagnostics.map((d, i) => ({ i, ...d })) as DiagRow[],
  };
}

/** The "Tables" button in either style's toolbar. */
export function TablesButton({ showLabel = true }: { showLabel?: boolean }) {
  const open = useLayers((s) => s.tables);
  const setTables = useLayers((s) => s.setTables);
  const available = useWorkspace((s) => !!s.caseSummary?.topology.available);
  return (
    <button
      type="button"
      onClick={() => setTables(!open)}
      aria-pressed={open}
      aria-label="Tables"
      disabled={!available}
      title={available ? "Tables: buses, islands, critical connections, equipment and diagnostics" : "Open a case to see its tables"}
      className={`shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium whitespace-nowrap disabled:opacity-40 disabled:cursor-not-allowed ${
        open ? "bg-teal-50 text-teal-800 ring-1 ring-inset ring-teal-200" : "text-slate-600 hover:bg-slate-100"
      }`}
    >
      <Table2 size={14} />
      {showLabel && <span>Tables</span>}
    </button>
  );
}

const MIN_HEIGHT = 140;

/** The tables drawer, under the canvas while it is open. */
export function TablesDrawer() {
  const summary = useWorkspace((s) => s.caseSummary);
  const selection = useWorkspace((s) => s.selection);
  const { snapshot, error } = useSnapshot(true);
  const setTables = useLayers((s) => s.setTables);
  const rules = useLayers((s) => s.rules);
  const fieldsOn = useLayers((s) => s.on.fields);
  const [tab, setTab] = useState<Tab>("buses");
  const [search, setSearch] = useState("");
  const [height, setHeight] = useState(280);

  const rows = useMemo(() => (snapshot ? tableRows(snapshot) : null), [snapshot]);
  // The Filters layer narrows the bus and equipment tables too.
  const active = useMemo(() => (fieldsOn ? rules.filter(isComplete) : []), [fieldsOn, rules]);
  const shown = useMemo(() => {
    if (!rows || !snapshot || !active.length) return rows;
    return {
      ...rows,
      buses: rows.buses.filter((r) => matchesBus(snapshot, r.bus, active)),
      equipment: rows.equipment.filter((r) => matchesEquipment(snapshot, r.id, active)),
    };
  }, [rows, snapshot, active]);

  function resize(ev: ReactPointerEvent<HTMLDivElement>) {
    const startY = ev.clientY;
    const start = height;
    const max = (ev.currentTarget.parentElement?.parentElement?.clientHeight ?? 800) * 0.75;
    const move = (e: PointerEvent) => setHeight(Math.round(Math.min(max, Math.max(MIN_HEIGHT, start + startY - e.clientY))));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  let body: ReactNode;
  if (!summary) body = <p className="p-4 text-xs text-slate-500">Open a case to see its tables.</p>;
  else if (!summary.topology.available) body = <p className="p-4 text-xs text-slate-500">The case's topology couldn't be built: {summary.topology.error}</p>;
  else if (error) body = <p className="p-4 text-xs text-red-700">{error}</p>;
  else if (!snapshot || !rows || !shown) body = <p className="p-4 text-xs text-slate-500">Building the topology…</p>;
  else {
    const { case: c, analysis: a } = snapshot;
    // Built from the power flow model: nodal reads RAW 35, anything else is revision 0.
    const derived = c.revision === 0;
    const warnings = c.diagnostics.filter((d) => d.severity === "warning");
    const tabs = [
      { id: "buses" as const, label: "Buses", count: shown.buses.length },
      { id: "islands" as const, label: "Islands", count: rows.islands.length },
      { id: "critical" as const, label: "Critical connections", count: rows.critical.length },
      { id: "equipment" as const, label: "Equipment", count: shown.equipment.length },
      { id: "diagnostics" as const, label: "Diagnostics", count: rows.diagnostics.length },
    ];
    const pick = (element: string) => useWorkspace.getState().focus(element);
    const onRowClick = (row: unknown) => {
      if (tab === "buses") pick(busElement((row as TopoBusRow).bus));
      else if (tab === "equipment") pick((row as EquipmentRow).id);
      else if (tab === "critical") {
        const r = row as CriticalRow;
        pick(r.bus !== null ? busElement(r.bus) : r.id);
      } else if (tab === "islands") useDrawingStyle.getState().showIsland((row as IslandRow).island);
    };
    const exportCsv = async () => {
      const [data, columns] = {
        buses: [shown.buses, busColumns],
        islands: [rows.islands, islandColumns],
        critical: [rows.critical, criticalColumns],
        equipment: [shown.equipment, equipmentColumns],
        diagnostics: [rows.diagnostics, diagnosticColumns],
      }[tab] as [unknown[], Column<unknown>[]];
      const csv = toCsv(filterRows(data, columns, search).map((r) => Object.fromEntries(columns.map((col) => [col.key, col.value(r)]))));
      await saveTextExport(`${summary.name.replace(/\.raw$/i, "")}-${tab}.csv`, csv, { name: "CSV", extensions: ["csv"] });
    };
    const exportJson = async () => {
      const path = useLayers.getState().path?.result ?? null;
      const json = JSON.stringify({ format_version: 1, filename: summary.name, case: c, graph: snapshot.graph, analysis: a, path }, null, 2);
      await saveTextExport(`${summary.name.replace(/\.raw$/i, "")}-topology.json`, json, { name: "JSON", extensions: ["json"] });
    };
    const selected = selection?.element ?? null;
    const selectedKey =
      tab === "buses" && selected?.startsWith("bus:") ? Number(selected.slice(4)) : tab === "equipment" ? (selected ?? undefined) : undefined;
    body = (
      <>
        <Tabs tabs={tabs} value={tab} onChange={(t) => { setTab(t); setSearch(""); }} />
        <div className="flex items-center gap-2 px-3 py-1.5 border-b border-slate-200">
          <SearchBox value={search} onChange={setSearch} placeholder={tab === "buses" ? "Search bus number or name…" : "Search…"} />
          <span className="hidden @3xl:inline truncate text-[11px] text-slate-500">
            {tab === "islands"
              ? "Pick an island to show it in the network."
              : `${count(c.buses.length)} buses · ${count(a.islands.length)} island${a.islands.length === 1 ? "" : "s"} · ${count(a.bridges.length)} bridges · ${count(a.articulation_buses.length)} cut buses${active.length ? " · filtered" : ""}`}
          </span>
          <Button variant="ghost" className="ml-auto" onClick={exportCsv}>
            <Download size={13} /> CSV
          </Button>
          <Button variant="ghost" onClick={exportJson} title="Export the whole topology as JSON">
            <Download size={13} /> JSON
          </Button>
        </div>
        {(derived || warnings.length > 0) && tab === "diagnostics" && (
          <div className="px-3 pt-2">
            <Notice>
              {derived && (
                <>
                  <strong>Built from the power flow model.</strong> {summary.format === "matpower" ? "This MATPOWER case" : "This RAW revision"} isn't read by
                  nodal directly, so switching devices and the original RAW records aren't shown, and loads are summed per bus.{" "}
                </>
              )}
              {warnings.length > 0 && <>Topology coverage is limited: {warnings.map((d) => `${d.message} (${d.section})`).join(" · ")}</>}
            </Notice>
          </div>
        )}
        <div className="flex-1 min-h-0">
          {tab === "buses" && <DataTable key="b" rows={shown.buses} columns={busColumns} rowKey={(r) => r.bus} search={search} onRowClick={onRowClick} selectedKey={selectedKey} />}
          {tab === "islands" && <DataTable key="i" rows={rows.islands} columns={islandColumns} rowKey={(r) => r.island} search={search} onRowClick={onRowClick} />}
          {tab === "critical" && <DataTable key="c" rows={rows.critical} columns={criticalColumns} rowKey={(r) => `${r.type}:${r.id}:${r.winding}`} search={search} onRowClick={onRowClick} />}
          {tab === "equipment" && <DataTable key="e" rows={shown.equipment} columns={equipmentColumns} rowKey={(r) => r.id} search={search} onRowClick={onRowClick} selectedKey={selectedKey} />}
          {tab === "diagnostics" && <DataTable key="d" rows={rows.diagnostics} columns={diagnosticColumns} rowKey={(r) => r.i} search={search} />}
        </div>
      </>
    );
  }

  return (
    <section
      className="@container relative shrink-0 flex flex-col border-t border-slate-200 bg-white"
      style={{ height }}
      data-testid="tables-drawer"
      aria-label="Tables"
    >
      <div
        className="absolute inset-x-0 -top-1 h-2 cursor-row-resize z-10"
        onPointerDown={resize}
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize tables"
        title="Drag to resize"
      />
      <button
        className="absolute right-1.5 top-1.5 z-10 p-1 rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700"
        onClick={() => setTables(false)}
        aria-label="Hide tables"
        title="Hide tables"
      >
        <X size={14} />
      </button>
      <div className="flex-1 min-h-0 flex flex-col pr-8">{body}</div>
    </section>
  );
}
