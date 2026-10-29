---
title: Study file format
description: "The .stannum file: a JSON record of a piece of work on one case, without the case itself."
---

A study file (`.stannum`) keeps a piece of work on one case: the study edits,
the saved solves, the settings, the one-line drawing and which modules were
showing. [Studies](/StannumRevival/guide/studies/) explains how to save and
open one. The case itself isn't copied in; the study names it by path and
fingerprint.

The file is UTF-8 JSON, written by the app. It isn't meant to be written by
hand, but it is plain enough to read, compare in version control or generate
from a script.

## Example

Shortened: the saved solve's report and the drawing are cut down.

```json
{
  "format": "stannum-study",
  "version": 1,
  "app": "0.4.1",
  "savedAt": "2026-10-28T20:35:16.252Z",
  "case": {
    "path": "C:/Users/engineer/cases/case9.m",
    "relative": "case9.m",
    "name": "case9.m",
    "format": "matpower",
    "fingerprint": { "size": 2235, "sha256": "ee50fc7b…", "modified_ms": 1793219542598 }
  },
  "dcGrids": null,
  "coordinates": { "path": "C:/Users/engineer/cases/case9-coordinates.csv", "relative": "case9-coordinates.csv" },
  "solveSettings": { "model": "ac", "start": "last", "tolerance_mva": 2, "max_iterations": 10 },
  "edits": [
    { "edit": { "kind": "status", "element": "line:4:5:1", "online": false } },
    { "edit": { "kind": "generation", "element": "generator:2:1", "mw": 120 }, "was": "163.0 MW" }
  ],
  "solves": [{ "name": "Case as filed", "entries": [], "report": { "converged": true, "buses": [], "branches": [] } }],
  "compare": { "a": 0, "b": "current" },
  "contingencies": { "branches": true, "generators": true, "method": "screen", "acCount": 50, "limitPct": 100 },
  "diagram": { "metadata": { "title": "case9", "schemaVersion": "1.0" }, "nodes": [], "edges": [], "viewport": { "x": 0, "y": 0, "zoom": 1 } },
  "view": { "module": "power", "split": null, "splitRatio": 0.5 }
}
```

## Fields

| Field | Meaning |
| --- | --- |
| `format` | Always `"stannum-study"`. A file without it isn't opened as a study. |
| `version` | The study format version, a whole number. See [Versions](#versions). |
| `app` | The Stannum version that wrote the file, for information only. |
| `savedAt` | When it was saved, ISO 8601 in UTC. |
| `case` | The case the study is about; see [Finding the case](#finding-the-case). |
| `dcGrids` | The attached [DC grid file](/StannumRevival/reference/dc-grid-file/), as `{ path, relative }`, or `null`. |
| `coordinates` | The attached [bus coordinates file](/StannumRevival/reference/coordinates-file/), as `{ path, relative }`, or `null`. Missing from studies saved before 0.4, which read as `null`. |
| `solveSettings` | The power flow settings, with the names and values listed under [run file settings](/StannumRevival/reference/run-file/#settings). A setting that is missing takes its default. |
| `edits` | The study edits, in the order they apply; see [Edits](#edits). |
| `solves` | Saved solves: each has a `name`, the edits it used (`entries`, as in `edits`) and the solve `report` as the core returned it. |
| `compare` | Which two solves Compare shows: `a` and `b` are each a position in `solves` (from 0) or `"current"`. `null` when nothing is being compared. |
| `contingencies` | The contingency settings: `branches` and `generators` (true or false), `method` (`"screen"` or `"ac"`), `acCount` and `limitPct`. |
| `diagram` | The one-line drawing, in the same shape as a [diagram file](/StannumRevival/reference/diagram-file/), or `null`. |
| `view` | The modules showing: `module` and `split` (one of `"diagram"`, `"power"`, `"records"`, `"diff"`; `split` is `null` with one pane) and `splitRatio` (0 to 1). |

### Finding the case

`case` has:

- `path`: the absolute path when the study was saved, with the separators of
  the system it was saved on;
- `relative`: the same file from the study's folder, with `/` separators, or
  `null` when there's no such path (another drive on Windows);
- `name`: the file name;
- `format`: `"matpower"` or `"raw"`;
- `fingerprint`: `size` in bytes, `sha256` (lowercase hex) of the file's
  bytes, and `modified_ms` (milliseconds since 1970, or `null`).

Opening a study looks for the case at `path`, then at `relative` from the
study's folder, then for a file called `name` beside the study. If the
SHA-256 differs, the study still opens and says the case has changed. The DC
grid and coordinates files are found the same way.

### Edits

Each entry is `{ "edit": …, "was": … }`. `was` is the text shown for the
value before the edit ("163.0 MW"), and may be missing. `edit` is one of:

| `kind` | Other fields | Meaning |
| --- | --- | --- |
| `"status"` | `element`, `online` | Take a branch or generator out of service (`false`) or put it back (`true`). |
| `"generation"` | `element`, `mw` | Set a generator's active power. |
| `"load"` | `bus`, `mw`, `mvar` | Set a bus's total load. |

`element` is a shared element id such as `line:4:5:1`, `transformer:1:4:1` or
`generator:2:1` (kind, bus numbers in ascending order, then circuit or machine
id).

## Versions

`version` is a whole number that goes up only when an older Stannum could not
read the file correctly: a field changes meaning, or one an older version
needs is taken away. Adding a field that older versions can do without (as
`coordinates` was in 0.4) does not change it.

- A Stannum opens every study version up to its own, converting older ones as
  it reads them.
- A study with a higher `version` than the Stannum reading it is refused with
  "This study was saved by a newer Stannum", not opened partly.
- Fields Stannum doesn't know are ignored, and aren't kept when the study is
  saved again.

| `version` | Written by | Changes |
| --- | --- | --- |
| 1 | 0.3.0 and later | The first version. 0.4 added `coordinates`. |

Throughout 1.x, Stannum keeps opening every study a 1.x release wrote. Studies
saved by 0.3.0 and 0.4.1 are kept in the repository under
[`tests/compat`](https://github.com/SklorGalore/StannumRevival/tree/development/tests/compat)
and opened by the tests on every change.
