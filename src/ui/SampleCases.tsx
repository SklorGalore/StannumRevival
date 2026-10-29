import { useEffect, useState } from "react";
import { Map as MapIcon, Sparkles } from "lucide-react";
import { api } from "../lib/analysis/api";
import type { SampleCase } from "../lib/analysis/commands.gen";
import { useWorkspace } from "../store/workspaceStore";

let cached: Promise<SampleCase[]> | null = null;
let known: SampleCase[] = [];

/** The sample cases built into Stannum; asked for once. */
export function sampleCases(): Promise<SampleCase[]> {
  cached ??= api.sampleCases().then(
    (s) => (known = s),
    () => {
      cached = null;
      return [];
    }
  );
  return cached;
}

/** The sample cases if they have arrived, else none (and asks for them). */
export function knownSampleCases(): SampleCase[] {
  void sampleCases();
  return known;
}

/** The sample cases, for an empty screen, so there is something to open on first run. */
export function SampleCases() {
  const [samples, setSamples] = useState<SampleCase[]>([]);
  useEffect(() => {
    let live = true;
    void sampleCases().then((s) => live && setSamples(s));
    return () => {
      live = false;
    };
  }, []);
  const openSample = useWorkspace((s) => s.openSample);
  const opening = useWorkspace((s) => s.opening);
  if (samples.length === 0) return null;
  return (
    <div className="mt-6 w-full max-w-md text-left" data-testid="sample-cases">
      <h3 className="px-2 text-[10px] font-semibold uppercase tracking-wider text-slate-400">Try a sample case</h3>
      <ul className="mt-1">
        {samples.map((s) => (
          <li key={s.id}>
            <button
              type="button"
              className="w-full flex items-start gap-2.5 rounded-md px-2 py-1.5 text-left hover:bg-slate-100 disabled:opacity-60"
              onClick={() => void openSample(s.id)}
              disabled={opening}
              title={s.description}
            >
              <Sparkles size={15} className="mt-0.5 shrink-0 text-teal-700" aria-hidden />
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline gap-2">
                  <span className="truncate text-sm text-slate-800">{s.title}</span>
                  <span className="shrink-0 text-[11px] text-slate-400">
                    {s.format === "matpower" ? "MATPOWER" : "PSS/E RAW"} · {s.buses} buses
                  </span>
                  {s.has_coordinates && (
                    <MapIcon size={12} className="shrink-0 self-center text-slate-400" aria-label="Has bus coordinates for the map" />
                  )}
                </span>
                <span className="block text-[11px] leading-snug text-slate-500">{s.description}</span>
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
