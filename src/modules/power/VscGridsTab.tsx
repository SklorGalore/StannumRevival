import { FileJson, Unplug } from "lucide-react";
import type {
  DcGridsSummary,
  SolveReport,
  VscGridBusRow,
  VscGridConverterRow,
  VscGridLineRow,
  VscGridReport,
} from "../../lib/analysis/types";
import { count, fixed } from "../../lib/analysis/format";
import { Button, Metric, Notice, Panel } from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";

const DC_CONTROL: Record<VscGridConverterRow["dc_control"], string> = {
  slack: "DC slack",
  power: "Power",
  droop: "Droop",
};

const AC_CONTROL: Record<VscGridConverterRow["ac_control"], string> = {
  reactive: "Q",
  voltage: "AC voltage",
  reactive_limit: "Q at limit",
};

function flags(c: VscGridConverterRow): string {
  if (!c.in_service) return "Out of service";
  const out = [];
  if (c.p_limited) out.push("P reduced to limit");
  if (c.outside_limits) out.push("Outside limits");
  return out.join(" · ");
}

const converterColumns: Column<VscGridConverterRow>[] = [
  { key: "name", header: "Converter", value: (c) => c.name },
  { key: "ac_bus", header: "AC bus", value: (c) => c.ac_bus, align: "right" },
  { key: "ac_bus_name", header: "Bus name", value: (c) => c.ac_bus_name },
  { key: "dc_bus", header: "DC bus", value: (c) => c.dc_bus, align: "right" },
  { key: "dc_control", header: "DC control", value: (c) => DC_CONTROL[c.dc_control] },
  { key: "ac_control", header: "AC control", value: (c) => AC_CONTROL[c.ac_control] },
  {
    key: "ps",
    header: "Ps (MW)",
    title: "Active power into the AC grid at the system bus; negative means taken from it",
    value: (c) => c.ps_mw,
    render: (c) => fixed(c.ps_mw),
    align: "right",
  },
  { key: "qs", header: "Qs (MVAr)", title: "Reactive power into the AC grid", value: (c) => c.qs_mvar, render: (c) => fixed(c.qs_mvar), align: "right" },
  { key: "us", header: "Us (p.u.)", value: (c) => c.us_pu, render: (c) => fixed(c.us_pu, 4), align: "right" },
  { key: "uc", header: "Uc (p.u.)", title: "Converter AC voltage", value: (c) => c.uc_pu, render: (c) => fixed(c.uc_pu, 4), align: "right" },
  { key: "ic", header: "Ic (kA)", title: "Converter current", value: (c) => c.current_ka, render: (c) => fixed(c.current_ka, 4), align: "right" },
  { key: "loss", header: "Loss (MW)", value: (c) => c.loss_mw, render: (c) => fixed(c.loss_mw, 3), align: "right" },
  {
    key: "pdc",
    header: "Pdc (MW)",
    title: "Power into the DC grid; negative means taken from it",
    value: (c) => c.pdc_mw,
    render: (c) => fixed(c.pdc_mw),
    align: "right",
  },
  { key: "udc", header: "Udc (kV)", value: (c) => c.udc_kv, render: (c) => fixed(c.udc_kv, 3), align: "right" },
  {
    key: "qrange",
    header: "Q range (MVAr)",
    title: "Reactive range the converter's current and voltage limits allow at this operating point",
    value: (c) => c.q_max_mvar ?? Infinity,
    render: (c) => (c.q_min_mvar === null && c.q_max_mvar === null ? "" : `${fixed(c.q_min_mvar)} to ${fixed(c.q_max_mvar)}`),
    align: "right",
  },
  {
    key: "flags",
    header: "Limits",
    value: flags,
    render: (c) => <span className={c.outside_limits || c.p_limited ? "text-amber-700 font-medium" : "text-slate-500"}>{flags(c)}</span>,
  },
];

const busColumns: Column<VscGridBusRow>[] = [
  { key: "number", header: "DC bus", value: (b) => b.number, align: "right" },
  { key: "name", header: "Name", value: (b) => b.name },
  { key: "v_pu", header: "U (p.u.)", value: (b) => b.v_pu, render: (b) => fixed(b.v_pu, 5), align: "right" },
  { key: "kv", header: "U (kV)", title: "Pole-to-ground voltage", value: (b) => b.kv, render: (b) => fixed(b.kv, 3), align: "right" },
  {
    key: "injection",
    header: "Converters (MW)",
    title: "Power the bus's converters put into the DC grid",
    value: (b) => b.injection_mw,
    render: (b) => fixed(b.injection_mw),
    align: "right",
  },
];

const lineColumns: Column<VscGridLineRow>[] = [
  { key: "from", header: "From", value: (l) => l.from, align: "right" },
  { key: "to", header: "To", value: (l) => l.to, align: "right" },
  { key: "name", header: "Name", value: (l) => (l.in_service ? l.name : `${l.name} (out of service)`) },
  { key: "current", header: "I (kA)", title: "Current per pole from the from bus to the to bus", value: (l) => l.current_ka, render: (l) => fixed(l.current_ka, 4), align: "right" },
  { key: "from_mw", header: "P from (MW)", value: (l) => l.from_mw, render: (l) => fixed(l.from_mw), align: "right" },
  { key: "to_mw", header: "P to (MW)", value: (l) => l.to_mw, render: (l) => fixed(l.to_mw), align: "right" },
  { key: "loss", header: "Loss (MW)", value: (l) => l.loss_mw, render: (l) => fixed(l.loss_mw, 3), align: "right" },
];

function GridPanel({ grid }: { grid: VscGridReport }) {
  return (
    <Panel
      title={grid.name}
      eyebrow={`${grid.poles === 2 ? "±" : ""}${fixed(grid.base_kv, 0)} kV · ${grid.poles === 2 ? "two poles" : "one pole"} · line loss ${fixed(
        grid.line_loss_mw,
        3
      )} MW · converter loss ${fixed(grid.converter_loss_mw, 3)} MW`}
    >
      <div className="space-y-3 p-3" data-testid="vsc-grid">
        <div>
          <h3 className="text-xs font-semibold text-slate-600 mb-1">Converters</h3>
          <DataTable
            rows={grid.converters}
            columns={converterColumns}
            rowKey={(c) => `${c.name}-${c.ac_bus}-${c.dc_bus}`}
            rowClassName={(c) => (!c.in_service ? "text-slate-500" : c.outside_limits ? "bg-amber-50/60" : "")}
          />
        </div>
        <div className="grid gap-3 lg:grid-cols-2">
          <div>
            <h3 className="text-xs font-semibold text-slate-600 mb-1">DC buses</h3>
            <DataTable rows={grid.buses} columns={busColumns} rowKey={(b) => b.number} />
          </div>
          <div>
            <h3 className="text-xs font-semibold text-slate-600 mb-1">DC lines</h3>
            <DataTable
              rows={grid.lines}
              columns={lineColumns}
              rowKey={(l) => grid.lines.indexOf(l)}
              rowClassName={(l) => (l.in_service ? "" : "text-slate-500")}
            />
          </div>
        </div>
      </div>
    </Panel>
  );
}

/**
 * Meshed and multi-terminal VSC DC grids from a Stannum DC grid file,
 * solved in sequence with the AC case.
 */
export function VscGridsTab({
  file,
  report,
  busy,
  onLoad,
  onClear,
}: {
  file: DcGridsSummary | null;
  report: SolveReport | null;
  busy: boolean;
  onLoad: () => void;
  onClear: () => void;
}) {
  const vsc = report?.vsc_grids ?? null;
  const converters = file?.grids.reduce((n, g) => n + g.converters, 0) ?? 0;
  const lost = vsc?.grids.reduce((n, g) => n + g.line_loss_mw + g.converter_loss_mw, 0) ?? 0;
  return (
    <div className="h-full overflow-auto p-3 space-y-3 bg-slate-50" data-testid="vsc-grids">
      <Panel
        title="DC grid file"
        actions={
          <div className="flex gap-2">
            <Button onClick={onLoad} disabled={busy}>
              <FileJson size={14} /> {file ? "Replace…" : "Attach DC grid file…"}
            </Button>
            {file && (
              <Button variant="ghost" onClick={onClear} disabled={busy}>
                <Unplug size={14} /> Detach
              </Button>
            )}
          </div>
        }
      >
        <div className="p-3 space-y-3 text-xs text-slate-600">
          {!file ? (
            <p className="max-w-2xl">
              Attach a Stannum DC grid file (<code>.dcgrid.json</code>) to solve meshed and multi-terminal VSC DC grids
              with this case. Converters connect to AC buses by number and can hold a DC voltage (slack), a power order or
              a voltage droop on their DC side, and a reactive power or AC voltage on their AC side. Each AC solve then
              alternates with a DC grid solve until the converters settle.
            </p>
          ) : (
            <>
              <p>
                <span className="font-medium text-slate-800" data-testid="dc-grid-file">
                  {file.name}
                </span>{" "}
                · {count(file.grids.length)} DC grid{file.grids.length === 1 ? "" : "s"} · {count(converters)} converter
                {converters === 1 ? "" : "s"}
              </p>
              {vsc ? (
                <div className="grid gap-2 sm:grid-cols-3 max-w-3xl">
                  <Metric
                    label="AC/DC iterations"
                    value={vsc.iterations}
                    detail={`${count(vsc.ac_iterations)} AC Newton steps in all`}
                    tone={vsc.converged ? "good" : "bad"}
                  />
                  <Metric label="Converters settled to" value={`${fixed(vsc.last_change_mva, 4)} MVA`} detail="largest change in the last iteration" />
                  <Metric label="DC grid losses" value={`${fixed(lost, 2)} MW`} detail="converters and DC lines" />
                </div>
              ) : (
                <Notice>
                  {report?.model === "dc"
                    ? "The DC (linear) model leaves the DC grids out. Solve with AC to include them."
                    : "Solve to include these DC grids."}
                </Notice>
              )}
              {vsc && !vsc.converged && (
                <Notice>The converters had not settled when the AC/DC iterations stopped. Treat these values with care.</Notice>
              )}
            </>
          )}
        </div>
      </Panel>
      {vsc
        ? vsc.grids.map((g) => <GridPanel key={g.name} grid={g} />)
        : file && (
            <Panel title="Grids in the file">
              <div className="p-3">
                <DataTable
                  rows={file.grids}
                  rowKey={(g) => g.name}
                  columns={[
                    { key: "name", header: "Grid", value: (g) => g.name },
                    { key: "kv", header: "Base kV", value: (g) => g.base_kv, align: "right" },
                    { key: "poles", header: "Poles", value: (g) => g.poles, align: "right" },
                    { key: "buses", header: "DC buses", value: (g) => g.buses, align: "right" },
                    { key: "lines", header: "DC lines", value: (g) => g.lines, align: "right" },
                    { key: "converters", header: "Converters", value: (g) => g.converters, align: "right" },
                  ]}
                />
              </div>
            </Panel>
          )}
    </div>
  );
}
