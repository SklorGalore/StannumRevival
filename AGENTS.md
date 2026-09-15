# Repository Guidelines

## Project Structure & Module Organization

`rs-power` is a Rust 2024 library and interactive CLI for AC/DC power flow.
`src/lib.rs` exposes the API; `src/main.rs` implements the CLI. Case types live
in `src/case.rs`, MATPOWER import/export in `src/input.rs` and `src/output.rs`,
RAW conversion in `src/raw.rs`, network assembly in `src/network.rs`, and solving
in `src/solver.rs`.

Integration tests live in `tests/`, with checked-in inputs and reference JSON
under `tests/fixtures/`. `_cases/` contains the Memphis RAW case. See `examples/`
for usage, `benches/` for timings, and `scripts/` for fixture generators.

## Build, Test, and Development Commands

Run from the repository root with a Rust toolchain supporting edition 2024:

- `cargo build --locked`: build the library and CLI.
- `cargo run -- tests/fixtures/case9.m`: start an interactive session.
- `cargo run --example power_flow -- tests/fixtures/case9.m ac`: run the example.
- `cargo fmt --all -- --check`: check formatting; use `cargo fmt --all` to apply it.
- `cargo clippy --locked --all-targets -- -D warnings`: lint all targets.
- `cargo test --locked --all-targets`: run tests and target checks.
- `cargo test --locked --doc`: verify README/API examples.
- `RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps`: validate documentation.
- `cargo bench --bench power_flow`: run the timing harness.

## Coding Style & Naming Conventions

Follow rustfmt defaults, including four-space indentation. Use `snake_case` for
modules, functions, and tests, and `UpperCamelCase` for types. Keep backend types
private. Document public fields with units, sign conventions, and whether they
are used or preserved. The README doubles as crate documentation.

## Testing Guidelines

Use Rust's built-in test framework and descriptive behavior-based test names.
Add focused regressions to the relevant suite for solver, importer, or CLI changes.
Reference comparisons allow `1e-6` p.u., `1e-5` degrees, and `1e-4` MW/MVAr;
no percentage coverage threshold is specified. Ordinary tests require no MATPOWER
installation. Regenerate fixtures only when required by the task, using the pinned
MATPOWER version in `CONTRIBUTING.md`. Review inputs and outputs together, explain
expected-result changes, and preserve provenance and licensing notices.

## Numerical Correctness

Preserve MATPOWER compatibility, units, sign conventions, and input row ordering.
Never loosen tolerances or change reference outputs merely to pass failing tests.
Distinguish nonconvergence from invalid input and numerical errors.

## Scope & API Changes

Keep changes focused on the requested behavior and preserve unrelated working-tree
changes. Preserve public API compatibility unless a breaking change is explicitly
requested. Add dependencies only when existing code cannot reasonably support the
change.

## Verification & Handoff

Run relevant regression tests during development and the documented checks before
completion. Report what changed, which checks passed, and which could not run.

## Commit & Pull Request Guidelines

History mixes imperative summaries with `feat:` and `docs:` prefixes; keep
messages concise and specific. PRs should explain the problem, resulting behavior,
and validation, and link relevant issues. Update `README.md` and `CHANGELOG.md`
for user-visible changes.
