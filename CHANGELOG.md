# Changelog

All notable changes to this project will be documented here. This project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.0 - 2026-09-14

Initial release.

### Added

- Sparse Newton–Raphson AC and linear DC power flow.
- Multiple active-island solving with independent reference buses.
- Optional generator reactive-limit enforcement and reference reassignment.
- Case-supplied and flat-start voltage initialization.
- Literal MATPOWER version-2 parsing, formatting, and file I/O.
- PSS/E RAW revision 23–33 and 35 import with strict and warning modes.
- MATPOWER 8.1 numerical and conversion reference tests.
- Optimized parsing and solver benchmark harness.

### Known limitations

- DC transmission-line tables are preserved but cannot be solved.
- RAW export, OPF, continuation power flow, ZIP iteration, and three-phase
  models are not implemented.
- Each active island must provide exactly one reference bus with an online
  generator.
