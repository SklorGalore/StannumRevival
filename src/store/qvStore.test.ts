import { describe, expect, it } from "vitest";
import { DEFAULT_QV, lowestVoltageBuses, qvRows, qvSettings } from "./qvStore";
import type { BusRow } from "../lib/analysis/types";
import type { QvResults } from "../lib/analysis/commands.gen";

describe("QV curve settings", () => {
  it("sends only the voltages typed", () => {
    expect(qvSettings({ ...DEFAULT_QV, buses: "7314, 7340" })).toEqual({ buses: [7314, 7340] });
    expect(qvSettings({ buses: "5", vHigh: "1.05", vLow: "0.5", step: "0.02" })).toEqual({
      buses: [5],
      v_high_pu: 1.05,
      v_low_pu: 0.5,
      step_pu: 0.02,
    });
  });

  it("picks the lowest-voltage load buses", () => {
    const bus = (id: number, area: number, vm: number, kind = "PQ", pd = 10) => ({ id, area, vm, kind, pd }) as unknown as BusRow;
    const buses = [bus(1, 1, 0.99), bus(2, 1, 0.95), bus(3, 2, 0.9), bus(4, 1, 0.94, "PV"), bus(5, 1, 0.93, "PQ", 0), bus(6, 1, 0.97)];
    expect(lowestVoltageBuses(buses, 1, 2)).toEqual([2, 6]);
    expect(lowestVoltageBuses(buses, null, 2)).toEqual([3, 2]);
  });

  it("lists every point of every curve", () => {
    const r = {
      curves: [
        { bus: 5, name: "A ", v_pu: [0.9, 1.0], q_mvar: [-50, 0], q_limited: [1, 0] },
        { bus: 7, name: "", v_pu: [1.0], q_mvar: [0], q_limited: [0] },
      ],
    } as unknown as QvResults;
    const rows = qvRows(r);
    expect(rows).toHaveLength(3);
    expect(rows[0]).toEqual({ bus: 5, name: "A", v_pu: 0.9, q_mvar: -50, q_limited: 1 });
  });
});
