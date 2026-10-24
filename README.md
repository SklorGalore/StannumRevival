# Stannum

A desktop workbench for power system engineers, built with Tauri 2, Rust and
React. Stannum brings four tools into one app that shares a single open case:

| Module | What it does | Engine |
|---|---|---|
| **Diagram** | Generate one-lines from the open case with power flow results on them, or draw, edit and export your own | React Flow + [ELK](https://eclipse.dev/elk/) |
| **Power flow** | AC/DC power flow, operating-limit checks, result tables and sensitivities | [rs-power](https://github.com/SklorGalore/rs-power) |
| **Topology** | Islands, bridges, articulation buses, shortest paths and a network graph | [nodal](https://github.com/SklorGalore/nodal) |
| **Case diff** | Exact record-by-record comparison of two RAW cases | Rust port of [case-diff](https://github.com/SklorGalore/case-diff) |

All analysis runs locally in native Rust. Case files never leave your machine.

![Topology module: metrics, network graph with a traced path, inspector and bus table](docs/screenshots/topology.png)

| Power flow | Case diff |
|---|---|
| ![Power flow overview](docs/screenshots/power-overview.png) | ![Case diff with a changed bus record](docs/screenshots/case-diff.png) |

![Diagram module: a one-line generated from a five-bus RAW case with power flow results](docs/screenshots/diagram.png)

## Using it

Open a case with **Open case…** or drop a `.raw` / `.m` file on the window. The
case bar shows what loaded, and each module uses the case if it can:

- **MATPOWER** (`.m`, version 2) works in Power flow.
- **PSS/E RAW** revisions 23–35 work in Power flow; revision 35 also works in
  Topology; revisions 33 and 35 work in Case diff.

If a module can't use the open case it says why, and the other modules keep
working.

### Power flow

Choose AC or DC, the starting voltages, tolerance, Newton iterations and
whether to enforce generator reactive limits, then **Solve**. The overview shows
convergence, residual (MVA and p.u.), totals and operating-limit checks
(voltage limits, generator Q limits, branch loading against rate A). The bus,
generator and branch tables are searchable, sortable and export to CSV; rows
outside limits are highlighted. A nonconverged result is kept and labelled so
you can inspect the final iterate.

**Calculate sensitivities** linearizes the solved case. Pick an injection bus
and P or Q, then a response (voltage angle, voltage magnitude or one of the four
branch-end flows), in engineering or raw units, ordered by magnitude, signed
value or input order. "How to read these results" explains the sign conventions.

**Export MATPOWER** writes the input case or the latest solved case.

### Topology

Shows RAW bus, line, transformer, island, bridge and articulation-bus counts, a
Cytoscape network graph and an inspector. Start from a bus's 2-hop
neighborhood, widen it, show a whole island, or show the full network (up to
500 nodes). **Path** finds the fewest equipment hops between two buses.
Tables cover buses, islands, critical connections, equipment and parser
diagnostics; all export to CSV, and **Export JSON** saves the parsed case,
analysis and current path. **Merge zero-impedance branches** treats
zero-impedance lines as a single node.

### Case diff

Choose a baseline and a comparison RAW file (the open case is offered as the
baseline) and **Compare files**. Sections list added, removed, changed and
unchanged counts; select a record to see its fields side by side with the
original lines. Numbers compare exactly (`1.0` equals `1.000`, any other change
counts), string padding is shown in quotes, and formatting, comments and line
endings are ignored. Content in layouts that are not verified is listed as
unsupported and the result is marked incomplete rather than guessed at.

### Diagram

**From case** draws a one-line of the open case (MATPOWER or RAW): the whole
case, one area, or the buses within a few hops of a chosen bus. Buses,
generators, loads, lines and transformers are laid out top to bottom with
right-angle wiring (ELK layered layout, run in a worker). Branches that leave
the selection end in off-page markers naming the far bus. Three-winding
transformers appear as their star point with a two-winding symbol per winding.

Generated symbols stay linked to case records. After a solve (in Power flow, or
**Solve** on the diagram toolbar) the diagram shows bus voltage and angle,
generator and load MW/Mvar, and branch flows with direction arrows and loading;
branches at 80% or more turn amber, overloads red. **Results** toggles them.
A saved diagram keeps the link and shows results again when the same case is open.

The editor also draws by hand: drag symbols from the palette, then drag from a
terminal to wire it. Releasing a wire anywhere along a bus bar adds a tap
there. Symbols sit on a 20 px grid with one stroke weight, so wires line up.
Breakers and disconnectors open and close on double-click. A selected bus has a
grip on its right end for changing its length.

| Action | How |
| --- | --- |
| Select several | Drag a box on empty canvas, or Shift/Ctrl-click |
| Pan | Middle or right drag, or hold Space |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) |
| Copy, cut, paste, duplicate | Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+D |
| Select all / clear | Ctrl+A / Esc |
| Nudge | Arrow keys (one grid step; Shift for 1 px) |
| Align and space evenly | Bar over the canvas, shown with two or more symbols selected |

**Print style** draws everything in black for printing and export. Diagrams
save as `.sld` and export to SVG, PNG or PDF with a title block. Keyboard
shortcuts act on the diagram only while it is showing.

Not yet: shunts, switched devices and HVDC from the case, rotating symbols,
movable labels, and selection shared with the other modules.

## Development

Prerequisites: Node.js 22+, Rust (stable, edition 2024), and the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform (on Linux: `libwebkit2gtk-4.1-dev` and friends).

```sh
npm install
npm run tauri dev      # run the desktop app
npm run tauri build    # package it
```

Checks:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p stannum-core
npm run build          # type-check and bundle the frontend
```

### Browser smoke test

`tests/browser/smoke.mjs` drives the real UI in Chromium against the real Rust
core. `dev_bridge` serves the same commands as the desktop app over local HTTP,
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

## Layout

```
crates/stannum-core/      Analysis core (no Tauri): workspace, power flow, topology, case diff
  src/workspace.rs        The shared open case; parses once per engine
  src/power.rs            rs-power solve/sensitivity results shaped for the UI
  src/diff/               case-diff port: tokenizer, exact values, parser, comparison
  schemas/raw.schema.json RAW field layouts (shared with nodal and case-diff)
  tests/                  Integration tests and fixtures
src-tauri/                Desktop shell; src/analysis.rs exposes the core as commands
src/
  shell/                  Module rail, case bar, drag-and-drop
  modules/                diagram/, power/, topology/, diff/
  lib/analysis/           Typed command wrappers and result types
  ui/                     Shared table and form components
  components/, store/     The diagram editor (from StannumSLD)
tests/browser/            Browser smoke test
```

rs-power and nodal are pinned git dependencies in
`crates/stannum-core/Cargo.toml`; bump the `rev` to pick up upstream changes.

## Licensing

rs-power is BSD-3-Clause with MATPOWER attribution (see its NOTICE). The
`case9.m` test fixture is MATPOWER case data; see
`crates/stannum-core/tests/fixtures/README.md`.
