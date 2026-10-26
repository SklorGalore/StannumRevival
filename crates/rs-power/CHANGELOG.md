# Changelog

All notable changes to this project will be documented here. This project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Changed

- Moved into the Stannum repository as the `crates/rs-power` workspace crate.
  The optional egui desktop GUI (`rs-power-gui`, the `gui` feature and
  `GUI_PLAN.md`) was removed; Stannum's Power flow module replaces it.
- Removed the interactive `rs-power` command line (REPL), its private
  presentation helpers and CLI tests; rs-power is now a library only. The
  entries below describe the GUI and CLI as they were before.

### Added

- Meshed VSC DC grids in MatACDC's model: `VscGrids` (read with
  `load_matacdc` / `parse_matacdc`) attach to an AC case through
  `DcModel::Detailed { vsc_grids, .. }`, with slack, droop (with deadband)
  and power-controlled converters, several DC grids, and station losses.
  Results are in `PowerFlowResult::vsc_grids`. MATPOWER version 1 cases
  (`function [baseMVA, bus, gen, branch] = name`) now load too, and case
  files that are not UTF-8 load with replacement characters.

- Sensitivities follow remote regulation: with `enforce_q_limits`, a held
  bus keeps its magnitude and the generator buses holding it move, as in the
  solve, and reactive sharing between them is kept.

- Faster reactive-limit solves: while generators may still switch between
  PV and PQ, each solve stops at a 0.01 p.u. mismatch, and only clear
  violations switch a generator. The final solve still meets the full
  tolerance. SyntheticUSA drops from 76 to 45 Newton iterations (31 s to
  18 s).

- Switched shunt and transformer tap voltage control:
  `RawImport::voltage_controls` lists switched shunts (MODSW 1 and 2) and
  transformer windings (COD 1) in voltage control, and
  `PowerFlowOptions::voltage_controls` applies them between Newton solves.
  Results report the final values in `PowerFlowResult::switched_shunts` and
  `tap_ratios`. Such records no longer warn on import.

- Remote voltage regulation: `RawOptions::remote_regulation` keeps PSS/E IREG
  and RMPCT in the new `Generator::regulated_bus` and
  `Generator::reactive_share`. With reactive limits enforced, the regulated
  bus is held at the setpoint, plant groups share reactive power by RMPCT,
  generators starting at a limit start there, and a generator released at a
  limit returns to voltage control when the bus voltage crosses back. This
  makes the SyntheticUSA Q-limit solve converge.
- Newton steps are shortened when they would change a voltage magnitude by
  more than half.
- The reactive-limit loop's island checks are linear in case size (they were
  quadratic, which cost several seconds per pass at 80,000 buses).

- Optional `rs-power-gui` desktop application for loading MATPOWER and RAW
  cases, configuring and running background AC/DC solves, reviewing convergence
  and operating-limit checks, browsing searchable result tables, calculating a
  bus-focused sensitivity view, and exporting MATPOWER results.
- GUI design and scope document in `GUI_PLAN.md`.
- AC and DC state and branch-flow sensitivity calculation, including full DC
  PTDF and LODF matrices, selected bus and explicit transfer columns,
  distributed and per-column slack, topology-based bridge masking, full and
  reduced AC Jacobians, and polar/Cartesian bus, branch power, current, and
  squared-magnitude derivatives.
- Interactive CLI commands for labeled AC/DC sensitivities, Jacobians, selected
  or full PTDF matrices, and masked or raw LODF matrices.
- Sensitivity-run progress logging with model, case, solver, balancing, timing,
  convergence, matrix dimensions, and explicit failure reporting.
- Selected-bus CLI sensitivity reports with active/reactive injection selection,
  engineering or raw units, signed rankings per quantity, and bounded or full
  text output; existing unselected reports retain their format.
- Diagnostic logging for MATPOWER and RAW I/O, conversion warnings, network
  assembly, AC Newton iterations, DC solves, reactive-limit passes, sensitivity
  matrices, CLI commands, and GUI startup. Interactive REPL sessions log at
  `info` automatically; `RUST_LOG` controls verbosity and non-interactive logs.
- UTF-8 text report output through a `to <path>` suffix on summary, result-table,
  sensitivity, Jacobian, PTDF, LODF, and option commands.
- CSV export selected by a `.csv` destination for 2D Jacobian, PTDF, and LODF
  matrices and for summary, bus, generator, flow, and option tables.

### Changed

- GUI sensitivities now provide searchable injection selection and complete,
  virtualized angle, magnitude, and branch-flow tables with filtering, sorting,
  unit selection, operating-point context, and sign explanations.
- AC branch-flow sensitivity assembly now exploits the endpoint sparsity of
  branch derivatives instead of multiplying dense zero-filled rows.
- RAW section parsing now follows the revision-defined section order and `0`
  terminators without interpreting human-readable `@!` section headers.

## 0.1.0 - 2026-09-14

Initial release.

### Added

- Sparse Newton–Raphson AC and linear DC power flow.
- Multiple active-island solving with independent reference buses.
- Optional generator reactive-limit enforcement and reference reassignment.
- Case-supplied and flat-start voltage initialization.
- Literal MATPOWER version-2 parsing, formatting, and file I/O.
- PSS/E RAW revision 23–33 and 35 import with strict and warning modes.
- MATPOWER two-terminal DC line support in AC and DC power flow.
- MATPOWER 8.1 numerical and conversion reference tests.
- Optimized parsing and solver benchmark harness.

### Known limitations

- RAW export, OPF, continuation power flow, ZIP iteration, and three-phase
  models are not implemented.
- Each active island must provide exactly one reference bus with an online
  generator.
