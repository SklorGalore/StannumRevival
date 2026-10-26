---
title: Architecture
description: How Stannum's engines, analysis core, desktop shell and interface fit together.
---

Stannum is a [Tauri 2](https://v2.tauri.app/) desktop app. The analysis runs
in native Rust; the interface is React in the system web view. Everything is
in one Cargo workspace and one repository.

```
┌──────────────────────────────── React interface (src/) ────────────────────────────────┐
│  Module rail · case bar · palette · log dock · side by side                              │
│  Diagram (React Flow + ELK)   Power flow   Topology (Cytoscape)   Case diff              │
└──────────────────────────────────────┬──────────────────────────────────────────────────┘
                                       │ Tauri commands (src-tauri/src/analysis.rs)
┌──────────────────────────────────────┴───────── stannum-core ───────────────────────────┐
│  Workspace: the open case, parsed once per engine · study edits · solve settings        │
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
| `src-tauri` | The desktop shell. `analysis.rs` exposes the core's workspace as Tauri commands. |

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
