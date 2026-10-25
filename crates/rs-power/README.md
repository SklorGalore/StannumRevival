# rs-power

A Rust library for balanced, steady-state AC and DC power flow, with literal
MATPOWER version-2 and PSS/E RAW revision 23–35 case loading. AC flow uses
a sparse Newton–Raphson solver with optional generator reactive-limit enforcement.

[Usage](#usage) · [Import and export](#import-and-export) ·
[Solver results](#solver-results) ·
[Sensitivity calculation](#sensitivity-calculation) · [Compatibility](#compatibility) ·
[Verification](#verification)

## Usage

rs-power is a crate in the Stannum workspace. Other workspace crates depend on
it by path:

```toml
[dependencies]
rs-power = { path = "../rs-power" }
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

## Logging

rs-power emits diagnostic events through the `log` facade; install any logger to
see them. `info` reports file operations and solve/sensitivity lifecycles;
`debug` adds network dimensions, solver options, Newton residuals, and RAW
conversion details; and `trace` adds lexer and sparse-linear-system activity.

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
Revisions 23–35 are supported, including comma/whitespace delimiters, quoted
names, mixed line endings, and revision detection from the header or its PSS/E
comment. An unidentified revision defaults to 23. `Q` ends the input; it is
optional after a section terminator. Trailing empty sections may be omitted.
Sections are assigned in the revision's defined order using their `0`
terminators; human-readable `@!` comments and section headers are ignored.
Malformed required data still returns a source-located error.

The default conversion follows **MATPOWER 8.1's RAW importer**, including its
approximations. Use the options API to inspect warnings or reject lossy imports:

```rust
use rs_power::{load_raw_with_options, RawOptions};
let imported = load_raw_with_options(
    "tests/fixtures/raw/revision_33.raw",
    &RawOptions { strict: false, ..Default::default() },
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
records, as in MATPOWER; those in voltage control and transformer windings in
voltage control are also listed in `RawImport::voltage_controls` (see below).
Transformer magnetizing admittance, other tap controls, correction tables and
wind control are ignored with warnings when present, and so is remote
regulation unless `remote_regulation` is set. Unsupported equipment sections are skipped with warnings, following
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

MATPOWER skips voltage source converter (VSC) and multi-terminal DC data. Set
`all_dc_lines: true` to append them to `Case::dcline` as well, after the
two-terminal rows, with `RawImport::dc_line_kinds` saying which section each
row came from. Both are lossless approximations and warn:

- A VSC line is one row from its sending converter to its receiving converter,
  scheduled at the MW-controlled converter's DCSET. A converter in AC voltage
  control holds ACSET within MINQ..MAXQ; one in power factor control gets a
  fixed reactive output at that factor.
- A multi-terminal system is one row per scheduled converter, linked to the
  converter that holds the DC voltage (VCONV), which picks up the balance.
  Current-mode SETVL values are turned into MW with VCONV's scheduled DC
  voltage. The DC buses and DC links are not modelled, and the negative
  pole's voltage converter (VCONVN) gets no row.

MATPOWER writes the reactive limits it estimates for line-commutated
converters as positive (supplied) MVAr, although these converters absorb
reactive power. Set `lcc_absorbs_reactive: true` to store them as
`[-QMAX, -QMIN]` instead for two-terminal and multi-terminal rows; VSC rows are
unchanged. When solving, a DC line end whose reactive range has zero width (a
VSC in power factor control) injects that fixed MVAr rather than holding its
bus voltage.

### Remote voltage regulation

MATPOWER holds every generator's voltage setpoint at its own bus. Set
`remote_regulation: true` to keep PSS/E's IREG and RMPCT instead, in
`Generator::regulated_bus` and `Generator::reactive_share`. When reactive
limits are enforced, the solver then holds each regulated bus at the
setpoint, freeing the generator terminals' voltages, and generators on
different buses that hold the same bus share its reactive power in
proportion to RMPCT (generators on one bus share as before). A generator at a
limit is fixed there and stops holding the bus; it takes the bus back in a
later pass if the bus's voltage moves past the setpoint on the other side.
A generator whose output already sits at a limit in the case starts at that
limit, since solved cases often leave small units pinned there. A group falls
back to holding its own bus when the regulated bus is a reference bus, is in
another island, or itself regulates a remote bus.

Without reactive limits, regulated generators hold their own bus, as in
MATPOWER: nothing then stops a small unit from driving its terminal to an
extreme voltage to hold a distant bus. Sensitivities and Jacobians from
[`make_jacobian`] still use the local formulation.

On the Texas A&M SyntheticUSA case (82,000 buses, 2,730 remote-regulating
generators) the reactive-limit solve diverged with local setpoints and
converges with remote regulation, landing within 0.001 p.u. on average of
the solved voltages saved in the case.

### Switched shunts and transformer taps

`RawImport::voltage_controls` lists the in-service switched shunts in voltage
control (MODSW 1 discrete, 2 continuous, revision 30 and later) and the
transformer windings in voltage control (COD 1). They act only when passed in
`PowerFlowOptions::voltage_controls`, and only in AC solves; the case's
`bs` and `tap` are their starting point either way.

- A discrete shunt switches one block at a time, between Newton solves, while
  its bus is outside VSWLO..VSWHI. A device that reverses direction twice is
  held, so that it cannot hunt.
- A continuous shunt holds the middle of its band within the blocks' range,
  as a static var compensator does. It is solved as a reactive source and,
  once it reaches an end of its range, becomes a fixed shunt there.
- A tap steps by whole positions (NTP) within RMI..RMA while the controlled
  bus (CONT) is outside VMI..VMA, aiming a little inside the nearer edge. The
  step size comes from the measured voltage response; a tap that barely moves
  its bus is held.
- Shunts and taps move only once the generators' reactive limits have
  settled, so they do not fight them. If the solve after a move fails, the
  move is undone and the devices it moved are held.

A shunt or tap whose bus is held by a generator stays put.
`PowerFlowResult::switched_shunts` and `tap_ratios` give the final values in
list order, and `voltage_control_passes` the number of passes that moved
anything. Sensitivities treat the final shunts and taps as fixed.

On SyntheticUSA (150 continuous shunts, 291 taps) the reactive-limit solve
still converges with both on and the largest bus voltage error against the
saved solution drops from 0.076 to 0.047 p.u. On the Memphis case (61
continuous shunts) the mean error drops from 0.0013 to 0.00006 p.u.

### Detailed two-terminal DC model

`RawImport::lcc_lines` keeps every field of each two-terminal DC record as an
`LccLine` that names its `Case::dcline` row. Pass them in
`PowerFlowOptions::dc_model = DcModel::Detailed { lines, adjust_taps }` to
solve those rows with converter equations instead of the generator pair; other
rows stay simplified, and DC (linear) solves always use the simplified model.

- Each converter is a six-pulse bridge model in PSS/E units: no-load voltage
  `Vdo = 3√2/π · NB · E · TR / TAP`, commutation drop `3/π · Xc` and
  resistive drop `2 · Rc` per bridge, DC line resistance RDC, and
  `Q = Id · √(Vdo² − Vd²)` absorbed at both ends.
- The rectifier holds the power (SETVL > 0 at the rectifier, < 0 at the
  inverter) or current order with α; the inverter runs at minimum γ. At
  minimum α, the inverter takes the order less the DELTI margin. Below VCMOD,
  a power order becomes the current SETVL / VSCHD.
- The operating point is recomputed from the terminal voltage magnitudes at
  every Newton step, with its voltage derivatives in the Jacobian. With
  `adjust_taps`, converter taps step between Newton solves (like reactive-limit
  passes) to bring the compounded DC voltage to VSCHD at RCOMP and α into its
  ANMN..ANMX range.
- `PowerFlowResult::lcc` reports current, DC voltages, angles, taps, control
  mode, losses and both ends' P and Q; the `dcline` row gets the AC-side P and
  Q (absorbed Q negative) and terminal voltages.
- Not modelled: capacitor-commutated converters (XCAP), the AC transformer
  named by IF/IT/ID (the tap is applied inside the converter), a separate
  commutating bus (IC), and DCVMIN. Import warns when a line uses them.

### Detailed VSC model

With `all_dc_lines`, `RawImport::vsc_lines` keeps every VSC line that has one
DC voltage converter (TYPE 1) and one power converter (TYPE 2) as a
`VscLine`. Pass them as `DcModel::Detailed { vsc_lines, .. }`:

- The power converter holds DCSET MW at its AC bus; the other holds DCSET kV
  on the DC side. Converter losses are `ALOSS + BLOSS · Idc` (at least
  MINLOSS), and the DC line loses `RDC · Idc²`, so the voltage end delivers
  the order less all three.
- On the AC side each converter holds its voltage within MINQ..MAXQ (MODE 1)
  or its power factor (MODE 2), as in the simplified model.
- After each converged Newton solve, a converter above SMAX or its IMAX
  current at the solved voltage has its output cut, weighted by PWF (0 cuts
  only P, 1 only Q): P through the power order, Q at a power factor end
  directly, and Q at a voltage end through tighter Q limits (enforced when
  `enforce_q_limits` is on). Cuts only shrink, so the passes end.
- `PowerFlowResult::vsc` reports the DC current and voltages, each loss, the
  kept fraction of the order, and which ends were limited.
- Not modelled: remote voltage regulation (REMOT, RMPCT); import warns.

### Detailed multi-terminal DC model

With `all_dc_lines`, `RawImport::mtdc` keeps each multi-terminal system as an
`MtdcSystem`: its converters, DC buses and DC links, with each scheduled
converter naming its `Case::dcline` row. Pass them as
`DcModel::Detailed { mtdc, .. }`:

- Each converter uses the two-terminal bridge equations and sits between its
  DC bus (IDC) and a second DC bus or ground (IDC2), on the pole its CNVCOD
  sign gives.
- The converter at VCONV (VCONVN on the negative pole) holds SETVL kV across
  it. The others hold SETVL MW of DC power, or SETVL amps in current control,
  positive at a rectifier and negative at an inverter. The DC network is
  solved by Newton's method on the DC bus voltages, so the voltage converters
  take the balance and the link losses.
- A converter that would need an angle below its minimum runs at that
  minimum instead and falls short of its order. Below VCMOD, power orders
  become current orders at SETVL over the voltage converter's SETVL.
- The DC solution is recomputed from the converter bus voltages at every
  Newton step, with numerical derivatives in the Jacobian. With
  `adjust_taps`, each converter's tap steps to keep its angle within
  ANGMN..ANGMX; when one tap step is wider than that range, the angle stays
  at or above ANGMN.
- `PowerFlowResult::mtdc` reports every DC bus voltage, link current, flow
  and loss, and each converter's mode, current, DC voltage, angle, tap and
  AC-side P and Q. Each `dcline` row gets its converter's P and Q at the
  converter end; the voltage converter's end gets a share of the voltage
  converters' Q and of the system's losses, in proportion to the converter's
  DC power, so the rows add up to the system's losses.
- A system is blocked when MDC is 0 or any of its rows is switched off.
- Not modelled: DCPF and MARG order sharing, RGRND (fault analysis only in
  PSS/E) and link inductance. Import warns about margins and ground
  resistances.

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
tolerance is `5e-6` MVAr. The iteration count includes all limit passes. A
Newton step that would change any voltage magnitude by more than half is
shortened so that none does; ordinary steps are far smaller and unaffected.

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

See the [contributor guide](CONTRIBUTING.md)
for the repository layout and fixture workflow. Run these checks from this crate's directory:

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

Software is BSD-3-Clause. See [NOTICE](NOTICE) for MATPOWER attribution and its
retained license. MATPOWER explicitly excludes case data from its software BSD
license; the bundled cases retain their original source and citation notices.
