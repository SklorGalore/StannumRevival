// The shape of a study file, checked when one is opened (see studyFile.ts).
import { z } from "zod";
import { STUDY_FORMAT, STUDY_VERSION } from "./studyFile";

const moduleId = z.enum(["diagram", "power", "records", "diff"]);
const edit = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("status"), element: z.string(), online: z.boolean() }),
  z.object({ kind: z.literal("generation"), element: z.string(), mw: z.number() }),
  z.object({ kind: z.literal("load"), bus: z.number(), mw: z.number(), mvar: z.number() }),
]);
const entry = z.object({ edit, was: z.string().optional() });
const side = z.union([z.number().int().nonnegative(), z.literal("current")]);
const fingerprint = z.object({ size: z.number(), sha256: z.string(), modified_ms: z.number().nullable() });

// Solve reports and drawings are checked for shape only at the top: they are
// what this app wrote, and a deep check would cost seconds on a big case.
export const studySchema = z.object({
  format: z.literal(STUDY_FORMAT),
  version: z.literal(STUDY_VERSION),
  app: z.string(),
  savedAt: z.string(),
  case: z.object({
    path: z.string(),
    relative: z.string().nullable(),
    name: z.string(),
    format: z.enum(["matpower", "raw", "stn"]),
    fingerprint,
  }),
  dcGrids: z.object({ path: z.string(), relative: z.string().nullable() }).nullable(),
  coordinates: z.object({ path: z.string(), relative: z.string().nullable() }).nullable().optional(),
  solveSettings: z.looseObject({ model: z.enum(["ac", "dc"]) }),
  edits: z.array(entry),
  solves: z.array(
    z.object({
      name: z.string(),
      entries: z.array(entry),
      report: z.looseObject({ buses: z.array(z.unknown()), branches: z.array(z.unknown()) }),
    })
  ),
  compare: z.object({ a: side, b: side }).nullable(),
  contingencies: z.object({
    branches: z.boolean(),
    generators: z.boolean(),
    method: z.enum(["screen", "ac"]),
    acCount: z.number(),
    limitPct: z.number(),
  }),
  diagram: z.looseObject({ metadata: z.looseObject({}), nodes: z.array(z.unknown()), edges: z.array(z.unknown()) }).nullable(),
  view: z.object({ module: moduleId, split: moduleId.nullable(), splitRatio: z.number() }),
});

