import { describe, expect, it } from "vitest";
import fixtures from "./__fixtures__/topology-wire.json";
import { decodeCase, decodeTopology, decodeView, type WireTopology } from "./topologyWire";

// Each fixture holds the compact snapshot the core sends and the full one it
// stands for (written by crates/stannum-core/tests/topology_wire.rs).
const cases = fixtures as unknown as { name: string; case: { wire: WireTopology; expected: unknown } }[];

describe("decodeTopology", () => {
  for (const { name, case: c } of cases)
    it(`rebuilds the full snapshot: ${name}`, () => {
      expect(decodeTopology(c.wire)).toEqual(c.expected);
    });

  it("rebuilds a merge answer against the snapshot's case", () => {
    const merged = cases.find((c) => c.name.endsWith("merged"))!.case;
    const view = decodeView(merged.wire, decodeCase(merged.wire.case));
    const expected = merged.expected as { analysis: unknown; graph: { nodes: { member_buses: number[] }[] } };
    expect(view).toEqual({ analysis: expected.analysis, graph: expected.graph });
    expect(view.graph.nodes.some((n) => n.member_buses.length > 1)).toBe(true);
  });
});
