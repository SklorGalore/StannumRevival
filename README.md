# rs-power

A Rust library for balanced, steady-state AC and DC power flow, with literal
MATPOWER version-2 and PSS/E RAW revision 23–33 and 35 case loading. The initial implementation uses a sparse
Newton–Raphson solver and optional generator reactive-limit enforcement.

## Usage

```rust
use rs_power::{load_matpower, solve, PowerFlowOptions, SolveStatus};

let case = load_matpower("tests/fixtures/case9.m")?;
let result = solve(&case, &PowerFlowOptions::default())?;
assert_eq!(result.status, SolveStatus::Converged);
for bus in &result.case.buses {
    println!("{}: {} p.u., {} degrees", bus.id, bus.vm, bus.va);
}
# Ok::<(), rs_power::Error>(())
```

Run the example from this repository:

```sh
cargo run --example power_flow -- tests/fixtures/case9.m ac
cargo run --example power_flow -- tests/fixtures/case300.m dc
```

Set `model: Model::Dc` for DC flow or `enforce_q_limits: true` for AC
reactive-limit enforcement. Cases can also be built or modified directly using
the public `Case`, `Bus`, `Generator`, and `Branch` structures.
Set `initialization: Initialization::Flat` to start AC Newton iterations at
1 p.u. with each island anchored to its supplied reference-bus angle. The
default, `Initialization::Case`, uses the case's supplied voltage iterate.

Export an edited case with `save_matpower("edited_case.m", &case)?`. The file
defines a MATLAB function named after its file stem, ready for MATPOWER's
`loadcase`. Use `format_matpower(&case)?` for a string of literal assignments,
and `parse_matpower(&text)?` to read one. Both export functions preserve row
order, extra columns, costs, and labels; generator statuses are normalized to
0/1 because the Rust representation is boolean. Serialization rejects ragged
tables, NaN, unsupported label fields, and multiline labels before writing.
Comments and original numeric formatting are not retained. Saving overwrites
the destination; its stem must be a valid MATLAB function name.

Load PSS/E `.raw` files with `load_raw("network.raw")?` or `parse_raw(&text)?`.
Revisions 23–33 and 35 are supported, including comma/whitespace delimiters, quoted
names, mixed line endings, and revision detection from the header or its PSS/E
comment. An unidentified revision defaults to 23. `Q` ends the input; it is
optional after a section terminator. Trailing empty sections may be omitted.
Malformed required data still returns a source-located error.

The default conversion follows **MATPOWER 8.1's RAW importer**, including its
approximations. Use the options API to inspect warnings or reject lossy imports:

```rust
use rs_power::{load_raw_with_options, RawOptions};
let imported = load_raw_with_options(
    "tests/fixtures/raw/revision_33.raw",
    &RawOptions { revision: None, strict: false },
)?;
for warning in &imported.warnings {
    eprintln!("line {}: {}", warning.line, warning.message);
}
let case = imported.case;
# Ok::<(), rs_power::Error>(())
```

`parse_raw_with_options` provides the same report for strings. Set `revision`
to override detection, or `strict: true` to reject approximations and ignored
equipment. The convenience APIs return only `Case` and discard warnings;
the example prints them. Warning wording and counts are not MATPOWER-compatible.

RAW conversion sorts buses by ID, retains bus names, aggregates loads and shunts,
normalizes negative metered-end branch IDs, and converts two- and three-winding
transformers (CW/CZ 1–3), adding star buses with MATPOWER's numbering and winding
statuses. Impedance conversion uses the winding nominal voltage base, matching
MATPOWER's `use_winding_baseV=1`, including its zero-NOMV behavior. ZIP loads are
frozen at the supplied bus voltage. Switched shunts use BINIT, including offline
records, as in MATPOWER. Transformer magnetizing admittance, tap controls,
correction tables, remote regulation and wind control are ignored with warnings
when present. Unsupported equipment sections are skipped with warnings, following
MATPOWER; ownership, zone, interchange and grouping metadata are not retained.
Change-code files warn and are treated as standalone cases, never incremental
updates. Unknown branch endpoints are retained with warnings; invalid transformer
endpoints are skipped. `solve` still validates the resulting network.

Two-terminal DC links are converted with MATPOWER's lossless approximation and
reactive-limit estimates, and retained in `Case::dcline` (17-column numeric rows).
Literal MATPOWER import/export preserves this table. **The solver rejects
nonempty DC line tables**; DC transmission support is separate from DC power
flow on an AC network. To solve an exported DC-line case in MATPOWER, enable its
extension with `toggle_dcline(mpc, 'on')`. RAW export is not provided.

Conversion tests compare every numeric bus, generator, branch and DC-line field,
and bus names, against generated MATPOWER 8.1 results. They cover all supported
revision layouts, all transformer CW/CZ combinations, partial winding statuses,
DC control modes, and three upstream RAW fixtures. AC/DC solution comparisons
also run for compatible reference cases. This establishes tested conversion
compatibility, not full PSS/E modeling or parity on every malformed file or
unimplemented revision. Regenerate the synthetic inputs and reference outputs:

```sh
python3 scripts/generate_raw_inputs.py
octave --no-gui --quiet --eval "addpath('scripts'); generate_raw_reference('/path/to/matpower', 'tests/fixtures/raw')"
```

`solve` borrows its input and returns a separate case with solved bus voltages
and generator outputs. `branch_flows` contains `[PF, QF, PT, QT]` for each input
branch. Bus IDs and all original row ordering are preserved. Power is in MW/MVAr,
voltage magnitude is per unit, and public angles are in degrees. Transformers
use the MATPOWER from-side tap convention; a zero tap magnitude means unity.

Defaults are AC, supplied case voltages (with generator voltage setpoints at
controlled buses), a `1e-8` per-unit mismatch tolerance, 10 Newton steps per
reactive-limit pass, and reactive-limit enforcement disabled. Reactive-limit
tolerance is `5e-6` MVAr. The iteration count includes all limit passes.

Always check `SolveStatus`. Reaching the iteration limit returns the final
iterate, computed outputs and residual. Reactive-limit infeasibility is a
separate status. Invalid inputs, unsupported topology, and numerical failures
return `Error`. No diagnostics are printed by the library. The reported residual
is the infinity norm of the equations constrained by the final solve, not a
measure of reactive-limit feasibility.

## Compatibility

Numerical behavior targets the **MATPOWER 8.1 legacy core**, AC power/polar Newton
and DC power flow, with constant-power loads. Supported equipment includes
shunts, line charging, taps, phase shifters, parallel branches, offline
equipment, and multiple generators at one bus. Reactive outputs are allocated
using MATPOWER's range-based sharing, including zero-width and infinite limits.
Reactive-limit enforcement fixes all current violations simultaneously, switches
affected buses to PQ and reassigns the reference bus when necessary.

The loader accepts the following data-only subset without running MATLAB:

- Version `'2'`, literal `baseMVA`, and numeric `bus`, `gen`, `branch` tables.
- Optional numeric `gencost` and `dcline`, preserved without interpretation.
- Literal `bus_name`, `gentype`, and `genfuel` string-cell tables, preserved in
  `Case::labels`.
- Standard `function mpc = name` or `function mpc = name()` wrappers, optional
  final `end`, `%` comments, `...` continuation lines, decimal/scientific
  notation, signed literals, and `Inf`/`-Inf` for reactive limits.

Numeric matrices must be rectangular; bus/gen/branch require at least 13/10/13
columns. Extra columns are retained without interpretation, including any
pre-existing result columns. Read current branch results from `branch_flows`.
Unknown fields, arithmetic expressions, indexed assignments, function calls,
and general MATLAB code are rejected with a source location. `.mat` files and
version-1 cases are not supported. Parsing checks syntax and table shape;
`solve` validates physical inputs and topology.

Each connected active island requires exactly one reference bus with an online
generator. Islands are solved together while retaining independent angle
references and slack-generator allocation. Missing or multiple reference buses
within an island return an explicit error. Type-4 buses and incident equipment
are excluded; their bus data is retained, with zero output for excluded
generators and branches.
Voltage-controlled buses without online generators are treated as PQ. DC flow
requires nonzero active branch reactances and does not accept reactive-limit
enforcement.

OPF, continuation power flow, ZIP loads, alternative AC formulations, radial
methods, three-phase models, extensions, language bindings and a production CLI
are outside this release. Passing these reference tests does not establish
compatibility with every MATPOWER case or numerical condition.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
```

Run the dependency-free optimized timing harness with `cargo bench`. It reports
MATPOWER parsing and AC/DC solve time for case9, case118 and case300, plus RAW
parsing and solving for the 993-bus Memphis revision-35 case. Set
`RS_POWER_BENCH_ITERATIONS` to change the default of 20 measured iterations.

Checked-in fixtures cover case9, case14, case30, case57, case118 and case300,
plus synthetic case9 variants for limits, slack reassignment, shared generators,
infinite/zero-width reactive ranges, taps, phase shifts, shunts, offline
equipment, parallel branches and nonconsecutive IDs. AC/DC comparisons require
agreement within `1e-6` p.u. voltage, `1e-5` degrees and `1e-4` MW/MVAr. Tests
also cover finite-difference Jacobians, nodal balance, singular systems, invalid
inputs and nonconvergence. GitHub Actions runs checks on Linux, macOS and
Windows, and verifies the release package on Linux.

Ordinary tests do not need Octave, MATLAB, or a MATPOWER installation. To
regenerate fixtures, provide a trusted MATPOWER checkout at tag `8.1`, commit
`3f8ecfdbc79b07697d6b45f8d868ac1c2d27f788`, and run:

```sh
octave --no-gui --quiet --eval "addpath('scripts'); generate_reference('/path/to/matpower', 'tests/fixtures')"
```

The generator requires version 8.1, selects `exp.use_legacy_core = 1`, and uses
`1e-10` mismatch tolerance for reference precision. It overwrites generated
fixtures and copies the six original case files with their provenance comments.
The JSON records both inputs and reference results. Octave encodes infinite
reactive limits as JSON `null`; the fixture test reader restores the signed
limits for the dedicated infinity scenario.

## Implementation and licensing

Sparse admittance rows feed a sparse analytical Jacobian; `faer` supplies sparse
LU. Backend types remain private. There is no MATLAB/Octave or native-library
runtime dependency. Network construction, sparse factorization and Newton
iteration are separate internal components for future extensions; v1 does not
cache factorizations across calls.

Software is BSD-3-Clause. See [NOTICE](NOTICE) for MATPOWER attribution and its
retained license. MATPOWER explicitly excludes case data from its software BSD
license; the bundled cases retain their original source and citation notices.
