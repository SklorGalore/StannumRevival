<img src="src/assets/stannum-logo.svg" alt="" width="96" align="right">

# Stannum

A desktop workbench for power system engineers, built with Tauri 2, Rust and
React. Its modules share a single open case:

| Module | What it does | Engine |
|---|---|---|
| **Diagram** | Generate one-lines from the open case with power flow results on them, or draw, edit and export your own; draw the network graph or a map, with topology layers and tables | React Flow + [ELK](https://eclipse.dev/elk/), Cytoscape, MapLibre |
| **Power flow** | AC and DC power flow with voltage, flow and area interchange controls, limit checks, result tables, sensitivities, N-1 contingencies and studies | [rs-power](crates/rs-power) |
| **Records** | Every record in the open case as a table, to sort, filter and edit | stannum-core |
| Topology (in Diagram) | Islands, bridges, cut buses and shortest paths, as layers on any drawing and as tables | [nodal](crates/nodal) |
| **Case diff** | Exact record-by-record comparison of two RAW cases | Rust port of [case-diff](https://github.com/SklorGalore/case-diff) |

All analysis runs locally in native Rust. Case files never leave your machine.

**Documentation:** the [user guide and technical manual](https://sklorgalore.github.io/StannumRevival/)
cover every module and the solver models behind them. Their source is in
[`docs-site/`](docs-site).

![Diagram, Network style: bridges and cut buses ringed, a shortest path in blue, and the Layers panel](docs/screenshots/topology.png)

| Power flow | Case diff |
|---|---|
| ![Power flow overview](docs/screenshots/power-overview.png) | ![Case diff with a changed bus record](docs/screenshots/case-diff.png) |

![Diagram module: a one-line generated from a five-bus RAW case with power flow results](docs/screenshots/diagram.png)

## Using it

Open a case with **Open case…** or drop a `.raw` / `.m` file on the window. The
case bar shows what loaded, and each module uses the case if it can:

- **PSS/E RAW** revisions 23–35 and **MATPOWER** (`.m`, version 2) work in
  Power flow, Records and Diagram.
- Topology reads revision 35 RAW files itself; for other files it builds the
  network from the power flow model.
- Case diff compares RAW revisions 33 and 35.

If a module can't use the open case it says why, and the other modules keep
working. The [user guide](https://sklorgalore.github.io/StannumRevival/guide/shell/) walks through every module and the
[technical manual](https://sklorgalore.github.io/StannumRevival/manual/architecture/) gives the models behind them.

- **Power flow.** AC by Auto (decoupled, finishing in Newton), full Newton,
  fixed-slope decoupled or fast decoupled XB/BX, or linear DC. Generator
  reactive limits, remote regulation, switched shunts in every RAW mode (1–6),
  tap changers and reactive flow taps, phase shifters, FACTS devices
  (STATCOM, UPFC), area interchange and detailed HVDC (two-terminal LCC and
  VSC, multi-terminal and meshed VSC grids). Shunts, taps and phase shifters
  can be locked where they are.
  Result tables page from the core, so they stay quick on 80,000-bus cases.
  See [Power flow](https://sklorgalore.github.io/StannumRevival/guide/power-flow/).
- **Contingencies.** N-1, N-2, bus outages, NERC TPL-001 events P1, P2.1
  and P7.2, and your own lists, checked against rate A, B or C, screened
  with linear outage factors and the worst checked in AC (or all of them in
  AC), ranked by the overloads and voltage problems they cause, and shown on
  the diagram. See
  [Contingencies](https://sklorgalore.github.io/StannumRevival/guide/contingencies/).
- **Transfer and voltage limits.** FCITC with PTDFs and OTDFs between areas,
  zones or bus sets; PV curves to the nose; QV curves for reactive margin.
  See [Transfer limits](https://sklorgalore.github.io/StannumRevival/guide/transfers/),
  [PV curves](https://sklorgalore.github.io/StannumRevival/guide/pv-curves/) and
  [QV curves](https://sklorgalore.github.io/StannumRevival/guide/qv-curves/).
- **Studies.** Edits every solve uses without changing the file (outages,
  generator output, loads, switch states), with saved solves to compare.
  Every change is kept in the History and can be saved as a run file. See
  [Studies](https://sklorgalore.github.io/StannumRevival/guide/studies/).
- **Records.** Every record and field of the case, sorted, searched and
  filtered in the core; edits are written back into the case. See
  [Records](https://sklorgalore.github.io/StannumRevival/guide/records/).
- **Export.** PSS/E RAW 33 or 35 from any open case, with its Records and
  study edits (RAW 33 and 35 files are written record by record), and
  MATPOWER. See [PSS/E RAW](https://sklorgalore.github.io/StannumRevival/reference/psse-raw/#writing-raw).
- **Diagram.** One-lines generated around a bus, an area or the whole case,
  with buses, generators, loads, shunts, lines, transformers, DC lines and
  switching devices, and results on them; a hand-drawing editor with
  right-angle wire routing; the network graph and a map from bus coordinates
  or RAW substation data. Exports SVG, PNG and PDF. See
  [Diagram](https://sklorgalore.github.io/StannumRevival/guide/diagram/).
- **Case diff.** Exact record-by-record comparison of two RAW cases. Numbers
  compare exactly, formatting and comments are ignored, and layouts that are
  not verified are listed as unsupported. See [Case diff](https://sklorgalore.github.io/StannumRevival/guide/case-diff/).
- **Scripting.** Run files (`.stnrun`) recorded from the app and replayed in
  it or by `stannum-cli`, a Python package, `stannum`, and an MCP server
  (`stannum-cli mcp`) for Claude and other AI assistants. See
  [Run files](https://sklorgalore.github.io/StannumRevival/guide/run-files/), [Python](https://sklorgalore.github.io/StannumRevival/guide/python/) and [AI assistants](https://sklorgalore.github.io/StannumRevival/guide/ai-assistants/).

The bottom strip and the log (**Ctrl+`**) say what Stannum is doing and keep
every step, including a solver report of each power flow. What isn't modelled
is listed in [What isn't modelled](https://sklorgalore.github.io/StannumRevival/reference/limitations/).

## Install

Installers for Windows, macOS (Apple silicon) and Linux are on the
[releases page](https://github.com/SklorGalore/StannumRevival/releases/latest).
See [docs/releasing.md](docs/releasing.md) for how a release is cut, and
[CHANGELOG.md](CHANGELOG.md) for what changed.

## Development

Prerequisites: Node.js 22+, Rust (stable, edition 2024), and the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform (on Linux: `libwebkit2gtk-4.1-dev` and friends).

```sh
npm install
npm run tauri dev      # run the desktop app
npm run tauri build    # package it
```

The repo uses npm (`package-lock.json`), and CI does too. `pnpm install` also
works: `pnpm-workspace.yaml` skips the one dependency build script (core-js)
that newer pnpm versions would otherwise stop on.

Checks:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p stannum-core -p rs-power -p nodal
npm run build          # type-check and bundle the frontend
npm test               # frontend unit tests (Vitest), e.g. the wire router
```

### Browser smoke test

`tests/browser/smoke.mjs` drives the real UI in Chromium against the real Rust
core. `dev_bridge` serves the core's command list (the same one the desktop app
uses) over local HTTP at `POST /invoke/<command>`,
and the test swaps Tauri's IPC for a shim that calls it:

```sh
cargo run --release -p stannum-core --example dev_bridge &
npm run build && npx vite preview --port 1420 &
F=crates/stannum-core/tests/fixtures
STANNUM_MATPOWER=$F/case9.m STANNUM_RAW=$F/five_bus.raw STANNUM_RAW_B=$F/five_bus_modified.raw \
  node tests/browser/smoke.mjs   # needs Playwright; set STANNUM_SHOTS=dir for screenshots
```

### Checking the case-diff port

`cargo run -p stannum-core --example diff_dump -- a.raw b.raw` prints a full
comparison as JSON in the same shape as a small script over the Python
case-diff, which makes it easy to confirm the two agree on new files.

Branching, pull request rules and the repository's security settings are in
[CONTRIBUTING.md](CONTRIBUTING.md). To report a vulnerability, see
[SECURITY.md](SECURITY.md).

## Layout

```
crates/rs-power/          AC/DC power flow, controls, HVDC, sensitivities, outages, MATPOWER and RAW loading
crates/nodal/             RAW v35 parser and topology (islands, bridges, paths)
crates/stannum-core/      Analysis core (no Tauri): workspace, power flow, topology, case diff
  src/commands.rs         Every command the app, the dev bridge and scripts use, listed once
  src/workspace.rs        The shared open case; parses once per engine
  src/run_file.rs         Run files (.stnrun): commands one per line, read and written
  src/power.rs            rs-power solve/sensitivity results shaped for the UI
  src/contingency.rs      N-1 contingency runs: linear screen, AC check, ranking
  src/paging.rs           Sorted, searched and filtered table pages
  src/raw_export/         PSS/E RAW 33/35 export
  src/diff/               case-diff port: tokenizer, exact values, parser, comparison
  schemas/raw.schema.json RAW field layouts (shared with nodal and case-diff)
  tests/                  Integration tests and fixtures
crates/stannum-cli/       stannum-cli: runs run files without the app; MCP server
crates/stannum-py/        The `stannum` Python package (PyO3)
crates/stannum-synth/     Synthetic test cases
src-tauri/                Desktop shell; src/analysis.rs passes commands to the core's list
src/
  shell/                  Module rail, case bar, drag-and-drop
  modules/                diagram/, power/, records/, diff/ (topology/ holds the shared graph and field code)
  lib/analysis/           Typed command wrappers and result types
  ui/                     Shared table and form components
  components/, store/     The diagram editor (from StannumSLD)
tests/browser/            Browser smoke test
docs-site/                Documentation site (Astro + Starlight)
```

rs-power and nodal used to be separate repositories. They now live here as
workspace crates, imported with their full git history, and are developed in
this repository only. Each crate has its own README with its API and checks.

## Licensing

Stannum is BSD-3-Clause; see [LICENSE](LICENSE). [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)
covers the work it includes or adapts: MATPOWER (in rs-power, see also
`crates/rs-power/NOTICE`), the Eclipse Layout Kernel via elkjs (EPL-2.0),
MapLibre GL JS and Natural Earth map data. [THIRD-PARTY-LICENSES.txt](THIRD-PARTY-LICENSES.txt)
has the licence of every Rust crate and npm package built into the app, written by
`pwsh scripts/release/third-party-licenses.ps1`. All three ship with the
installers, the Python wheels and the `stannum-cli` archives. MATPOWER test
cases in the repository keep their own terms; see
`crates/stannum-core/tests/fixtures/README.md`.

## Logo

The mark is an S traced along the edges of a cube, a tin (Sn) crystal cell drawn as
a network. The masters are `src/assets/stannum-logo.svg` (48 px and up) and
`src/assets/stannum-logo-small.svg` (16 to 32 px). After editing either one, run
`pwsh scripts/update-icons.ps1` to regenerate the Tauri icons, the favicon and the
docs-site logo.
