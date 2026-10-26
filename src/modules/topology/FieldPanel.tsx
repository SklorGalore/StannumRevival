// Field filters and colour rules for the Topology view (from nodal).
import { Plus, X } from "lucide-react";
import type { TopologySnapshot } from "../../lib/analysis/types";
import { Button, inputClass } from "../../ui/primitives";
import {
  BUS_FIELDS,
  EQUIPMENT_FIELDS,
  fieldOptions,
  type FieldDefinition,
  type FieldTarget,
  type FilterOperator,
  type FilterRule,
} from "./fields/field-view";
import { OPERATORS, newRule as newFilterRule } from "../../lib/filters";
import { COLOR_PALETTES, type ColorPresentation, type ColorScheme, type ColorSettings } from "./fields/field-colors";

const FIELDS: Record<FieldTarget, FieldDefinition[]> = { bus: BUS_FIELDS, equipment: EQUIPMENT_FIELDS };

const selectClass = `${inputClass} min-w-0`;

function definition(target: FieldTarget, key: string): FieldDefinition | undefined {
  return FIELDS[target].find((f) => f.key === key);
}

export function newRule(target: FieldTarget = "bus"): FilterRule {
  return newFilterRule(FIELDS[target], target);
}

function RuleRow({
  rule,
  snapshot,
  onChange,
  onRemove,
}: {
  rule: FilterRule;
  snapshot: TopologySnapshot;
  onChange: (r: FilterRule) => void;
  onRemove: () => void;
}) {
  const def = definition(rule.target, rule.field) ?? FIELDS[rule.target][0];
  const ops = OPERATORS[def.type];
  const needsValue = rule.operator !== "missing" && rule.operator !== "present";
  const options = def.type === "category" ? fieldOptions(snapshot, rule.target, rule.field).filter((v) => v !== null) : [];
  const reset = (target: FieldTarget, field: string): FilterRule => {
    const d = definition(target, field) ?? FIELDS[target][0];
    return { ...rule, target, field: d.key, operator: OPERATORS[d.type][0].id, value: "", upper: undefined };
  };
  return (
    <div className="rounded-md border border-slate-200 p-2 flex flex-col gap-1.5" data-testid="filter-rule">
      <div className="flex items-center gap-1.5">
        <select className={`${selectClass} w-24`} aria-label="Filter applies to" value={rule.target} onChange={(e) => onChange(reset(e.target.value as FieldTarget, ""))}>
          <option value="bus">Buses</option>
          <option value="equipment">Equipment</option>
        </select>
        <select className={`${selectClass} flex-1`} aria-label="Filter field" value={rule.field} onChange={(e) => onChange(reset(rule.target, e.target.value))}>
          {FIELDS[rule.target].map((f) => (
            <option key={f.key} value={f.key}>
              {f.label}
            </option>
          ))}
        </select>
        <button className="text-slate-400 hover:text-slate-700" onClick={onRemove} aria-label="Remove filter" title="Remove filter">
          <X size={14} />
        </button>
      </div>
      <div className="flex items-center gap-1.5">
        <select className={`${selectClass} w-24`} aria-label="Filter operator" value={rule.operator} onChange={(e) => onChange({ ...rule, operator: e.target.value as FilterOperator })}>
          {ops.map((o) => (
            <option key={o.id} value={o.id}>
              {o.label}
            </option>
          ))}
        </select>
        {needsValue &&
          (def.type === "category" ? (
            <select className={`${selectClass} flex-1`} aria-label="Filter value" value={rule.value} onChange={(e) => onChange({ ...rule, value: e.target.value })}>
              <option value="">Choose…</option>
              {options.map((v) => (
                <option key={String(v)} value={String(v)}>
                  {String(v)}
                </option>
              ))}
            </select>
          ) : (
            <input
              className={`${inputClass} flex-1 min-w-0`}
              aria-label="Filter value"
              type={def.type === "number" ? "number" : "text"}
              value={rule.value}
              onChange={(e) => onChange({ ...rule, value: e.target.value })}
            />
          ))}
        {needsValue && rule.operator === "between" && (
          <input
            className={`${inputClass} flex-1 min-w-0`}
            aria-label="Upper value"
            type="number"
            value={rule.upper ?? ""}
            onChange={(e) => onChange({ ...rule, upper: e.target.value })}
          />
        )}
      </div>
    </div>
  );
}

function ColorRule({
  target,
  scheme,
  legend,
  onChange,
}: {
  target: FieldTarget;
  scheme: ColorScheme;
  legend?: ColorPresentation["legends"][number];
  onChange: (s: ColorScheme) => void;
}) {
  const def = definition(target, scheme.field);
  return (
    <div className="flex flex-col gap-1.5">
      <label className="flex items-center gap-2 text-xs text-slate-600">
        <span className="w-20 shrink-0">{target === "bus" ? "Buses by" : "Equipment by"}</span>
        <select
          className={`${selectClass} flex-1`}
          aria-label={target === "bus" ? "Colour buses by" : "Colour equipment by"}
          value={scheme.field}
          onChange={(e) => onChange({ ...scheme, field: e.target.value, colors: {} })}
        >
          <option value="">No colour rule</option>
          {FIELDS[target].map((f) => (
            <option key={f.key} value={f.key}>
              {f.label}
            </option>
          ))}
        </select>
      </label>
      {def && def.type !== "number" && (
        <label className="flex items-center gap-2 text-xs text-slate-600">
          <span className="w-20 shrink-0">Palette</span>
          <select className={`${selectClass} flex-1`} value={scheme.palette} onChange={(e) => onChange({ ...scheme, palette: e.target.value, colors: {} })}>
            {COLOR_PALETTES.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
      )}
      {def?.type === "number" && (
        <div className="flex items-center gap-2 text-xs text-slate-600">
          <span className="w-20 shrink-0">Low → high</span>
          <input type="color" aria-label="Low colour" value={scheme.low} onChange={(e) => onChange({ ...scheme, low: e.target.value })} />
          <input type="color" aria-label="High colour" value={scheme.high} onChange={(e) => onChange({ ...scheme, high: e.target.value })} />
        </div>
      )}
      {def && legend && legend.entries.length > 0 && (
        <ul className="flex flex-col gap-0.5 max-h-40 overflow-y-auto pl-1">
          {legend.entries.map((entry) => (
            <li key={entry.label} className="flex items-center gap-2 text-[11px] text-slate-700">
              {/* Missing values use the scheme's "missing" colour and are not overridable per value. */}
              <input
                type="color"
                className="h-4 w-5 cursor-pointer border-0 p-0 bg-transparent"
                aria-label={`Colour for ${entry.label}`}
                value={entry.color}
                disabled={entry.label === "Missing"}
                onChange={(e) => onChange({ ...scheme, colors: { ...scheme.colors, [entry.label]: e.target.value } })}
              />
              <span className="truncate">{entry.label}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function FieldPanel({
  snapshot,
  rules,
  onRules,
  colors,
  onColors,
  presentation,
  counts,
}: {
  snapshot: TopologySnapshot;
  rules: FilterRule[];
  onRules: (r: FilterRule[]) => void;
  colors: ColorSettings;
  onColors: (c: ColorSettings) => void;
  presentation: ColorPresentation;
  counts: { buses: number; totalBuses: number; equipment: number; totalEquipment: number };
}) {
  const legendFor = (t: FieldTarget) => presentation.legends.find((l) => l.target === t);
  return (
    <div className="p-3 flex flex-col gap-4 text-xs">
      <section className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <h3 className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">Filters</h3>
          {rules.length > 0 && (
            <button className="text-[11px] text-slate-500 hover:text-slate-800" onClick={() => onRules([])}>
              Clear all
            </button>
          )}
        </div>
        {rules.length === 0 && <p className="text-slate-500">Show only buses and equipment that match every rule.</p>}
        {rules.map((rule, i) => (
          <RuleRow
            key={rule.id}
            rule={rule}
            snapshot={snapshot}
            onChange={(r) => onRules(rules.map((x, j) => (j === i ? r : x)))}
            onRemove={() => onRules(rules.filter((_, j) => j !== i))}
          />
        ))}
        <Button variant="ghost" onClick={() => onRules([...rules, newRule()])}>
          <Plus size={13} /> Add filter
        </Button>
        {rules.length > 0 && (
          <p className="text-slate-500" data-testid="filter-counts">
            Showing {counts.buses.toLocaleString()} of {counts.totalBuses.toLocaleString()} buses and {counts.equipment.toLocaleString()} of{" "}
            {counts.totalEquipment.toLocaleString()} equipment.
          </p>
        )}
      </section>
      <section className="flex flex-col gap-2">
        <h3 className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">Colours</h3>
        <ColorRule target="bus" scheme={colors.bus} legend={legendFor("bus")} onChange={(bus) => onColors({ ...colors, bus })} />
        <ColorRule
          target="equipment"
          scheme={colors.equipment}
          legend={legendFor("equipment")}
          onChange={(equipment) => onColors({ ...colors, equipment })}
        />
      </section>
    </div>
  );
}
