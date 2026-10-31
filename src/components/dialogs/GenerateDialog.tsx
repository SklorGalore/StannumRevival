import { useEffect, useMemo, useState } from "react";
import { X, Wand2 } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { useDiagramStore } from "../../store/diagramStore";
import { caseNetwork } from "../../lib/diagram/network";
import { confirmDialog } from "../../lib/tauri/commands";
import type { CaseNetwork } from "../../lib/analysis/types";
import {
  busLabel,
  DRAW_FILTER_FIELDS,
  drawFieldValue,
  generateDiagram,
  READABLE_BUSES,
  selectBuses,
  type Grouping,
  type Scope,
} from "../../lib/diagram/generate";
import { distinctValues, ruleReady, type FilterRule } from "../../lib/filters";
import { FilterChips } from "../../ui/FilterChips";
import { Button, ErrorBanner, Field, inputClass } from "../../ui/primitives";
import { Modal } from "../../ui/Modal";

type ScopeKind = Scope["kind"];

const GROUPINGS: { kind: Grouping; label: string; one: string }[] = [
  { kind: "area", label: "One area", one: "Area" },
  { kind: "zone", label: "One zone", one: "Zone" },
  { kind: "owner", label: "One owner", one: "Owner" },
];

/** Choose which part of the open case to draw, then lay it out. */
export function GenerateDialog({ onClose, around }: { onClose: () => void; around?: number }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const replaceWithGenerated = useDiagramStore((s) => s.replaceWithGenerated);

  const [net, setNet] = useState<CaseNetwork | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [kind, setKind] = useState<ScopeKind>("all");
  const [group, setGroup] = useState<Record<Grouping, number>>({ area: 0, zone: 0, owner: 0 });
  const [filters, setFilters] = useState<FilterRule[]>([]);
  const [center, setCenter] = useState("");
  const [depth, setDepth] = useState(2);

  useEffect(() => {
    let live = true;
    setNet(null);
    setError(null);
    caseNetwork()
      .then((n) => {
        if (!live) return;
        setNet(n);
        const real = n.buses.filter((b) => !b.star);
        const big = real.length > READABLE_BUSES;
        // One-lines are mostly drawn a few hops around the bus under study.
        setKind(real.length > 20 || big ? "around" : "all");
        setGroup({ area: real[0]?.area ?? 0, zone: real[0]?.zone ?? 0, owner: real[0]?.owner ?? 0 });
        const ref = real.find((b) => b.kind === "Ref") ?? real[0];
        setCenter(ref ? String(ref.id) : "");
        // Asked to draw around one bus, such as the shared selection.
        if (around !== undefined && real.some((b) => b.id === around)) {
          setKind("around");
          setCenter(String(around));
        }
      })
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
    };
  }, [caseVersion]);

  // Bus counts of each area, zone and owner.
  const groups = useMemo(() => {
    const out = {} as Record<Grouping, [number, number][]>;
    for (const { kind: g } of GROUPINGS) {
      const counts = new Map<number, number>();
      for (const b of net?.buses ?? []) if (!b.star) counts.set(b[g], (counts.get(b[g]) ?? 0) + 1);
      out[g] = [...counts].sort((a, b) => a[0] - b[0]);
    }
    return out;
  }, [net]);
  // MATPOWER cases have no owners: owner 0 on every bus.
  const hasOwners = groups.owner.some(([o]) => o > 0);
  const groupings = GROUPINGS.filter((g) => g.kind !== "owner" || hasOwners);
  const fields = hasOwners ? DRAW_FILTER_FIELDS : DRAW_FILTER_FIELDS.filter((f) => f.key !== "owner");
  const ready = filters.filter(ruleReady);

  const centerBus = net?.buses.find((b) => b.id === Number(center) && !b.star);
  const base: Scope | null =
    kind === "all"
      ? { kind }
      : kind === "around"
        ? centerBus
          ? { kind, bus: centerBus.id, depth }
          : null
        : { kind, value: group[kind] };
  const scope: Scope | null = base && (ready.length ? { ...base, filters: ready } : base);
  const filterKey = JSON.stringify(ready);
  const count = useMemo(() => {
    if (!net || !scope) return 0;
    const chosen = selectBuses(net, scope);
    return net.buses.filter((b) => chosen.has(b.id) && !b.star).length;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [net, kind, group, centerBus?.id, depth, filterKey]);

  async function generate() {
    if (!net || !scope || !summary) return;
    if (
      useDiagramStore.getState().isDirty &&
      !(await confirmDialog("Replace the current drawing? It has unsaved changes."))
    )
      return;
    setBusy(true);
    setError(null);
    try {
      const out = await generateDiagram(net, scope);
      replaceWithGenerated(
        out.nodes,
        out.edges,
        summary.name,
        scope.kind === "around" ? { bus: scope.bus, depth: scope.depth } : undefined
      );
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const option = (value: ScopeKind, label: string) => (
    <label className="flex items-center gap-2 text-xs text-slate-700">
      <input type="radio" name="scope" checked={kind === value} onChange={() => setKind(value)} />
      {label}
    </label>
  );

  return (
    <Modal label="Generate one-line from case" onClose={onClose} className="w-96 p-5 flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-slate-800">Generate one-line from case</h2>
          <button onClick={onClose} className="text-slate-500 hover:text-slate-700" aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <p className="text-xs text-slate-500">
          Draws buses, generators, loads, lines, transformers and switching devices from <b>{summary?.name}</b>. Symbols stay linked
          to the case, so power flow results show on them.
        </p>

        {error && <ErrorBanner>{error}</ErrorBanner>}
        {!net && !error && <p className="text-xs text-slate-500">Reading case…</p>}

        {net && (
          <div className="flex flex-col gap-2.5">
            {option("all", `Whole case (${net.buses.filter((b) => !b.star).length} buses)`)}
            {groupings.map(({ kind: g, label, one }) => (
              <div key={g} className="contents">
                {option(g, label)}
                {kind === g && (
                  <select
                    className={`${inputClass} ml-6`}
                    aria-label={one}
                    value={group[g]}
                    onChange={(e) => setGroup({ ...group, [g]: Number(e.target.value) })}
                  >
                    {groups[g].map(([v, n]) => (
                      <option key={v} value={v}>
                        {one} {v} ({n} buses)
                      </option>
                    ))}
                  </select>
                )}
              </div>
            ))}
            {option("around", "Around a bus")}
            {kind === "around" && (
              <div className="ml-6 grid grid-cols-2 gap-2">
                <Field label="Bus number">
                  <input
                    className={inputClass}
                    value={center}
                    inputMode="numeric"
                    onChange={(e) => setCenter(e.target.value.trim())}
                    list="generate-buses"
                  />
                </Field>
                <Field label="Hops out">
                  <select className={inputClass} value={depth} onChange={(e) => setDepth(Number(e.target.value))}>
                    {[1, 2, 3, 4, 5, 6].map((d) => (
                      <option key={d} value={d}>
                        {d}
                      </option>
                    ))}
                  </select>
                </Field>
                <p className="col-span-2 text-[11px] text-slate-500">
                  {centerBus ? `${busLabel(centerBus)}, ${centerBus.base_kv} kV` : "No bus with that number."}
                </p>
                <datalist id="generate-buses">
                  {net.buses
                    .filter((b) => !b.star)
                    .slice(0, 2000)
                    .map((b) => (
                      <option key={b.id} value={b.id}>
                        {busLabel(b)}
                      </option>
                    ))}
                </datalist>
              </div>
            )}
            <div className="rounded-md border border-slate-200 overflow-visible [&>[data-testid=filter-chips]]:border-b-0 [&>[data-testid=filter-chips]]:rounded-md">
              <FilterChips
                fields={fields}
                options={(field) =>
                  distinctValues(net.buses.filter((b) => !b.star).map((b) => drawFieldValue(b, field)))
                }
                rules={filters}
                onRules={setFilters}
              />
            </div>
            {ready.length > 0 && kind === "around" && (
              <p className="text-[11px] text-slate-500">
                The drawing grows only through buses that pass the filters; the studied bus is always drawn.
              </p>
            )}
            <p className={`text-[11px] ${count > READABLE_BUSES ? "text-amber-700" : "text-slate-500"}`}>
              {count} buses will be drawn.
              {count > READABLE_BUSES && " That's a lot for one sheet; an area, a filter or a neighbourhood reads better."}
            </p>
          </div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={generate} disabled={!net || !scope || !count || busy}>
            <Wand2 size={14} /> {busy ? "Laying out…" : "Generate"}
          </Button>
        </div>
    </Modal>
  );
}
