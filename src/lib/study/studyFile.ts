// The study file (.stannum): JSON holding everything about a piece of work
// on one case, except the case itself. The case is referred to by path (and
// by path relative to the study, so the two can move together) and by its
// SHA-256, so opening the study can tell when the case file has changed.
import type { ModuleId } from "../../store/workspaceStore";
import type { SLDDiagram } from "../../store/types";
import type { FileFingerprint, SolveReport, SolveSettings, StudyEdit } from "../analysis/types";

export const STUDY_FORMAT = "stannum-study";
export const STUDY_VERSION = 1;
export const STUDY_FILTERS = [{ name: "Stannum study", extensions: ["stannum"] }];
export const STUDY_EXTENSION = /\.stannum$/i;

/** A compare side: a saved solve by its place in `solves`, or what is solved now. */
export type StudyCompareSide = number | "current";

export interface StudyFile {
  format: typeof STUDY_FORMAT;
  version: typeof STUDY_VERSION;
  /** The app version that wrote it. */
  app: string;
  /** ISO 8601. */
  savedAt: string;
  case: {
    path: string;
    /** From the study's folder, with "/" separators; null when there is none. */
    relative: string | null;
    name: string;
    format: "matpower" | "raw";
    fingerprint: FileFingerprint;
  };
  /** The attached DC grid file, if any. */
  dcGrids: { path: string; relative: string | null } | null;
  solveSettings: SolveSettings;
  edits: { edit: StudyEdit; was?: string }[];
  solves: { name: string; entries: { edit: StudyEdit; was?: string }[]; report: SolveReport }[];
  compare: { a: StudyCompareSide; b: StudyCompareSide } | null;
  contingencies: { branches: boolean; generators: boolean; method: "screen" | "ac"; acCount: number; limitPct: number };
  /** The one-line drawing, when there is one. */
  diagram: SLDDiagram | null;
  view: { module: ModuleId; split: ModuleId | null; splitRatio: number };
}

/**
 * Read a study file, or say plainly why it can't be read. The schema (and
 * zod with it) loads on first use, so it stays out of the startup bundle.
 */
export async function parseStudyFile(text: string): Promise<StudyFile> {
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error("This isn't a Stannum study: the file isn't JSON.");
  }
  const head = json as { format?: unknown; version?: unknown } | null;
  if (!head || head.format !== STUDY_FORMAT) throw new Error("This isn't a Stannum study file.");
  if (typeof head.version === "number" && head.version > STUDY_VERSION)
    throw new Error(`This study was saved by a newer Stannum (study format ${head.version}). Update Stannum to open it.`);
  const { studySchema } = await import("./studySchema");
  const parsed = studySchema.safeParse(json);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    throw new Error(`The study file is damaged: ${issue.path.join(".") || "file"}: ${issue.message}`);
  }
  return parsed.data as unknown as StudyFile;
}

export function serializeStudyFile(file: StudyFile): string {
  return JSON.stringify(file);
}

/** Saved solves keep their compare pairing by position in the file. */
export function compareToFile(
  compare: { a: number | "current"; b: number | "current" } | null,
  ids: number[]
): StudyFile["compare"] {
  if (!compare) return null;
  const at = (c: number | "current") => (c === "current" ? c : ids.indexOf(c));
  const a = at(compare.a);
  const b = at(compare.b);
  return a === -1 || b === -1 ? null : { a, b };
}

/** "case300.m changed since …": how long ago, in words, for the warning. */
export function describeChange(saved: FileFingerprint, now: FileFingerprint): string | null {
  if (saved.sha256 === now.sha256) return null;
  const sizeNote = saved.size === now.size ? "" : ` (${saved.size.toLocaleString()} bytes then, ${now.size.toLocaleString()} now)`;
  return `its contents are different from when the study was saved${sizeNote}`;
}
