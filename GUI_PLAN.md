# rs-power GUI plan

## Goal

Provide a functional, cross-platform desktop interface for loading, solving,
inspecting, and exporting power-flow cases. The GUI is an optional companion to
the existing library and CLI, not a replacement for either.

The first release prioritizes a reliable analysis workflow and dense, readable
data over decorative presentation.

## Primary workflow

1. Open or drag in a MATPOWER or PSS/E RAW case.
2. Review the case dimensions and any RAW conversion warnings.
3. Select AC or DC and configure solver options.
4. Run the solve without blocking the interface.
5. Review convergence, operating-limit checks, and result tables.
6. Save the latest input or solved case in MATPOWER format.
7. Run sensitivity analysis explicitly when it is needed.

## Layout mockup

```text
+------------------------------------------------------------------------------+
| rs-power   case9.m                                      Open...   Save result |
+----------------------+-------------------------------------------------------+
| CASE                 | Overview                                      AC      |
| case9.m              |                                                       |
| 9 buses              | +-------------+ +-------------+ +-----------------+   |
| 3 generators         | | CONVERGED   | | 4 iterations| | 2.31e-10 MVA    |   |
| 9 branches           | | AC solve    | | 0 Q passes  | | final residual  |   |
| 100 MVA base         | +-------------+ +-------------+ +-----------------+   |
|                      |                                                       |
| ANALYSIS             | Operating limits                                     |
| * Overview           | +------------------+-------+------------+----------+  |
|   Buses              | | Check            | Count | Worst      | Location |  |
|   Generators         | +------------------+-------+------------+----------+  |
|   Branches           | | Voltage limits   |     0 | -          | -        |  |
|   Sensitivities      | | Branch rating A  |     0 | 72.4%      | 8 -> 9   |  |
|                      | | Generator Q       |     0 | -          | -        |  |
| SOLVE SETTINGS       | +------------------+-------+------------+----------+  |
| Model       [ AC  v] |                                                       |
| Start       [Case v] | Recent messages                                       |
| Tolerance   [1e-8  ] | Loaded MATPOWER case9.m                               |
| Iterations  [10    ] | AC power flow converged                               |
| [ ] Enforce Q limits |                                                       |
|                      |                                                       |
| [       Solve       ]|                                                       |
+----------------------+-------------------------------------------------------+
| Ready - solved result - residual displayed in MVA                  Warnings 0 |
+------------------------------------------------------------------------------+
```

The central area switches between the overview and searchable, sortable bus,
generator, and branch tables. A separate sensitivity workspace shows dimensions
and a bounded matrix preview only after the user requests calculation.

## Interaction and visual rules

- Use the system light or dark theme and a restrained teal accent.
- Keep numeric columns right-aligned with tabular figures and units in headings.
- Pair semantic colors with icons or text; color is never the only status cue.
- Make `Solve` the single dominant action.
- Keep advanced settings subordinate to the normal AC/DC workflow.
- Retain and clearly label nonconverged final iterates for inspection.
- Show RAW warnings with source line numbers in a dedicated window.
- Disable options that do not apply to the selected model.
- Run solves and sensitivity calculations on a worker thread. The solver does
  not currently support cooperative cancellation, so the UI does not offer a
  misleading cancel control.

## Architecture

The GUI uses `eframe`/`egui` and is gated behind the Cargo feature `gui`.
Ordinary library and CLI builds therefore do not compile graphical dependencies.

```text
src/bin/rs-power-gui/
|-- main.rs       native window setup
|-- app.rs        application state and navigation
|-- worker.rs     background solve and sensitivity jobs
|-- overview.rs   limit checks and overview presentation
|-- tables.rs     filterable result tables
`-- theme.rs      shared visual styling
```

The binary consumes only the public `rs_power` API. This keeps GUI state and
presentation concerns out of the numerical library.

## Initial scope

- Native open/save dialogs and file drag-and-drop.
- MATPOWER and RAW loading, including conversion warnings.
- AC/DC options and asynchronous solving.
- Convergence diagnostics in MVA or per unit.
- Voltage, generator reactive-power, and branch rating-A checks.
- Searchable, sortable bus, generator, and branch-flow tables.
- MATPOWER export of the loaded or latest solved case.
- On-demand sensitivity calculation with dimensions and a bounded preview.

## Deferred work

- A one-line network diagram, because imported cases do not include dependable
  display coordinates.
- Direct case editing, which needs validation, undo, and dirty-state semantics.
- Full matrix heatmaps and exports for large PTDF, LODF, and Jacobian results.
- Project files, recent-file persistence, and side-by-side case comparisons.
