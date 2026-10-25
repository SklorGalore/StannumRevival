// Filters as chips: each rule reads as a phrase ("Area is 2") and opens an
// editor when clicked. Uses the same rules and matching as Topology.
import { useEffect, useRef, useState } from "react";
import { Filter, Plus, X } from "lucide-react";
import {
  OPERATORS,
  needsValue,
  newRule,
  ruleLabel,
  ruleReady,
  type FieldDefinition,
  type FieldValue,
  type FilterOperator,
  type FilterRule,
} from "../lib/filters";
import { inputClass } from "./primitives";

const selectClass = `${inputClass} min-w-0`;

function RuleEditor({
  rule,
  fields,
  options,
  onChange,
  onRemove,
  onDone,
}: {
  rule: FilterRule;
  fields: FieldDefinition[];
  options: (field: string) => FieldValue[];
  onChange: (r: FilterRule) => void;
  onRemove: () => void;
  onDone: () => void;
}) {
  const def = fields.find((f) => f.key === rule.field) ?? fields[0];
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => {
      // The chip itself toggles the editor, so a press on it isn't "away".
      const chip = ref.current?.parentElement;
      if (chip && !chip.contains(e.target as Node)) onDone();
    };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [onDone]);
  const values = def.type === "category" ? options(def.key).filter((v) => v !== null) : [];
  return (
    <div
      ref={ref}
      role="dialog"
      aria-label="Edit filter"
      className="absolute left-0 top-full mt-1 z-30 w-72 rounded-md border border-slate-200 bg-white shadow-lg p-2 flex flex-col gap-1.5"
      onKeyDown={(e) => {
        if (e.key === "Escape" || e.key === "Enter") {
          e.preventDefault();
          onDone();
        }
      }}
    >
      <select
        className={selectClass}
        aria-label="Filter field"
        value={rule.field}
        onChange={(e) => {
          const d = fields.find((f) => f.key === e.target.value) ?? fields[0];
          onChange({ ...rule, field: d.key, operator: OPERATORS[d.type][0].id, value: "", upper: undefined });
        }}
      >
        {fields.map((f) => (
          <option key={f.key} value={f.key}>
            {f.label}
          </option>
        ))}
      </select>
      <div className="flex items-center gap-1.5">
        <select
          className={`${selectClass} w-24`}
          aria-label="Filter operator"
          value={rule.operator}
          onChange={(e) => onChange({ ...rule, operator: e.target.value as FilterOperator })}
        >
          {OPERATORS[def.type].map((o) => (
            <option key={o.id} value={o.id}>
              {o.label}
            </option>
          ))}
        </select>
        {needsValue(rule.operator) &&
          (def.type === "category" ? (
            <select
              autoFocus
              className={`${selectClass} flex-1`}
              aria-label="Filter value"
              value={rule.value}
              onChange={(e) => onChange({ ...rule, value: e.target.value })}
            >
              <option value="">Choose…</option>
              {values.map((v) => (
                <option key={String(v)} value={String(v)}>
                  {String(v)}
                </option>
              ))}
            </select>
          ) : (
            <input
              autoFocus
              className={`${inputClass} flex-1 min-w-0`}
              aria-label="Filter value"
              type={def.type === "number" ? "number" : "text"}
              step="any"
              value={rule.value}
              onChange={(e) => onChange({ ...rule, value: e.target.value })}
            />
          ))}
        {rule.operator === "between" && (
          <input
            className={`${inputClass} flex-1 min-w-0`}
            aria-label="Upper value"
            type="number"
            step="any"
            value={rule.upper ?? ""}
            onChange={(e) => onChange({ ...rule, upper: e.target.value })}
          />
        )}
      </div>
      <div className="flex items-center justify-between pt-0.5">
        <button type="button" className="text-[11px] text-slate-500 hover:text-red-700" onClick={onRemove}>
          Remove
        </button>
        <button
          type="button"
          className="rounded px-2 py-0.5 text-[11px] font-medium text-white bg-teal-700 hover:bg-teal-800"
          onClick={onDone}
        >
          Done
        </button>
      </div>
    </div>
  );
}

/**
 * A row of filter chips. Rules that aren't finished yet don't apply, and one
 * left unfinished is dropped when its editor closes.
 */
export function FilterChips({
  fields,
  options,
  rules,
  onRules,
  note,
}: {
  fields: FieldDefinition[];
  /** Values to offer for a category field. */
  options: (field: string) => FieldValue[];
  rules: FilterRule[];
  onRules: (rules: FilterRule[]) => void;
  /** What the filters leave, such as "40 of 300 buses". */
  note?: string;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const close = () => {
    onRules(rules.filter((r) => r.id !== editing || ruleReady(r)));
    setEditing(null);
  };
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-3 py-1.5 border-b border-slate-200 bg-slate-50 text-xs" data-testid="filter-chips">
      <span className="inline-flex items-center gap-1 text-slate-500 mr-0.5">
        <Filter size={12} /> Filters
      </span>
      {rules.map((rule) => (
        <span key={rule.id} className="relative">
          <span
            className={`inline-flex items-center rounded-full border text-[11px] ${
              ruleReady(rule) ? "border-teal-300 bg-teal-50 text-teal-900" : "border-dashed border-slate-300 bg-white text-slate-500"
            }`}
            data-testid="filter-chip"
          >
            <button type="button" className="pl-2 pr-1 py-0.5" onClick={() => (editing === rule.id ? close() : setEditing(rule.id))}>
              {ruleReady(rule) ? ruleLabel(rule, fields) : `${fields.find((f) => f.key === rule.field)?.label ?? rule.field}…`}
            </button>
            <button
              type="button"
              className="pr-1.5 pl-0.5 py-0.5 text-teal-700 hover:text-red-700"
              aria-label={`Remove filter ${ruleLabel(rule, fields)}`}
              onClick={() => {
                if (editing === rule.id) setEditing(null);
                onRules(rules.filter((r) => r.id !== rule.id));
              }}
            >
              <X size={11} />
            </button>
          </span>
          {editing === rule.id && (
            <RuleEditor
              rule={rule}
              fields={fields}
              options={options}
              onChange={(next) => onRules(rules.map((r) => (r.id === rule.id ? next : r)))}
              onRemove={() => {
                setEditing(null);
                onRules(rules.filter((r) => r.id !== rule.id));
              }}
              onDone={close}
            />
          )}
        </span>
      ))}
      <button
        type="button"
        className="inline-flex items-center gap-0.5 rounded-full px-2 py-0.5 text-[11px] text-slate-600 hover:bg-slate-200/70"
        onClick={() => {
          const rule = newRule(fields);
          onRules([...rules.filter((r) => r.id !== editing || ruleReady(r)), rule]);
          setEditing(rule.id);
        }}
      >
        <Plus size={11} /> Add filter
      </button>
      {rules.length > 1 && (
        <button type="button" className="text-[11px] text-slate-500 hover:text-slate-800" onClick={() => onRules([])}>
          Clear all
        </button>
      )}
      {note && <span className="ml-auto text-[11px] text-slate-500">{note}</span>}
    </div>
  );
}
