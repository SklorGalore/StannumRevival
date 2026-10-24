# Contributing

Use a Rust toolchain with edition 2024 support and run commands from the repository
root. Ordinary builds and tests need no MATLAB, Octave, or MATPOWER installation.

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/case.rs` | Public case structures and units |
| `src/input.rs`, `src/output.rs` | Literal MATPOWER parsing and serialization |
| `src/raw.rs` | PSS/E RAW parsing, conversion, and warnings |
| `src/network.rs` | Validation, active islands, and sparse admittance assembly |
| `src/solver.rs` | AC/DC equations, result allocation, and reactive limits |
| `examples/power_flow.rs` | Small file-loading and solving example |
| `tests/` | Behavior, format, and MATPOWER reference comparisons |
| `tests/fixtures/` | Checked-in case data and generated reference results |
| `_cases/` | Memphis revision-35 RAW case used by tests and benchmarks |
| `scripts/` | Synthetic RAW inputs and MATPOWER reference generators |
| `benches/power_flow.rs` | Dependency-free timing harness |

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps
```

The README is also the crate-level documentation, so keep its Rust examples
valid as doctests. Document public fields with their units, sign conventions,
and whether the solver uses or merely preserves them. Generated API docs are in
`target/doc/rs_power/index.html`.

CI runs formatting, Clippy, tests, and doctests on Linux, macOS, and Windows;
Linux also runs `cargo package --locked`. The strict documentation command above
is an additional local check.

For timings, run `cargo bench --bench power_flow`. Set
`RS_POWER_BENCH_ITERATIONS` to a positive integer to override 20 measured
iterations. Each operation has one warm-up; reported load timings include file
I/O. Timings depend on the machine and are not correctness thresholds.

## Reference fixtures

Use a trusted MATPOWER 8.1 checkout at commit
`3f8ecfdbc79b07697d6b45f8d868ac1c2d27f788`. The generators execute upstream
MATLAB/Octave code and overwrite reference outputs:

```sh
octave --no-gui --quiet --eval "addpath('scripts'); generate_reference('/path/to/matpower', 'tests/fixtures')"
python3 scripts/generate_raw_inputs.py
octave --no-gui --quiet --eval "addpath('scripts'); generate_raw_reference('/path/to/matpower', 'tests/fixtures/raw')"
```

Review changed inputs and expected results together, then run the checks above.
Keep upstream provenance and citation comments intact. See the
[RAW fixture notes](tests/fixtures/raw/README.md) for conversion coverage and
[NOTICE](NOTICE) for attribution and case-data licensing.

When changing solver or importer behavior, add a focused regression case to the
relevant existing test suite. Update the README and changelog for user-visible
behavior changes.
