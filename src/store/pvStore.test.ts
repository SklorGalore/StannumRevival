import { describe, expect, it } from "vitest";
import { DEFAULT_PV, positive, pvRows, pvSettings, validPositive } from "./pvStore";
import type { PvResults } from "../lib/analysis/commands.gen";

describe("PV curve settings", () => {
  it("reads optional positive numbers", () => {
    expect(positive("")).toBeNull();
    expect(positive(" 25 ")).toBe(25);
    expect(positive("-1")).toBeNull();
    expect(validPositive("")).toBe(true);
    expect(validPositive("abc")).toBe(false);
  });

  it("takes every bus as the source unless one is picked", () => {
    const s = pvSettings({ ...DEFAULT_PV, sink: { kind: "area", value: 7, buses: "" } });
    expect(s.sink).toEqual({ areas: [7] });
    expect(s.source).toEqual({});
    expect(s.step_mw).toBeNull();
    const t = pvSettings({ ...DEFAULT_PV, allSource: false, source: { kind: "zone", value: 2, buses: "" }, stepMw: "50", vminPu: "0.92" });
    expect(t.source).toEqual({ zones: [2] });
    expect(t.step_mw).toBe(50);
    expect(t.vmin_pu).toBe(0.92);
  });

  it("puts each curve's voltages beside its points", () => {
    const r = {
      points: [
        { transfer_mw: 0, sink_load_mw: 90, losses_mw: 5, min_vm_pu: 0.99, min_vm_bus: 5, q_limited: 0, q_reserve_mvar: 10, iterations: 0 },
        { transfer_mw: 10, sink_load_mw: 100, losses_mw: 6, min_vm_pu: 0.98, min_vm_bus: 5, q_limited: 1, q_reserve_mvar: 8, iterations: 3 },
      ],
      curves: [{ bus: 5, name: "", base_kv: 345, vmin_pu: 0.9, vm_pu: [0.99, 0.98], drop_pu: 0.01, sensitivity_pu_per_100mw: 0.1, vmin_transfer_mw: null }],
    } as unknown as PvResults;
    expect(pvRows(r)[1]).toMatchObject({ transfer_mw: 10, vm_5: 0.98, q_limited: 1 });
  });
});
