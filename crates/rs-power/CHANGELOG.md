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

- DC line transformer taps: `FlowControls::dc_line_taps` (`DcLineTap`)
  keeps two-winding transformers with COD 4 that a two-terminal DC line
  converter names (IFR/ITR/IDR, IFI/ITI/IDI). With the detailed DC model's
  `adjust_taps`, the branch tap steps in place of the converter tap, within
  its TMX/TMN and by its STP; COD -4 holds both.
  `PowerFlowResult::dc_line_tap_ratios` gives their final taps. Import no
  longer warns that IF/IT/ID is ignored, and warns instead when a converter
  names a transformer without COD 4 (its own tap moves) or a COD 4
  transformer is named by no converter.
- Reactive flow taps: `FlowControls::reactive_taps` (`ReactiveTap`) keeps
  transformer windings in reactive power flow control (COD 2). They step in
  whole positions with the phase shifters, from the same joint Jacobian
  solve; `lock_taps` locks them, and `PowerFlowResult::reactive_tap_ratios`
  gives their final ratios. COD 2 windings no longer warn on import.
- Phase shifters: `RawImport::flow_controls` (`FlowControls`, `PhaseShifter`,
  `Asymmetric`) keeps transformer windings in active power flow control
  (COD 3 and 5). `PowerFlowOptions::flow_controls` moves their angles between
  Newton solves to hold each MW band, with the moves solved together from
  Jacobian sensitivities; `lock_phase_shifters` keeps them at their angles.
  `PowerFlowResult::phase_shift_angles` and `flow_control_passes` report the
  outcome. COD 3 and 5 windings no longer warn on import.
- Area interchange control: `PowerFlowOptions::interchange`
  (`AreaInterchange` with `InterchangeMode::TieLines` or `TieLinesAndLoads`)
  moves each area's slack generation until its net interchange meets
  `Area::desired_mw` within `tolerance_mw`. `PowerFlowResult::areas` reports
  every area's interchange (`AreaSolution`), also with control off, and
  `interchange_passes` counts the passes. RAW import reads area data into
  `RawImport::areas` and loads assigned to another area into
  `RawImport::area_loads`.
- `PowerFlowOptions::method` (`AcMethod`): fixed-slope decoupled Newton,
  fast decoupled XB and BX, and `Auto` alongside full Newton (the default),
  with `decoupled_iterations` as their iteration limit and
  `PowerFlowResult::decoupled_iterations` and `newton_fallbacks` to report
  them.
- `PowerFlowOptions::lock_switched_shunts` and `lock_taps` hold listed
  voltage controls at their starting values.
- Voltage-dependent (ZIP) loads. `Bus` gains constant-current (`pd_i`,
  `qd_i`) and constant-impedance (`pd_z`, `qd_z`) parts, drawn in
  proportion to |V| and |V|²; AC power flow, its Jacobian and sensitivities
  include them, and DC power flow takes them at 1 p.u. `RawOptions::zip_loads`
  keeps RAW IP/IQ/YP/YQ instead of MATPOWER's constant-power conversion.
  MATPOWER export writes the load drawn at each bus's voltage. Checked
  against pandapower on case30 with mixed loads.
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

### Fixed

- A generator bus holding a remote bus that a reactive-limit pass fixes at
  its limit restarts from the held bus's voltage. Its own voltage was free
  while it regulated and could be left near 0 pu, nearly in antiphase,
  after a solve that asked it for far more than its limit; Newton from
  there found the collapsed root of the fixed injection.

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
