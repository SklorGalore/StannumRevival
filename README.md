# rs-power

A Rust library for balanced, steady-state AC and DC power flow, with literal
MATPOWER version-2 case loading. The initial implementation uses a sparse
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
- Optional numeric `gencost`, preserved without interpretation.
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

V1 requires one connected active network and exactly one reference bus with an
online generator. Multiple active islands and multiple reference buses return
explicit errors. Type-4 buses and incident equipment are excluded; their bus
data is retained, with zero output for excluded generators and branches.
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

Checked-in fixtures cover case9, case14, case30, case57, case118 and case300,
plus synthetic case9 variants for limits, slack reassignment, shared generators,
infinite/zero-width reactive ranges, taps, phase shifts, shunts, offline
equipment, parallel branches and nonconsecutive IDs. AC/DC comparisons require
agreement within `1e-6` p.u. voltage, `1e-5` degrees and `1e-4` MW/MVAr. Tests
also cover finite-difference Jacobians, nodal balance, singular systems, invalid
inputs and nonconvergence. GitHub Actions runs checks on Linux and macOS.

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
