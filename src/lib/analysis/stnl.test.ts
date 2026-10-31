import { describe, expect, it } from "vitest";
import { describeSubsystem, monitorsText, outagesText, subsystemId, subsystemsText, type SetupSubsystem } from "./stnl";

const north: SetupSubsystem = { id: "north", name: "North 138", areas: [1], zones: [], owners: [], buses: [], kvMin: 100, kvMax: null };

describe("stnl", () => {
  it("makes ids the format accepts", () => {
    expect(subsystemId("North 138 kV")).toBe("north_138_kv");
    expect(subsystemId("2nd area")).toBe("s_2nd_area");
    expect(subsystemId("")).toBe("subsystem");
    expect(subsystemId("North", ["north", "north_2"])).toBe("north_3");
  });
  it("writes subsystems, quoting names", () => {
    expect(subsystemsText([north, { ...north, id: "q", name: 'Say "hi"', areas: [], buses: [1, 2], kvMin: null, kvMax: 230 }])).toBe(
      'stannum-lists 0.1\nsubsystem north name="North 138" areas=[1] kv_min=100\nsubsystem q name="Say \\"hi\\"" buses=[1, 2] kv_max=230\n'
    );
  });
  it("writes contingency sets for a subsystem", () => {
    const text = outagesText([north], { subsystem: "north", branches: true, generators: true, buses: false, pairs: true });
    expect(text).toContain('contingency_set n2_branches name="N-2 branches" kind=n_2_branches subsystem=north');
    expect(text).toContain('contingency_set n1_generators name="N-1 generators" kind=n_1_generators subsystem=north');
    expect(text).not.toContain("bus_outages");
  });
  it("writes monitors for a subsystem", () => {
    expect(monitorsText([north], "north", 95, false)).toBe(
      'stannum-lists 0.1\nsubsystem north name="North 138" areas=[1] kv_min=100\nmonitor flows subsystem=north limit_pct=95\n'
    );
    expect(monitorsText([north], "north", 100, true)).toContain("monitor voltages kind=voltage subsystem=north");
  });
  it("describes members", () => {
    expect(describeSubsystem(north)).toBe("area 1, 100 kV and up");
    expect(describeSubsystem({ ...north, areas: [], kvMin: null })).toBe("every bus");
  });
});
