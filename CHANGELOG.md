# Changelog

All notable changes to this project will be documented here. This project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- AC and DC state and branch-flow sensitivity calculation, including full DC
  PTDF and LODF matrices, selected bus and explicit transfer columns,
  distributed and per-column slack, topology-based bridge masking, full and
  reduced AC Jacobians, and polar/Cartesian bus, branch power, current, and
  squared-magnitude derivatives.

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
