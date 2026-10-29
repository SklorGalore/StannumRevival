// Files saved by earlier releases still open. The samples in tests/compat
// were written by the released apps (see tests/compat/README.md); a change
// that stops one opening breaks the promise that 1.x opens every file 1.x
// wrote. Add a folder there for each release that changes a format.
import { describe, expect, it } from "vitest";
import { nodeTypes } from "../../components/nodes";
import { edgeTypes } from "../../components/edges";
import { checkDiagram, deserialize } from "../serialization/deserialize";
import { serialize } from "../serialization/serialize";
import { parseStudyFile } from "../study/studyFile";
import type { SLDDiagram } from "../../store/types";

const files = import.meta.glob("/tests/compat/*/*.{stannum,sld}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const samples = (extension: string) =>
  Object.entries(files)
    .filter(([path]) => path.endsWith(extension))
    .map(([path, text]) => ({ release: path.split("/")[3], path, text }));

/** Every symbol and wire is one the canvas can draw, and every wire ends on a symbol. */
function expectDrawable(diagram: SLDDiagram) {
  expect(diagram.nodes.length).toBeGreaterThan(0);
  const ids = new Set(diagram.nodes.map((n) => n.id));
  expect(ids.size).toBe(diagram.nodes.length);
  for (const node of diagram.nodes) {
    expect(Object.keys(nodeTypes), `symbol ${node.id}`).toContain(node.type);
    expect(typeof node.position.x).toBe("number");
    expect(typeof node.position.y).toBe("number");
  }
  for (const edge of diagram.edges) {
    expect(Object.keys(edgeTypes), `wire ${edge.id}`).toContain(edge.type ?? "conductor");
    expect(ids.has(edge.source) && ids.has(edge.target), `wire ${edge.id} ends`).toBe(true);
  }
}

describe("files from earlier releases", () => {
  it("has samples from 0.3.0 and 0.4.1", () => {
    const releases = new Set([...samples(".stannum"), ...samples(".sld")].map((s) => s.release));
    expect([...releases]).toEqual(expect.arrayContaining(["0.3.0", "0.4.1"]));
  });

  for (const sample of samples(".stannum")) {
    it(`opens the ${sample.release} study ${sample.path.split("/").pop()}`, async () => {
      const study = await parseStudyFile(sample.text);
      expect(study.app).toBe(sample.release);
      expect(study.case.name).toBe("case9.m");
      expect(study.edits.map((e) => e.edit.kind)).toEqual(["status", "generation"]);
      expect(study.solves.map((s) => s.name)).toEqual(["Case as filed"]);
      expect(study.compare).toEqual({ a: 0, b: "current" });
      expect(study.diagram).not.toBeNull();
      expectDrawable(study.diagram!);
      // Coordinates arrived in 0.4: older studies have none.
      if (sample.release === "0.3.0") expect(study.coordinates ?? null).toBeNull();
      else expect(study.coordinates?.relative).toBe("case9-coordinates.csv");
    });
  }

  for (const sample of samples(".sld")) {
    it(`opens the ${sample.release} diagram ${sample.path.split("/").pop()}`, () => {
      const diagram = deserialize(sample.text);
      expect(diagram.metadata.schemaVersion).toBe("1.0");
      expect(diagram.metadata.caseName).toBe("five_bus.raw");
      expectDrawable(diagram);
      // Saved again, it reads back the same drawing.
      const again = deserialize(serialize(diagram));
      expect(again.nodes).toEqual(diagram.nodes);
      expect(again.edges).toEqual(diagram.edges);
    });
  }
});

describe("diagram versions", () => {
  const base = { metadata: { title: "x" }, nodes: [], edges: [] };

  it("reads a diagram without a version or view as 1.0 at 1:1", () => {
    const d = checkDiagram(base);
    expect(d.viewport).toEqual({ x: 0, y: 0, zoom: 1 });
    expect(JSON.parse(serialize({ ...d, metadata: d.metadata })).metadata.schemaVersion).toBe("1.0");
  });

  it("opens a newer minor version and refuses a newer major one", () => {
    expect(() => checkDiagram({ ...base, metadata: { schemaVersion: "1.7" } })).not.toThrow();
    expect(() => checkDiagram({ ...base, metadata: { schemaVersion: "2.0" } })).toThrow(/newer Stannum \(diagram schema 2\.0\)/);
    expect(() => checkDiagram({ ...base, metadata: { schemaVersion: "two" } })).toThrow(/isn't a version number/);
  });

  it("refuses a study whose diagram is from a newer major version", async () => {
    const study = JSON.parse(samples(".stannum")[0].text);
    study.diagram.metadata.schemaVersion = "2.0";
    await expect(parseStudyFile(JSON.stringify(study))).rejects.toThrow(/study's diagram can't be opened/);
  });
});
