import { useEffect, useMemo, useState } from "react";
import { X, Wand2 } from "lucide-react";
import { useWorkspace } from "../../store/workspaceStore";
import { useDiagramStore } from "../../store/diagramStore";
import { caseNetwork } from "../../lib/diagram/network";
import type { CaseNetwork } from "../../lib/analysis/types";
import { busLabel, generateDiagram, READABLE_BUSES, selectBuses, type Scope } from "../../lib/diagram/generate";
import { Button, ErrorBanner, Field, inputClass } from "../../ui/primitives";

type ScopeKind = Scope["kind"];

/** Choose which part of the open case to draw, then lay it out. */
export function GenerateDialog({ onClose }: { onClose: () => void }) {
  const summary = useWorkspace((s) => s.caseSummary);
  const caseVersion = useWorkspace((s) => s.caseVersion);
  const replaceWithGenerated = useDiagramStore((s) => s.replaceWithGenerated);

  const [net, setNet] = useState<CaseNetwork | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [kind, setKind] = useState<ScopeKind>("all");
  const [area, setArea] = useState(0);
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
        setArea(real[0]?.area ?? 0);
        const ref = real.find((b) => b.kind === "Ref") ?? real[0];
        setCenter(ref ? String(ref.id) : "");
      })
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
    };
  }, [caseVersion]);

  const areas = useMemo(() => {
    const counts = new Map<number, number>();
    for (const b of net?.buses ?? []) if (!b.star) counts.set(b.area, (counts.get(b.area) ?? 0) + 1);
    return [...counts].sort((a, b) => a[0] - b[0]);
  }, [net]);

  const centerBus = net?.buses.find((b) => b.id === Number(center) && !b.star);
  const scope: Scope | null =
    kind === "all"
      ? { kind }
      : kind === "area"
        ? { kind, area }
        : centerBus
          ? { kind, bus: centerBus.id, depth }
          : null;
  const count = useMemo(() => {
    if (!net || !scope) return 0;
    const chosen = selectBuses(net, scope);
    return net.buses.filter((b) => chosen.has(b.id) && !b.star).length;
  }, [net, kind, area, centerBus?.id, depth]);

  async function generate() {
    if (!net || !scope || !summary) return;
    if (
      useDiagramStore.getState().isDirty &&
      !window.confirm("Replace the current drawing? It has unsaved changes.")
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30" onClick={onClose}>
      <div
        role="dialog"
        aria-label="Generate from case"
        className="bg-white rounded-lg shadow-xl w-96 p-5 flex flex-col gap-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-slate-800">Generate one-line from case</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600" aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <p className="text-xs text-slate-500">
          Draws buses, generators, loads, lines and transformers from <b>{summary?.name}</b>. Symbols stay linked
          to the case, so power flow results show on them.
        </p>

        {error && <ErrorBanner>{error}</ErrorBanner>}
        {!net && !error && <p className="text-xs text-slate-500">Reading case…</p>}

        {net && (
          <div className="flex flex-col gap-2.5">
            {option("all", `Whole case (${net.buses.filter((b) => !b.star).length} buses)`)}
            {option("area", "One area")}
            {kind === "area" && (
              <select className={`${inputClass} ml-6`} value={area} onChange={(e) => setArea(Number(e.target.value))}>
                {areas.map(([a, n]) => (
                  <option key={a} value={a}>
                    Area {a} ({n} buses)
                  </option>
                ))}
              </select>
            )}
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
            <p className={`text-[11px] ${count > READABLE_BUSES ? "text-amber-700" : "text-slate-500"}`}>
              {count} buses will be drawn.
              {count > READABLE_BUSES && " That's a lot for one sheet; an area or a neighbourhood reads better."}
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
      </div>
    </div>
  );
}
