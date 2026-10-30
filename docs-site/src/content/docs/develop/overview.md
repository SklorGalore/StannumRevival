---
title: Codebase tour
description: How the Stannum repository is laid out, and how one click in the window reaches the solver and comes back.
---

This guide is for people working on Stannum itself. It explains where the
code lives, how the pieces talk to each other, and how to change them
safely. For what the solver computes, see the
[technical manual](/StannumRevival/manual/architecture/); for using the app,
see the [user guide](/StannumRevival/guide/shell/).

Everything is in one repository and one Cargo workspace:
[SklorGalore/StannumRevival](https://github.com/SklorGalore/StannumRevival).
rs-power and nodal used to be separate repositories; they were brought in
with their history and are now developed here only.

## Repository layout

```
crates/
  rs-power/       Power flow: MATPOWER/RAW loading, AC/DC flow, HVDC
  nodal/          RAW v35 parser that keeps every record; topology
  stannum-core/   Analysis core: workspace, commands, case diff
  stannum-synth/  Synthetic test cases of any size
  stannum-py/     The `stannum` Python package (PyO3 + Python)
  stannum-cli/    `stannum-cli`: runs run files without the window
src-tauri/        Desktop shell: one command into the core, attach
src/              The React interface
  shell/          Module rail, menu bar, toolbar, Ctrl+K, log dock, split view
  modules/        Diagram, Power flow, Records, Case diff
  lib/analysis/   Typed core calls, generated types, wire decoding
  store/          Zustand stores: workspace, study, layers, log…
  components/     The one-line editor (React Flow), from StannumSLD
  ui/             Shared tables, tabs, inspector and so on
tests/browser/    Browser smoke test (Playwright, real Rust core)
tests/large/      Hashes of large cases too big to commit
scripts/          PowerShell: schemas, icons, releases, CI helpers
docs-site/        This site (Astro + Starlight)
docs/             Release notes for maintainers, PR screenshots
```

## The shape of the app

```
React interface (src/)
  invokeCore("solve_power_flow", { settings })
        │
        ▼
Tauri shell (src-tauri/)
  stannum_command(command, args)        one Tauri command for all
        │
        ▼
stannum-core
  Session::invoke → commands! dispatch → Workspace
        ├── rs-power   power flow, HVDC, sensitivities
        ├── nodal      RAW v35 records, topology
        └── diff/      case comparison
        │
        ▼
JSON text back to the window
```

Three rules hold the design together:

1. **The core has no Tauri dependency.** `stannum-core` is a plain Rust
   library. The desktop shell, the dev bridge used by browser tests, the
   Python package, `stannum-cli` and attach all drive the same core.
2. **Every operation is one command, declared once.** The list in
   `crates/stannum-core/src/commands.rs` is the only list. Each client sends
   a command name and JSON arguments; none keeps its own copy. The
   frontend's types are generated from the commands' schemas, so they can't
   drift. See [The command registry](/StannumRevival/develop/commands/).
3. **One open case, one identity per element.** The workspace parses a
   case once per engine and every module reads from it. Each bus, branch and
   generator has one id, such as `bus:101` or `line:101:102:1`, used by
   every engine and module, so a selection in one module finds the same
   element in the others.

## Following one click

Pressing **Solve** in Power flow goes through these steps:

1. `PowerFlowModule.tsx` calls `api.solve(settings)` in
   `src/lib/analysis/api.ts`.
2. `api.solve` runs `call("solve_power_flow", { settings })`, which writes a
   line in the log strip and calls `invokeCore`. That calls Tauri's
   `invoke("stannum_command", { command, args })`. The argument type comes
   from `commands.gen.ts`, so a misspelt setting fails `tsc`.
3. `stannum_command` in `src-tauri/src/analysis.rs` hands the call to
   `Session::invoke` on a blocking worker thread, so the window stays live.
4. The `commands!` macro in `commands.rs` parses the arguments into a typed
   struct (unknown fields are refused) and runs the command's body.
5. `Session::solve` takes what it needs from the open case, lets go of the
   workspace lock, runs rs-power's Newton solve, then locks again to keep
   the result. Other commands, including `cancel_solve`, answer meanwhile.
6. The report goes back as JSON, with big tables as columns
   (`table.rs`). `decodeSolveReport` in `wire.ts` turns them back into rows,
   and `workspaceStore` keeps the report so every module can show it.
7. While the solve runs, the core's log lines stream to the window every
   150 ms (`stream_log` in `analysis.rs`).

## Where to go next

- [Set up and run](/StannumRevival/develop/setup/): build the app, run the
  interface in a browser, run the checks.
- [Crates](/StannumRevival/develop/crates/): what each crate holds.
- [The command registry](/StannumRevival/develop/commands/) and
  [Add a command](/StannumRevival/develop/add-a-command/).
- [Desktop shell](/StannumRevival/develop/desktop-shell/) and
  [Frontend](/StannumRevival/develop/frontend/).
- [Tests and CI](/StannumRevival/develop/testing/): what runs on a pull
  request and how to land a change.
