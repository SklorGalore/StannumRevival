---
title: Crates
description: What each Rust crate in the workspace holds, what it depends on, and where to look inside it.
---

The Cargo workspace (`Cargo.toml` at the root) has nine members. The arrows
are path dependencies:

```
 stannum-py ──┐
 stannum-cli ─┼──► stannum-core ──► rs-power
 src-tauri ───┘          │    └───► nodal
                         └ (dev) ─► stannum-synth ──► rs-power
 rs-power, stannum-core ── (dev) ─► stannum-verify
```

None of the crates is published to crates.io (`publish = false`).

## rs-power

`crates/rs-power`: the power flow engine. Its numerical behaviour targets
the MATPOWER 8.1 legacy core, and its tests compare against MATPOWER
results.

| Module | Holds |
| --- | --- |
| `case.rs` | `Case`, `Bus`, `Generator`, `Branch` in MATPOWER column order and units. |
| `input.rs`, `output.rs` | Reading and writing MATPOWER version 2 files, as literal data (no MATLAB is run). |
| `raw.rs` | PSS/E RAW revisions 23–35 converted to a `Case`, following MATPOWER's `psse_read`/`psse_convert`, with source-located warnings. |
| `network.rs`, `solver.rs` | Admittance matrices and the sparse Newton–Raphson AC and linear DC solves, with reactive limits and remote regulation. |
| `controls.rs` | Switched shunts and transformer tap changers, applied between Newton solves. |
| `dc.rs`, `vsc.rs`, `mtdc.rs`, `vsc_grid.rs` | HVDC: two-terminal LCC, two-terminal VSC, multi-terminal LCC, and meshed VSC grids (Beerten et al. 2012). |
| `sensitivity.rs`, `outage.rs` | AC sensitivities, PTDF, LODF and linear outage screening from one factorization. |

It has its own README, CHANGELOG and `AGENTS.md` with notes on its
conventions. `tests/` holds MATPOWER reference comparisons and one file per
model; `tests/large_cases.rs` is ignored by default and runs on the
[large cases](https://github.com/SklorGalore/StannumRevival/tree/main/tests/large).
`benches/power_flow.rs` is a plain timing benchmark (`cargo bench -p rs-power`) and
`examples/power_flow.rs` solves a file from the command line.

The [technical manual](/StannumRevival/manual/ac-power-flow/) documents the
equations each module implements.

## nodal

`crates/nodal`: a RAW revision 35 parser that keeps every record and field
(`parser.rs`, `model.rs`, laid out by `raw.schema.json`) and topology
analysis on a rustworkx-core graph (`topology.rs`): islands, bridges,
articulation buses and fewest-hop paths. The public API is `parse_raw`,
`analyze` and `shortest_path`. With the `schemars` feature, which
stannum-core turns on, its types also describe themselves as JSON Schema.

Element ids such as `bus:101` and `line:101:102:1` started in nodal; the
core's shared ids follow them, so topology results need no translation.

## stannum-core

`crates/stannum-core`: the analysis core every client drives. It depends on
rs-power and nodal and has no Tauri or UI dependency.

| Module | Holds |
| --- | --- |
| `commands.rs` | The command registry: `Session` and the `commands!` list. See [The command registry](/StannumRevival/develop/commands/). |
| `workspace.rs` | `Workspace` and `OpenCase`: the open case, parsed once per engine, its version, solves, study edits and side files. |
| `error.rs` | `Error` and `ErrorKind`, the typed error every command returns. |
| `power.rs` | rs-power solves and sensitivities, shaped into `SolveReport` and friends for the interface. |
| `contingency.rs` | N-1 analysis: linear screen, then AC on the worst. Runs as a job that can be cancelled and followed. |
| `study.rs`, `working_case.rs` | Study edits applied to a copy of the case, and written back into RAW text for the working-case diff. |
| `records.rs`, `record_edit.rs` | Every record type as a table for the Records module, and cell edits written into the file's text. |
| `elements.rs` | Shared element ids and the list of kinds (`KINDS`), exported to the frontend. |
| `derived_topology.rs`, `topology_wire.rs` | Topology from the power model when nodal can't read a file, and the compact snapshot sent to the window. |
| `diagram.rs`, `inspect.rs` | The case as a drawable network, and one element's data and limits for the inspector. |
| `diff/` | The Rust port of case-diff: tokenizer, exact decimal values, parser, comparison. |
| `dc_grids.rs`, `geo.rs` | Side files: Stannum DC grid files and bus coordinate CSVs. |
| `results.rs` | Names for every solution a case can show (`base`, `study`, `contingency 3`, `trip …`). |
| `run_file.rs` | Run files (`.stnrun`): reading, checking, writing and the list of commands that count as actions. |
| `study_file.rs` | Fingerprints and relative paths for study files (`.stannum`). |
| `attach.rs` | The local socket a Python script uses to drive the running app. |
| `schema.rs`, `table.rs` | JSON Schema helpers and TypeScript generation; tables sent as columns. |
| `logbook.rs` | The log ring buffer every crate writes into through the `log` facade. |

`schemas/raw.schema.json` holds the RAW field layouts shared with nodal and
case-diff. `schema/commands.schema.json` is the generated command schema.

`examples/` has three small binaries:

- `dev_bridge`: serves the command list over HTTP for browser work and the
  smoke test.
- `print_schemas`: prints the command schemas.
- `diff_dump`: prints a full case comparison as JSON, in the shape of the
  original Python case-diff, for checking the port against it.

`tests/` runs the commands end to end on real fixtures. The ones that guard
the whole design are `schemas.rs` (every command's answers fit its schema),
`lock_free_work.rs` (long work doesn't hold the workspace),
`readers_agree.rs` (the three RAW readers see the same case) and
`elements.rs` (every engine names elements the same way). `budgets.rs` is
ignored by default; CI runs it in release mode on a 12,000-bus synthetic
case and fails if a command goes over its time or size budget.

## stannum-synth

`crates/stannum-synth`: synthetic cases of any size, built with the method
of Birchfield et al. (2017) from a seed, so nothing needs downloading. The
modules follow the paper's pipeline: `substations`, `lines`, `build`, then
`validate` for the structural statistics. stannum-core uses it as a
dev-dependency to build test cases. See
[Synthetic test cases](/StannumRevival/manual/synthetic-cases/).

## stannum-verify

`crates/stannum-verify`: a small dev-dependency the reference tests use to
record how close they come to their references. A test makes a `Check`
for each quantity, passes it every difference and calls `finish()`, which
asserts the largest one is within the limit and, when `STANNUM_VERIFY`
names a file, appends it there. Its `collect` binary turns those lines
into `docs-site/src/data/verification.json`, the tables on the
[Verification](/StannumRevival/reference/verification/) page;
`scripts/verify.ps1` runs both.

## stannum-py

`crates/stannum-py`: the `stannum` Python package. It has two halves:

- `src/lib.rs`, built by maturin into `stannum._native` (an abi3 wheel for
  Python 3.10+). It holds one `Session` class with `invoke(command, json)`.
  Each call runs on its own thread with the GIL released; Ctrl+C sends the
  cancel commands and raises `KeyboardInterrupt`.
- `python/stannum/`: everything Pythonic. `__init__.py` re-exports the
  public names and nothing else; `_api.py` holds `Case`, `App` and the
  functions, whose methods such as `solve`, `contingencies` and `records`
  are thin wrappers over `invoke`; tables come back as NumPy columns
  (`_table.py`); error kinds become typed exceptions (`_errors.py`);
  `_attach.py` connects to a running app. `tests/test_public_api.py` lists
  the public API, which stays compatible across 1.x.

Any command works from Python without new code, through `case.call(name,
**args)`. Wrapper methods are for commands people use often. Tests in
`crates/stannum-py/tests` run against the installed wheel
(`pwsh scripts/ci/test-python.ps1`). User docs: [Python](/StannumRevival/guide/python/).

## stannum-cli

`crates/stannum-cli`: one `main.rs` that runs (`run`), checks (`check`) and
lists commands for (`commands`) run files, on a `Session` of its own. Exit
code 0 means every line ran, 1 that a line failed or the run was
cancelled, 2 that the file or command line is wrong. Release builds attach
it to each GitHub release (`scripts/release/build-cli.ps1`).

## src-tauri

`src-tauri` is the desktop shell, a workspace member like the rest. See
[Desktop shell](/StannumRevival/develop/desktop-shell/).
