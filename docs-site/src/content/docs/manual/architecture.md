---
title: Architecture
description: How Stannum's engines, analysis core, desktop shell and interface fit together.
---

Stannum is a [Tauri 2](https://v2.tauri.app/) desktop app. The analysis runs
in native Rust; the interface is React in the system web view. Everything is
in one Cargo workspace and one repository.

This page is the overview. The
[developer guide](/StannumRevival/develop/overview/) goes through the code
itself: each crate, the command registry, the desktop shell and the
frontend, and how to add a command.

```
┌──────────────────────────────── React interface (src/) ────────────────────────────────┐
│  Module rail · case bar · palette · log dock · side by side                              │
│  Diagram (React Flow + ELK; network: Cytoscape)   Power flow   Case diff                 │
└──────────────────────────────────────┬──────────────────────────────────────────────────┘
                                       │ stannum_command (src-tauri/src/analysis.rs)
┌──────────────────────────────────────┴───────── stannum-core ───────────────────────────┐
│  commands.rs: every command, listed once · Workspace: the open case, parsed once per    │
│  engine · study edits · solve settings                                                    │
│  power.rs   derived_topology.rs   diagram.rs   diff/ (case-diff port)   logbook.rs        │
└───────────────┬────────────────────────────────┬───────────────────────────────────────┘
                │                                │
        ┌───────┴────────┐               ┌───────┴────────┐
        │    rs-power    │               │     nodal      │
        │ MATPOWER, RAW  │               │ RAW v35 parser │
        │ AC/DC flow     │               │ topology via   │
        │ HVDC, controls │               │ rustworkx-core │
        │ sensitivities  │               └────────────────┘
        └────────────────┘
```

## Crates

| Crate | Role |
| --- | --- |
| `crates/rs-power` | Case loading (MATPOWER version 2, PSS/E RAW revisions 23–35), AC Newton and DC power flow, reactive limits, remote regulation, switched shunts and taps, detailed HVDC, sensitivities, PTDF and LODF. Numerical behaviour targets the MATPOWER 8.1 legacy core. |
| `crates/nodal` | A RAW revision 35 parser that keeps every record, and topology analysis (islands, bridges, articulation points, shortest paths) on a graph built with rustworkx-core. |
| `crates/stannum-core` | The analysis core, with no Tauri dependency. Holds the open case, shapes engine results for the interface, applies study edits, derives a topology from the power flow model when nodal can't read a file, builds the one-line network, and contains the Rust port of case-diff. |
| `crates/stannum-py` | The `stannum` Python package: PyO3 bindings over the command list (`src/lib.rs`) and the Python layer on top (`python/stannum/`), built into abi3 wheels with maturin. |
| `src-tauri` | The desktop shell. `analysis.rs` passes core commands to the core's command list through one Tauri command. |

## Commands

Every operation the interface asks of the core (open, solve, run
contingencies, read records and so on) is a **command** in
`crates/stannum-core/src/commands.rs`. Each command is declared once there,
with its arguments and a one-line description. The desktop shell sends them
all through a single Tauri command, `stannum_command`, and the `dev_bridge`
test server answers the same list over HTTP, so the two can't drift apart.
The Python package (`crates/stannum-py`, see
[Python](/StannumRevival/guide/python/)) calls the same list in-process.

Commands that work on a case take an optional `case` argument: the `id` from
the case summary. Left out, a command uses the open case. Given, it must name
the open case, so a caller holding an old id gets an error rather than
reaching a different case. `list_commands` returns every command with its
description and the API version.

Every command declares its result type too, and the core writes a JSON
Schema of each command's arguments and result (`command_schemas`, checked in
as `crates/stannum-core/schema/commands.schema.json`). The frontend's
`src/lib/analysis/commands.gen.ts` is generated from it, so a call with a
misspelled argument, or a hand-written type that no longer matches what the
core sends, fails the TypeScript build. A test runs every command against
real cases and checks each answer against its schema. After changing a
command or a type it returns, run `pwsh scripts/update-schemas.ps1`.

A command that fails returns an error with a **kind** and a message, such as
`{"kind": "no_case", "message": "Open a case first."}`. The kinds are
`unknown_command`, `invalid_arguments`, `no_case`, `case_not_open`,
`not_ready` (another step comes first, such as solving), `not_found`,
`unsupported` (not for this case's format), `case_changed` (run it again),
`cancelled`, `file`, `failed` and `internal`. Clients act on the kind and show
the message.

Long runs (solves and contingency runs) can be cancelled with `cancel_solve`
and `cancel_contingencies`, and `contingency_progress` reports how far a run
has got. These work while the run is going, from any client.

Long work doesn't hold the workspace while it runs: a solve, sensitivities,
"Trip and compare", contingency runs, the first read of a case's records and
Records edits each take what they need from the open case (shared, not
copied), run, and lock the workspace again only to keep their results. Other
commands answer meanwhile. Every open case has a version that changes when
the case itself changes (study edits, Records edits, opening another case).
Results computed for an older version are set aside with a message instead
of being stored on the changed case; Records reads and edits are redone on
the case as it is now.

## The workspace

When a case opens, the workspace reads it once for each engine that can use
it: rs-power for power flow and the diagram, nodal for topology (revision 35
RAW only), and the case-diff parser (revisions 33 and 35). Each engine's
result is kept separately, so a file one engine can't read still works in the
others, and the interface shows the reason a module is unavailable.

Every element has one **shared identity** across engines, built from its kind
and its RAW or MATPOWER key (for example a branch's from bus, to bus and
circuit). Selection, study edits, diagram symbols and diff records all use it,
which is what lets a selection in one module find the same element in
another.

A [study](/StannumRevival/guide/studies/) is a list of edits applied to a copy
of the power flow case before each solve. The file is never changed.

## Results on the wire

A solve report goes to the interface as JSON. Its bus, generator and branch
tables travel as one array per field rather than one object per row, with
flows rounded to 0.1 kW and per-unit voltages to six decimals; the interface
turns them back into rows (`src/lib/analysis/wire.ts`). On a 78,400-bus case
this cuts the report from 62 MB to 22 MB.

The topology snapshot travels the same way (`topology_wire` in stannum-core,
`src/lib/analysis/topologyWire.ts`). Graph nodes and connections refer to
buses and equipment by position, so their ids and labels are rebuilt rather
than sent, and each record's RAW fields stay in the core until the inspector
asks for one element's (`topology_source`). At 12,000 buses the snapshot is
2.3 MB instead of 14 MB; the Texas 2,000-bus RAW case's is 0.9 MB instead of
10 MB.

A solve can be cancelled: the shell
hands the core a flag, which the Newton loop checks before each step.

## Logging

Every crate logs through the Rust `log` facade. stannum-core installs a
logger that keeps a ring of the latest 5,000 lines (debug and above for
Stannum's own crates, warnings and above for dependencies). The desktop shell
streams new lines to the interface every 150 ms, and the [log
panel](/StannumRevival/guide/log/) shows them.

## Testing

- Rust unit and integration tests for each crate, including MATPOWER 8.1
  reference comparisons (see [Verification](/StannumRevival/reference/verification/)).
- Frontend unit tests with Vitest.
- A browser smoke test drives the real interface in Chromium against the real
  Rust core. A small HTTP server (`dev_bridge`) serves the same commands as the
  desktop shell, and the test replaces Tauri's IPC with a shim that calls it.
