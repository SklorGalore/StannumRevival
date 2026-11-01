// Transfer limits (DFAX and FCITC): how much power can move from a source
// area, zone, owner or list of buses to a sink before a branch reaches its
// limit, in the base case and after each branch outage. The study is
// stannum-core's (transfer.rs); this sets it up and shows what it found.
import { useEffect, useMemo, useState } from "react";
import {
  ArrowRightLeft,
  Download,
  FileText,
  Play,
  Square,
  X,
} from "lucide-react";
import {
  useTransfers,
  type Side,
  type SideKind,
  busList,
  transferSettings,
} from "../../store/transferStore";
import {
  LISTS_FILTERS,
  pickFile,
  saveCsvExport,
} from "../../lib/analysis/api";
import { useCaseGroups, type CaseGroups } from "../../ui/useCaseGroups";
import type { SolveReport } from "../../lib/analysis/types";
import type {
  ContingencyLimit,
  Scaling,
  TransferFactor,
  TransferLimit,
} from "../../lib/analysis/commands.gen";
import { branchParts, count, fixed, toCsv } from "../../lib/analysis/format";
import { elementLabel } from "../../lib/elements";
import { RATINGS, ratingLabel, type Rating } from "../../lib/analysis/ratings";
import {
  Button,
  EmptyState,
  ErrorBanner,
  Metric,
  Notice,
  SearchBox,
  Tabs,
  inputClass,
} from "../../ui/primitives";
import { DataTable, type Column } from "../../ui/DataTable";
import { ElementLink } from "../../ui/DrawAround";

const SCALINGS: { value: Scaling; label: string; hint: string }[] = [
  {
    value: "generation",
    label: "Generation output",
    hint: "Online generators move in proportion to their MW now",
  },
  {
    value: "reserve",
    label: "Generation reserve",
    hint: "Online generators move in proportion to their room to Pmax (source) or Pmin (sink)",
  },
  {
    value: "load",
    label: "Load",
    hint: "The source's loads drop and the sink's grow, in proportion to their MW",
  },
];

const KINDS: { value: SideKind; label: string }[] = [
  { value: "area", label: "Area" },
  { value: "zone", label: "Zone" },
  { value: "owner", label: "Owner" },
  { value: "buses", label: "Buses" },
];

type View = "limits" | "past" | "contingencies" | "factors";

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function pct(v: number): string {
  return `${fixed(v * 100, 1)}%`;
}

function mw(v: number | null | undefined): string {
  return v === null || v === undefined ? "—" : `${fixed(v, 1)} MW`;
}

/** What is out, by name: a lists file's name or PSS/E style bus names and numbers. */
function contingencyLabel(r: {
  contingency: string;
  contingency_name?: string;
}): string {
  if (!r.contingency) return "Base case";
  return r.contingency_name || elementLabel(r.contingency);
}

/** The monitored element by name (PSS/E style), or the interface. */
function monitoredLabel(
  r: { monitored: string; monitored_name?: string; winding?: number },
  withWinding = true,
): string {
  if (r.monitored.startsWith("interface:"))
    return `Interface ${r.monitored.slice("interface:".length)}`;
  const name = r.monitored_name || elementLabel(r.monitored);
  return withWinding && r.winding ? `${name} (winding ${r.winding})` : name;
}

function SidePicker({
  label,
  side,
  onChange,
  groups,
  areaNames,
  testId,
}: {
  label: string;
  side: Side;
  onChange: (s: Side) => void;
  groups: CaseGroups | null;
  areaNames: Map<number, string>;
  testId: string;
}) {
  const kinds = KINDS.filter(
    (k) =>
      k.value === "buses" || (groups?.[k.value as "area"]?.length ?? 0) > 0,
  );
  const options = side.kind === "buses" ? [] : (groups?.[side.kind] ?? []);
  return (
    <fieldset className="flex flex-col gap-1" data-testid={testId}>
      <legend className="text-[11px] font-medium text-slate-600 mb-1">
        {label}
      </legend>
      <div className="flex items-center gap-1">
        <select
          className={inputClass}
          value={side.kind}
          aria-label={`${label}: by`}
          onChange={(e) => {
            const kind = e.target.value as SideKind;
            const first =
              kind === "buses" ? 0 : (groups?.[kind]?.[0]?.[0] ?? 0);
            onChange({ ...side, kind, value: first });
          }}
        >
          {kinds.map((k) => (
            <option key={k.value} value={k.value}>
              {k.label}
            </option>
          ))}
        </select>
        {side.kind === "buses" ? (
          <input
            className={`${inputClass} w-40`}
            value={side.buses}
            placeholder="101, 102, 205"
            aria-label={`${label}: bus numbers`}
            onChange={(e) => onChange({ ...side, buses: e.target.value })}
          />
        ) : (
          <select
            className={`${inputClass} max-w-52`}
            value={side.value}
            aria-label={`${label}: ${side.kind}`}
            onChange={(e) =>
              onChange({ ...side, value: Number(e.target.value) })
            }
          >
            {options.map(([v, n]) => {
              const name = side.kind === "area" ? areaNames.get(v) : undefined;
              return (
                <option key={v} value={v}>
                  {v}
                  {name ? ` ${name}` : ""} ({count(n)} buses)
                </option>
              );
            })}
          </select>
        )}
      </div>
    </fieldset>
  );
}

function limitColumns(
  rating: Rating,
  baseRating: Rating,
  onPick: (e: string) => void,
): Column<TransferLimit>[] {
  return [
    {
      key: "fcitc",
      header: "FCITC (MW)",
      title: "The transfer that brings the element to its limit",
      value: (r) => r.fcitc_mw,
      render: (r) => (
        <span
          className={`font-semibold ${r.fcitc_mw < 0 ? "text-red-700" : "text-slate-800"}`}
        >
          {fixed(r.fcitc_mw, 1)}
        </span>
      ),
      align: "right",
    },
    {
      key: "monitored",
      header: "Limiting element",
      value: (r) => monitoredLabel(r),
      render: (r) =>
        r.monitored.startsWith("interface:") ? (
          monitoredLabel(r)
        ) : (
          <ElementLink
            element={r.monitored}
            from="power"
            onOpen={() => onPick(r.monitored)}
          >
            {monitoredLabel(r)}
          </ElementLink>
        ),
    },
    {
      key: "contingency",
      header: "After outage of",
      value: (r) => contingencyLabel(r),
      render: (r) =>
        r.contingency ? (
          <span className="text-slate-700">{contingencyLabel(r)}</span>
        ) : (
          <span className="text-slate-500">Base case</span>
        ),
    },
    {
      key: "flow",
      header: "Flow (MW)",
      title: "MW at the from end before the transfer, after the outage",
      value: (r) => r.flow_mw,
      render: (r) => fixed(r.flow_mw, 1),
      align: "right",
    },
    {
      key: "limit",
      header: "Limit (MW)",
      title: `The rating (${ratingLabel(rating)} after an outage, ${ratingLabel(baseRating)} in the base case) times the limit percent`,
      value: (r) => r.limit_mw,
      render: (r) => fixed(r.limit_mw, 1),
      align: "right",
    },
    {
      key: "factor",
      header: "OTDF",
      title:
        "MW on the element per MW of transfer: the PTDF in the base case, the OTDF after an outage",
      value: (r) => r.factor,
      render: (r) => pct(r.factor),
      align: "right",
    },
    {
      key: "ptdf",
      header: "PTDF",
      title: "Base case MW on the element per MW of transfer",
      value: (r) => r.ptdf,
      render: (r) => <span className="text-slate-500">{pct(r.ptdf)}</span>,
      align: "right",
    },
  ];
}

const contingencyColumns: Column<ContingencyLimit>[] = [
  {
    key: "fcitc",
    header: "FCITC (MW)",
    value: (r) => r.fcitc_mw,
    render: (r) => (
      <span className="font-semibold">{fixed(r.fcitc_mw, 1)}</span>
    ),
    align: "right",
  },
  { key: "contingency", header: "Outage", value: (r) => contingencyLabel(r) },
  {
    key: "monitored",
    header: "Limiting element",
    value: (r) => monitoredLabel(r),
  },
  {
    key: "factor",
    header: "OTDF",
    value: (r) => r.factor,
    render: (r) => pct(r.factor),
    align: "right",
  },
];

function factorColumns(onPick: (e: string) => void): Column<TransferFactor>[] {
  return [
    {
      key: "ptdf",
      header: "PTDF",
      title: "MW on the element per MW of transfer, in the base case",
      value: (r) => r.ptdf,
      render: (r) => <span className="font-semibold">{pct(r.ptdf)}</span>,
      align: "right",
    },
    {
      key: "monitored",
      header: "Element",
      value: (r) => monitoredLabel(r),
      render: (r) =>
        r.monitored.startsWith("interface:") ? (
          monitoredLabel(r)
        ) : (
          <ElementLink
            element={r.monitored}
            from="power"
            onOpen={() => onPick(r.monitored)}
          >
            {monitoredLabel(r)}
          </ElementLink>
        ),
    },
    {
      key: "flow",
      header: "Flow (MW)",
      value: (r) => r.flow_mw,
      render: (r) => fixed(r.flow_mw, 1),
      align: "right",
    },
    {
      key: "limit",
      header: "Limit (MW)",
      value: (r) => r.limit_mw,
      render: (r) => (r.limit_mw ? fixed(r.limit_mw, 1) : "Unrated"),
      align: "right",
    },
  ];
}

export function TransferTab({
  report,
  onPick,
}: {
  report: SolveReport | null;
  onPick: (element: string) => void;
}) {
  const s = useTransfers();
  const c = s.choice;
  const groups = useCaseGroups();
  const areaNames = useMemo(
    () => new Map((report?.areas ?? []).map((a) => [a.number, a.name.trim()])),
    [report],
  );
  const [view, setView] = useState<View>("limits");
  const [search, setSearch] = useState("");

  // Start with two different areas (or zones) once the case's groups are known.
  useEffect(() => {
    if (!groups) return;
    const pick = (kind: "area" | "zone") => groups[kind].map(([v]) => v);
    const kind =
      pick("area").length > 1
        ? "area"
        : pick("zone").length > 1
          ? "zone"
          : "area";
    const values = pick(kind);
    const known = (side: Side) =>
      side.kind === "buses" ||
      (groups[side.kind]?.some(([v]) => v === side.value) ?? false);
    if (known(c.source) && known(c.sink) && c.source.value !== c.sink.value)
      return;
    s.setChoice({
      source: { kind, value: values[0] ?? 0, buses: "" },
      sink: { kind, value: values[values.length - 1] ?? 0, buses: "" },
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [groups]);

  const r = s.results;
  const sum = r?.summary;
  const acReady = !!report && report.model === "ac" && report.converged;
  const sameSide =
    c.source.kind === c.sink.kind &&
    (c.source.kind === "buses"
      ? c.source.buses.trim() === c.sink.buses.trim()
      : c.source.value === c.sink.value);
  const noBuses = (side: Side) =>
    side.kind === "buses" && busList(side.buses).length === 0;
  const blocked = !acReady
    ? "Solve the case with the AC model first."
    : c.listsPath
      ? null
      : sameSide
        ? "Pick a sink different from the source."
        : noBuses(c.source) || noBuses(c.sink)
          ? "List the buses of each side."
          : null;
  const progressPct =
    s.progress && s.progress.total
      ? (100 * s.progress.done) / s.progress.total
      : 0;
  const shownRating = sum?.rating ?? c.rating;
  const shownBase = sum?.base_rating ?? c.baseRating;
  const limitCols = useMemo(
    () => limitColumns(shownRating, shownBase, onPick),
    [shownRating, shownBase, onPick],
  );
  const factorCols = useMemo(() => factorColumns(onPick), [onPick]);

  async function exportCsv() {
    if (!r) return;
    const csv = toCsv(
      r.limits.map((l) => ({
        fcitc_mw: l.fcitc_mw,
        monitored: l.monitored_name,
        monitored_id: l.monitored,
        monitored_from_bus: branchParts(l.monitored).from_bus,
        monitored_to_bus: branchParts(l.monitored).to_bus,
        monitored_circuit: branchParts(l.monitored).circuit,
        winding: l.winding,
        contingency: l.contingency ? l.contingency_name : "BASE CASE",
        contingency_id: l.contingency,
        flow_mw: l.flow_mw,
        limit_mw: l.limit_mw,
        otdf: l.factor,
        ptdf: l.ptdf,
      })),
    );
    await saveCsvExport(
      "transfer-limits.csv",
      "Transfer limits",
      {
        ...transferSettings(c),
        fcitc_mw: r.summary.fcitc_mw == null ? null : Number(r.summary.fcitc_mw.toFixed(1)),
      },
      csv,
    );
  }

  const kv = (
    value: number,
    onChange: (v: number) => void,
    aria: string,
    title: string,
  ) => (
    <input
      className={`${inputClass} w-16`}
      type="number"
      min={0}
      step="any"
      value={value}
      aria-label={aria}
      title={title}
      onChange={(e) => onChange(Math.max(0, Number(e.target.value) || 0))}
    />
  );

  return (
    <div className="h-full flex flex-col" data-testid="transfer-tab">
      <form
        className="flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2 border-b border-slate-200 bg-white text-xs"
        onSubmit={(e) => {
          e.preventDefault();
          if (!blocked && !s.running) void s.run();
        }}
      >
        <SidePicker
          label="From (source)"
          side={c.source}
          onChange={(source) => s.setChoice({ source })}
          groups={groups}
          areaNames={areaNames}
          testId="transfer-source"
        />
        <ArrowRightLeft size={14} className="mb-2 text-slate-400" aria-hidden />
        <SidePicker
          label="To (sink)"
          side={c.sink}
          onChange={(sink) => s.setChoice({ sink })}
          groups={groups}
          areaNames={areaNames}
          testId="transfer-sink"
        />
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Shared out by
          <select
            className={inputClass}
            value={c.scaling}
            onChange={(e) =>
              s.setChoice({ scaling: e.target.value as Scaling })
            }
            title={SCALINGS.find((x) => x.value === c.scaling)?.hint}
            aria-label="Shared out by"
          >
            {SCALINGS.map((x) => (
              <option key={x.value} value={x.value} title={x.hint}>
                {x.label}
              </option>
            ))}
          </select>
        </label>
        <fieldset className="flex flex-col gap-1">
          <legend className="text-[11px] font-medium text-slate-600 mb-1">
            Outages
          </legend>
          <div className="flex items-center gap-2">
            <select
              className={inputClass}
              value={c.outages ? (c.generators ? "both" : "branches") : "none"}
              onChange={(e) =>
                s.setChoice({
                  outages: e.target.value !== "none",
                  generators: e.target.value === "both",
                })
              }
              aria-label="Outages"
              data-testid="transfer-outages"
            >
              <option value="none">Base case only</option>
              <option value="branches">Each branch outage</option>
              <option value="both">Each branch and generator outage</option>
            </select>
            {c.outages && (
              <>
                <span className="text-[11px] text-slate-500">at or above</span>
                {kv(
                  c.outageKvMin,
                  (outageKvMin) => s.setChoice({ outageKvMin }),
                  "Outages at or above, kV",
                  "Only branch outages with a terminal at or above this voltage",
                )}
                <span className="text-[11px] text-slate-500">kV</span>
              </>
            )}
          </div>
        </fieldset>
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Monitor at or above (kV)
          {kv(
            c.monitorKvMin,
            (monitorKvMin) => s.setChoice({ monitorKvMin }),
            "Monitor at or above, kV",
            "Rated branches with a terminal at or above this voltage are monitored",
          )}
        </label>
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Cutoff (%)
          <input
            className={`${inputClass} w-16`}
            type="number"
            min={0}
            step="any"
            value={c.cutoffPct}
            onChange={(e) =>
              s.setChoice({
                cutoffPct: Math.max(0, Number(e.target.value) || 0),
              })
            }
            aria-label="Distribution factor cutoff, percent"
            title="Elements whose distribution factor is smaller than this don't limit the transfer"
          />
        </label>
        <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
          Rating
          <span className="flex items-center gap-1">
            <select
              className={inputClass}
              value={c.baseRating}
              onChange={(e) =>
                s.setChoice({ baseRating: e.target.value as Rating })
              }
              aria-label="Rating in the base case"
              title="The rating in the base case"
            >
              {RATINGS.map((x) => (
                <option key={x.value} value={x.value}>
                  {x.label} before
                </option>
              ))}
            </select>
            <select
              className={inputClass}
              value={c.rating}
              onChange={(e) =>
                s.setChoice({ rating: e.target.value as Rating })
              }
              aria-label="Rating after an outage"
              title="The rating after an outage"
            >
              {RATINGS.map((x) => (
                <option key={x.value} value={x.value}>
                  {x.label} after
                </option>
              ))}
            </select>
          </span>
        </label>
        <fieldset
          className="flex flex-col gap-1 min-w-0"
          data-testid="transfer-lists"
        >
          <legend
            className="text-[11px] font-medium text-slate-600 mb-1"
            title="A lists file's contingencies and monitors replace the outages and monitored elements here, and its transfer record the source and sink"
          >
            Lists file (optional)
          </legend>
          <div className="flex items-center gap-2 min-w-0">
            <Button
              type="button"
              onClick={async () => {
                const path = await pickFile(LISTS_FILTERS);
                if (path) s.setChoice({ listsPath: path });
              }}
              data-testid="transfer-choose-lists"
            >
              <FileText size={13} />{" "}
              {c.listsPath ? fileName(c.listsPath) : "Choose…"}
            </Button>
            {c.listsPath && (
              <>
                <input
                  className={`${inputClass} w-32`}
                  value={c.transferName}
                  placeholder="Its only transfer"
                  aria-label="Transfer in the lists file"
                  title="The transfer record to run; blank for the file's only one, or the source and sink here when it has none"
                  onChange={(e) =>
                    s.setChoice({ transferName: e.target.value })
                  }
                />
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() =>
                    s.setChoice({ listsPath: null, transferName: "" })
                  }
                  title="Stop using the lists file"
                  aria-label="Stop using the lists file"
                >
                  <X size={13} />
                </Button>
              </>
            )}
          </div>
        </fieldset>
        <label
          className="flex items-center gap-1.5 mb-1.5"
          title="Let elements already past their limit before any transfer set the FCITC (negative), as PSS/E reports it"
        >
          <input
            type="checkbox"
            checked={c.includePast}
            onChange={(e) => s.setChoice({ includePast: e.target.checked })}
          />
          Count elements already past their limit
        </label>
        <label
          className="flex items-center gap-1.5 mb-1.5"
          title="Elements loaded at or above this percent of their limit before any transfer, and pushed further by it, are listed apart instead of setting a near-zero FCITC"
        >
          Already past from
          <input
            className={`${inputClass} w-16`}
            type="number"
            min={0}
            max={100}
            step={1}
            value={c.pastPct}
            aria-label="Already past from, percent of the limit"
            data-testid="transfer-past-pct"
            onChange={(e) => {
              const v = Number(e.target.value);
              if (Number.isFinite(v) && v > 0 && v <= 100)
                s.setChoice({ pastPct: v });
            }}
          />
          %
        </label>
        <div className="flex items-center gap-2 ml-auto">
          {s.running ? (
            <Button
              type="button"
              onClick={s.cancel}
              data-testid="transfer-cancel"
            >
              <Square size={13} /> Cancel
            </Button>
          ) : (
            <Button
              type="submit"
              variant="primary"
              disabled={!!blocked}
              title={blocked ?? undefined}
              data-testid="transfer-run"
            >
              <Play size={13} /> Find the transfer limit
            </Button>
          )}
        </div>
      </form>
      {s.running && (
        <div
          className="h-1 bg-slate-100"
          role="progressbar"
          aria-valuenow={Math.round(progressPct)}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <div
            className="h-full bg-teal-600 transition-all"
            style={{ width: `${progressPct}%` }}
          />
        </div>
      )}
      {s.error && (
        <div className="px-3 pt-2">
          <ErrorBanner>{s.error}</ErrorBanner>
        </div>
      )}
      {!r || !sum ? (
        <EmptyState icon={<ArrowRightLeft size={22} />} title="Transfer limits">
          {blocked && !acReady ? (
            blocked
          ) : (
            <>
              How many MW can move from the source to the sink before a branch
              reaches its limit, in the base case and after each outage (DFAX
              and FCITC, from linear factors on the AC solve).
            </>
          )}
        </EmptyState>
      ) : (
        <div className="flex-1 min-h-0 flex flex-col">
          <div
            className="grid grid-cols-2 lg:grid-cols-4 gap-2 px-3 pt-2"
            data-testid="transfer-summary"
          >
            <Metric
              label="Transfer limit"
              value={
                sum.transfer_limit_mw === null
                  ? "No limit found"
                  : mw(sum.transfer_limit_mw)
              }
              tone={
                sum.transfer_limit_mw !== null && sum.transfer_limit_mw < 0
                  ? "bad"
                  : "neutral"
              }
              detail={
                sum.limited_by === "generation"
                  ? "Set by generation"
                  : sum.limited_by === "network"
                    ? "Set by the network"
                    : undefined
              }
            />
            <Metric
              label="FCITC (network)"
              value={mw(sum.fcitc_mw)}
              detail={
                sum.limiting
                  ? `${monitoredLabel(sum.limiting)}, ${sum.limiting.contingency ? `after ${contingencyLabel(sum.limiting)}` : "base case"}`
                  : `No element at or above ${fixed(sum.cutoff * 100, 0)}%`
              }
            />
            <Metric
              label="Generation limit"
              value={mw(sum.generation_limit_mw)}
              detail={`Source room ${mw(sum.source.available_mw)}, sink ${sum.sink.available_mw === null ? "unbounded" : mw(sum.sink.available_mw)}`}
            />
            <Metric
              label="Studied"
              value={`${count(sum.contingencies)} outage${sum.contingencies === 1 ? "" : "s"}`}
              detail={`${count(sum.monitored)} monitored · ${count(sum.source.units)} source and ${count(sum.sink.units)} sink units`}
            />
          </div>
          {sum.limiting && (
            <p
              className="px-3 pt-2 text-xs text-slate-700"
              data-testid="transfer-limiting"
            >
              The first limit:{" "}
              {sum.limiting.monitored.startsWith("interface:") ? (
                monitoredLabel(sum.limiting)
              ) : (
                <ElementLink
                  element={sum.limiting.monitored}
                  from="power"
                  onOpen={() => onPick(sum.limiting!.monitored)}
                >
                  {monitoredLabel(sum.limiting)}
                </ElementLink>
              )}{" "}
              {sum.limiting.contingency ? (
                <>after the outage of {contingencyLabel(sum.limiting)}</>
              ) : (
                "in the base case"
              )}{" "}
              carries {mw(sum.limiting.flow_mw)} against{" "}
              {mw(sum.limiting.limit_mw)} and takes{" "}
              {pct(Math.abs(sum.limiting.factor))} of the transfer
              {sum.limiting.limit_mw > 0 &&
              Math.abs(sum.limiting.flow_mw) >= 0.95 * sum.limiting.limit_mw
                ? `. It is already at ${fixed((100 * Math.abs(sum.limiting.flow_mw)) / sum.limiting.limit_mw, 1)}% of its limit before any transfer, so only a small transfer fits; lower "Already past from" to set such elements aside`
                : ""}
              {sum.cancelled
                ? ". Cancelled: these are the outages studied before it stopped."
                : "."}
            </p>
          )}
          {sum.notes.length > 0 && (
            <div className="px-3 pt-2 flex flex-col gap-1">
              {sum.notes.map((n) => (
                <Notice key={n}>{n}</Notice>
              ))}
            </div>
          )}
          <div className="flex items-center gap-2 px-3 pt-2">
            <div className="flex-1 min-w-0">
              <Tabs<View>
                label="Transfer results"
                value={view}
                onChange={setView}
                tabs={[
                  { id: "limits", label: "Limits", count: r.limits.length },
                  {
                    id: "past",
                    label: "Already past",
                    count: r.past_limits.length,
                  },
                  {
                    id: "contingencies",
                    label: "By outage",
                    count: r.contingencies.length,
                  },
                  { id: "factors", label: "PTDFs", count: r.factors.length },
                ]}
              />
            </div>
            <div className="flex items-center gap-2 shrink-0">
              <SearchBox
                value={search}
                onChange={setSearch}
                placeholder="Search elements…"
              />
              <Button
                type="button"
                onClick={exportCsv}
                title="Save the limits as CSV"
              >
                <Download size={13} /> CSV
              </Button>
            </div>
          </div>
          <div className="flex-1 min-h-0 px-3 py-2">
            {view === "limits" && (
              <DataTable
                rows={r.limits}
                columns={limitCols}
                rowKey={(l) => `${l.monitored}|${l.winding}`}
                search={search}
                label="Transfer limits"
                empty="No element limits the transfer."
              />
            )}
            {view === "past" && (
              <DataTable
                rows={r.past_limits}
                columns={limitCols}
                rowKey={(l) => `${l.monitored}|${l.winding}`}
                search={search}
                label="Already past their limit"
                empty="No element is past its limit before the transfer."
              />
            )}
            {view === "contingencies" && (
              <DataTable
                rows={r.contingencies}
                columns={contingencyColumns}
                rowKey={(l) => l.contingency || "base"}
                search={search}
                label="Limit by outage"
              />
            )}
            {view === "factors" && (
              <DataTable
                rows={r.factors}
                columns={factorCols}
                rowKey={(f) => `${f.monitored}|${f.winding}`}
                search={search}
                label="Transfer PTDFs"
                empty={`No element takes ${fixed(sum.cutoff * 100, 0)}% or more of the transfer.`}
              />
            )}
          </div>
          <p className="px-3 pb-2 text-[11px] text-slate-500">
            Linear transfer on the AC solve: flows in MW against{" "}
            {ratingLabel(shownBase)} in the base case and{" "}
            {ratingLabel(shownRating)} after an outage, at{" "}
            {fixed(sum.limit_pct, 0)}%. Took {fixed(sum.elapsed_ms, 0)} ms.
            Lists keep the lowest 500.
          </p>
        </div>
      )}
    </div>
  );
}
