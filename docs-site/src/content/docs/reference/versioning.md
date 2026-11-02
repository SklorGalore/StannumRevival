---
title: Versions and compatibility
description: What a Stannum version number promises for files, Python scripts, stannum-cli, run files and attach, and how anything is retired.
---

Stannum's version numbers follow [Semantic Versioning](https://semver.org/):
**major.minor.patch**. The app, `stannum-cli` and the `stannum` Python
package are released together and always carry the same number, so
"Stannum 1.4" means all three.

From **1.0.0** on:

| Release | What it may do |
|---|---|
| **Patch** (1.4.0 → 1.4.1) | Fix bugs. Nothing you rely on changes, apart from the bug. |
| **Minor** (1.4 → 1.5) | Add things: commands, arguments, fields, file contents, Python methods. Mark things deprecated. Nothing is removed, and nothing changes meaning. |
| **Major** (1.x → 2.0) | Remove what was deprecated, and make other breaking changes. The changelog lists each one with what to do instead. |

So a file, script or run file that works with 1.2 works with every later
1.x. Before 1.0 (the 0.x releases) any minor release may break things; the
changelog says what changed.

## What is covered

Six things make up the contract. Anything not listed here, such as the
window's layout, log wording or the Rust crates' own APIs, can change in
any release.

### Files Stannum writes

The [study](/StannumRevival/reference/study-file/) (`.stannum`),
[diagram](/StannumRevival/reference/diagram-file/) (`.sld`),
[run file](/StannumRevival/reference/run-file/) (`.stnrun`),
[DC grid file](/StannumRevival/reference/dc-grid-file/) and
[bus coordinates file](/StannumRevival/reference/coordinates-file/):

- Every 1.x release opens a file any earlier 1.x release saved, and reads it
  the same way.
- A minor release may add **optional** fields and new choices. A file
  without them reads as it always did.
- The promise runs forwards only: an older release isn't guaranteed to open
  a file a newer one saved. Each file carries a version: `version` in study
  and DC grid files, `metadata.schemaVersion` in diagrams, and a first
  comment line such as `# stannum-run-file version 1` in run files and
  coordinates files. A release refuses a file whose version is newer than it
  reads, saying to update Stannum, instead of reading it wrong. Each format's
  page has its version history. The DC grid file
  also refuses fields it doesn't know, to catch typos. Share files with
  colleagues on the same or a newer release.
- A file's version only goes up when an older release couldn't read the
  new file correctly.
- Files saved by earlier releases are kept in the repository's
  `tests/compat` folder, and the tests open them on every change.

The [Stannum case](/StannumRevival/reference/case-file/) (`.stn`, `.stnb`) and
[lists](/StannumRevival/reference/lists-file/) (`.stnl`) formats reached
format version 1.0 with Stannum 2.0 and are under this promise from then
on, with one difference: a release reads a newer *minor* version of them,
keeping what it doesn't know, and refuses only a newer major version. The
0.x preview files that Stannum 1.x wrote still open.

### Files Stannum reads from other tools

PSS/E RAW and MATPOWER cases: what a 1.x release reads, every later 1.x
reads too. Support only grows. A **fix** to how a record is read or solved
may change results; such fixes are listed in the changelog under *Fixed*.
RAW and MATPOWER files Stannum **writes** stay readable by the tools they
are for; their exact text (spacing, comments, order of optional records)
may change.

### Results

Solve reports, contingency rankings and other results keep their fields and
units. The numbers themselves are what the solver finds: a bug fix, a
better starting point or a tighter control loop may move them within the
solve tolerance, or more when the old answer was wrong. Those changes are
listed in the changelog.

### Commands: run files, Python's `call`, and attach

Run files, `Case.call` and `App.call` in Python, and attach all use one
list of commands (`stannum-cli commands`, or `stannum.commands()`). Each
command has a name, arguments and a result, all described as JSON Schema by
`stannum.schemas()`.

Across 1.x:

- A command keeps its name, and its arguments keep their names, types and
  meaning.
- A minor release may add commands, add **optional** arguments, add fields
  to a result and add values to a list of choices (a new solve `method`, a
  new error kind). Scripts should ignore result fields they don't know.
- Error **kinds** (`not_found`, `not_ready`, …) are part of the contract;
  error **messages** are for people and can change.
- The command list's version, `API_VERSION`, stays the same through 1.x. It
  only goes up with a major release, when a command is removed or changes in
  a way existing callers would notice.

A run file written for 1.2 runs on 1.5. One that uses a command added in
1.5 fails on 1.2 before its first line runs, naming the unknown command.

### The `stannum` Python package

The public API is what `stannum.__all__` lists, and the documented methods,
properties and attributes of the classes in it:

- **Functions:** `open`, `diff`, `attach`, `run_file`, `commands`,
  `schemas`, `log`, and the study edits `outage`, `restore`, `generation`,
  `bus_load`.
- **Classes:** `Case`, `App`, `Diff`, `Table`, `RecordTable`, `Answer`.
  Get them from the functions above; their constructors aren't public.
- **Errors:** `StannumError` and one subclass per error kind.
- **Values:** `__version__` and `API_VERSION`.

Not public, and free to change in any release: modules and names starting
with `_` (`stannum._native`, `stannum._api`, `_table`, `_errors`,
`_attach`), constructors, and anything undocumented. The package's tests
hold the full public list, so it only changes on purpose.

Within 1.x, a public function or method keeps its name, and its parameters
keep their names, order and defaults; new parameters are keyword arguments
with defaults. Answers are dicts of command results, so they follow the
command rules above.

The package needs Python 3.10 or newer through 1.x. Dropping a Python
version that is past its
[end of life](https://devguide.python.org/versions/) is allowed in a minor
release, and is listed in the changelog.

### `stannum-cli`

The subcommands `run`, `check`, `commands` and `version`, their options
(`--keep-going`, `--json`, `--verbose`) and the exit codes stay as they
are:

| Exit code | Means |
|---|---|
| 0 | Every line ran. |
| 1 | A line failed, or the run was cancelled. |
| 2 | The file or the command line is wrong. |

`--json` prints the run report, which follows the command rules above. The
plain text output is for people and can change.

`stannum-cli mcp` (the [MCP server](/StannumRevival/guide/ai-assistants/)
for AI assistants) is a preview and not part of this promise: its tools are
written for assistants and can change in a minor release.

### Attach

A script attaches to the app with a local connection whose handshake
carries two numbers: the attach **protocol** (how messages are framed,
version 1) and the command list's `API_VERSION`. Both stay the same
through 1.x, so any 1.x `stannum` package attaches to any 1.x app. A script
on a newer package that calls a command the older app doesn't have gets
`UnknownCommand`.

The `ui.*` commands (`ui.show`, `ui.go`, `ui.run`, …) follow the command
rules too. The ids of Ctrl+K commands that `ui.run` takes may change with
the window and aren't covered; look them up with `App.commands()`.

## Deprecation

Nothing covered above is removed or changed without warning first:

1. A minor release marks it **deprecated**: the changelog lists it under
   *Deprecated* with what to use instead, and its documentation says so.
   It keeps working.
2. Where it can, Stannum warns when it's used: Python raises a
   `DeprecationWarning`, and run files, `stannum-cli` and attach write a
   warning in the log.
3. It stays until at least the next major release, and is removed there.
   The changelog lists the removal under *Removed*.

A thing is deprecated for at least one minor release before the major
release that removes it, so there is always a release where both the old
and the new way work.

## Changing it (for contributors)

- A change that adds something is fine in any minor release. Note it under
  *Added* in `CHANGELOG.md`.
- A change that would break something covered here waits for the next
  major release. Until then add the new way beside the old one and
  deprecate the old one.
- Changing Python's public names means changing the list in
  `crates/stannum-py/tests/test_public_api.py` too. That test failing is
  the reminder to check this page.
- Bump a file's `version` only when an older release couldn't read the new
  file correctly, and bump `API_VERSION` only in a major release.
