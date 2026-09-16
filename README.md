# rs-power

A Rust library for balanced, steady-state AC and DC power flow, with literal
MATPOWER version-2 and PSS/E RAW revision 23–33 and 35 case loading. AC flow uses
a sparse Newton–Raphson solver with optional generator reactive-limit enforcement.

[Usage](#usage) · [Import and export](#import-and-export) ·
[Solver results](#solver-results) ·
[Sensitivity calculation](#sensitivity-calculation) · [Compatibility](#compatibility) ·
[Verification](#verification)

## Usage

Use a Rust toolchain that supports edition 2024. Add this repository as a Git
dependency in your project’s `Cargo.toml`:

```toml
[dependencies]
rs-power = { git = "https://github.com/SklorGalore/rs-power" }
```

The paths below refer to fixtures in a local checkout; replace them with your
case path when using the library in another project.

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
cargo run --example power_flow -- tests/fixtures/raw/revision_33.raw ac
```

Set `model: Model::Dc` for DC flow or `enforce_q_limits: true` for AC
reactive-limit enforcement. Cases can also be built or modified directly using
the public `Case`, `Bus`, `Generator`, and `Branch` structures.
Set `initialization: Initialization::Flat` to start AC Newton iterations at
1 p.u. with each island anchored to its supplied reference-bus angle. The
default, `Initialization::Case`, uses the case's supplied voltage iterate.

## Interactive CLI

Start a session, optionally loading a case at startup:

```sh
cargo run -- tests/fixtures/case9.m
```

```text
rs-power> solve ac
rs-power> buses
rs-power> generators
rs-power> flows
rs-power> sensitivities ac
rs-power> ptdf 4 5 6
rs-power> lodf
rs-power> set enforce_q_limits on
rs-power> solve
rs-power> save solved_case.m
rs-power> quit
```

Use `help` for all commands and `options` for current solver settings.
The CLI defaults to residuals in MVA (per-unit mismatch multiplied by the case
base MVA). Use `set residual pu` or `set residual mva` to change solve and summary
displays immediately; `options` shows the current choice. The example reports
MVA; solver tolerance and the library residual remain per unit. `load`
accepts MATPOWER and RAW files, including paths with spaces and optional matching
quotes. RAW conversion warnings are displayed. By default, `update_on_solve: true` replaces the working case with each solve's
result, including a nonconverged iterate, so subsequent solves start from it.
Use `set update_on_solve false` to keep the current working case unchanged during
future solves; `options` shows this setting. Solver errors leave the working case
unchanged. Inspection and saving use the latest result. Loading another case clears results; a failed load preserves the session.
Saving overwrites the destination in MATPOWER format.

The REPL also exposes the library's principal sensitivity matrices. Use
`sensitivities [ac|dc]` to solve the working case and print labeled state and
branch-flow sensitivities; like `solve`, it updates the latest result and honors
`update_on_solve`. Before computing, the command logs the selected model, case
size, solver settings, balancing convention, and state variables, and flushes
that progress message immediately. It then reports elapsed time, convergence,
and result dimensions before printing the matrices. Debug builds also note that
AC linear algebra is substantially faster under `cargo run --release`.
`jacobian [reduced|full]` evaluates the AC Jacobian at the latest case voltages.
`ptdf` prints the full branch-by-bus DC matrix, while optional external bus IDs
select and order columns (for example, `ptdf 4 5 6`).
`lodf [mask|raw]` prints the branch-by-branch outage matrix; the default `mask`
mode reports undefined bridge-outage columns as `NaN`. Branches use one-based
input row numbers in all CLI matrix output.

Run `cargo install --path .` to install the `rs-power` command. Commands can also
be piped into it; prompts are omitted and any command failure produces a nonzero
exit status after processing the remaining commands. End-of-input exits the REPL.

## Import and export

### MATPOWER

Export an edited case with `save_matpower("edited_case.m", &case)?`. The file
defines a MATLAB function named after its file stem, ready for MATPOWER's
`loadcase`. Use `format_matpower(&case)?` for a string of literal assignments,
and `parse_matpower(&text)?` to read one. Both export functions preserve row
order, extra columns, costs, and labels; generator statuses are normalized to
0/1 because the Rust representation is boolean. Serialization rejects ragged
tables, NaN, unsupported label fields, and multiline labels before writing.
Comments and original numeric formatting are not retained. Saving overwrites
the destination; its stem must be a valid MATLAB function name.

### PSS/E RAW

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
Literal MATPOWER import/export preserves this table. Active links are solved as
paired terminal injections, matching MATPOWER's `toggle_dcline` power-flow
representation; solved PF/PT/QF/QT and terminal voltages are written back to the
table. RAW export is not provided.

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

## Solver results

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

## Sensitivity calculation

`calculate_sensitivities` solves a case and linearizes the selected AC or DC
model at the converged operating point. It returns the inverse reduced
power-flow Jacobian and the resulting derivatives of all four branch-flow
components:

```rust
use rs_power::{
    InjectionVariable, PowerFlowOptions, StateVariable, calculate_sensitivities,
    load_matpower,
};

let case = load_matpower("tests/fixtures/case9.m")?;
let result = calculate_sensitivities(&case, &PowerFlowOptions::default())?;
assert!(matches!(result.injection_variables[0], InjectionVariable::ActivePower(_)));
assert!(matches!(result.state_variables[0], StateVariable::VoltageAngle(_)));
println!("d(state 0)/d(injection 0) = {}", result.state_sensitivities[0][0]);
# Ok::<(), rs_power::Error>(())
```

State rows and injection columns identify buses by their external IDs. Angles
are in radians; voltage magnitudes and injections are in per unit. Divide MW or
MVAr perturbations by `base_mva` before applying the state matrix. Branch-flow
derivatives are in p.u./p.u., which is numerically MW/MW or MVAr/MVAr on the
common system base. Branch-flow assembly exploits the fact that each branch
depends only on its endpoint states, avoiding dense zero work on large AC
cases. In DC mode, active branch-flow derivatives are PTDFs.

For a full branch-by-bus DC PTDF matrix without first solving a power flow, use
`make_ptdf`. Passing `None` uses each island's reference bus as slack; a slice
of bus weights selects distributed slack. `make_lodf` converts that matrix to
branch-by-branch line-outage factors:

```rust
use rs_power::{load_matpower, make_lodf, make_ptdf};

let case = load_matpower("tests/fixtures/case9.m")?;
let ptdf = make_ptdf(&case, None)?;
let lodf = make_lodf(&case, &ptdf)?;
assert_eq!(ptdf.len(), case.branches.len());
assert_eq!(lodf.len(), case.branches.len());
# Ok::<(), rs_power::Error>(())
```

Both matrices retain input row order; PTDF columns retain input bus order.
LODF columns for islanding outages contain `NaN` because no connected-network
outage factor exists for them. `make_ptdf_columns` computes requested external
bus-ID columns, `make_ptdf_transfers` accepts explicit zero-sum transfer
columns, and `make_ptdf_with_slack_matrix` selects a different slack
distribution for every bus-injection column. These correspond to MATPOWER's
`BUS_IDX`, `TXFR`, and matrix-valued `SLACK` forms. Transfer and slack matrices
are indexed `[bus][column]` in input bus order. Selected columns are solved
directly without constructing the full PTDF. `make_lodf_with_bridge_handling`
can preserve MATPOWER's raw infinite/`NaN` bridge results when needed.

`make_jacobian` constructs the reduced Newton or full AC Jacobian directly.
`bus_power_derivatives` and `branch_flow_derivatives` expose the complex
first-order derivatives corresponding to MATPOWER's `dSbus_dV`, `dSbr_dV`, and
`dIbr_dV`, in polar or Cartesian voltage coordinates. Branch results also
provide squared power- and current-magnitude derivatives corresponding to
`dAbr_dV`. Active bus IDs accompany matrix rows and columns; branch rows retain
input order, including zero rows for excluded branches.

Reference-bus angle and injection columns, PV-bus voltage-magnitude and reactive
injection columns, and isolated buses are omitted from the reduced matrices. A
non-reference injection is balanced by the reference bus in its electrical
island. Offline and isolated branches retain their input positions with zero
derivatives. The calculation supports multiple islands and uses bus types after
reactive-limit enforcement. A nonconverged operating point or singular reduced
Jacobian returns `Error::Numerical`.

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
- Optional numeric `gencost`, preserved without interpretation, and `dcline`,
  preserved for import/export and used as terminal injections when solving.
- Literal `bus_name`, `gentype`, and `genfuel` string-cell tables, preserved in
  `Case::labels`.
- Standard `function mpc = name` or `function mpc = name()` wrappers, optional
  final `end`, `%` comments, `...` continuation lines, decimal/scientific
  notation, signed literals, and `Inf`/`-Inf` for reactive limits.

Numeric matrices must be rectangular; bus/gen/branch require at least 13/10/13
columns. Extra columns are retained without interpretation, including any
pre-existing result columns. Read current branch results from `branch_flows`. Positive flow enters the branch
at the corresponding end. Voltage, branch rating, angle-difference, and generator
active-power limits are retained but not enforced; reactive limits are enforced
only when requested.
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
methods, three-phase models, extensions and language bindings
are outside this release. Passing these reference tests does not establish
compatibility with every MATPOWER case or numerical condition.

## Verification

See the [contributor guide](https://github.com/SklorGalore/rs-power/blob/main/CONTRIBUTING.md)
for the repository layout and fixture workflow. Run these checks from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps
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
iteration are separate internal components for future extensions; the current implementation does not
cache factorizations across calls.

Software is BSD-3-Clause. See [NOTICE](https://github.com/SklorGalore/rs-power/blob/main/NOTICE) for MATPOWER attribution and its
retained license. MATPOWER explicitly excludes case data from its software BSD
license; the bundled cases retain their original source and citation notices.
