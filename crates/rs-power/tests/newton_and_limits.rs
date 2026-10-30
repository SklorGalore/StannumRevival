//! Generators leaving and returning to voltage control at reactive limits,
//! and the Newton line search.

use rs_power::{BusType, Initialization, PowerFlowOptions, SolveStatus, parse_matpower, solve};

/// Two generators a short line apart asking for different voltages, behind
/// a long line from the reference bus at 0.98 p.u., with load at bus 2.
/// Holding both setpoints takes far more reactive power than either has, so
/// the first limit pass fixes bus 2 at its Qmax and bus 3 at its Qmin. With
/// both fixed, the voltage there falls below bus 3's setpoint, and bus 3,
/// at its Qmin, has reactive power to spare.
const TUG_OF_WAR: &str = "function mpc = tug
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
\t1\t3\t0\t0\t0\t0\t1\t0.98\t0\t230\t1\t1.1\t0.9;
\t2\t2\t50\t20\t0\t0\t1\t1\t0\t230\t1\t1.1\t0.9;
\t3\t2\t0\t0\t0\t0\t1\t1\t0\t230\t1\t1.1\t0.9;
];
mpc.gen = [
\t1\t0\t0\t999\t-999\t0.98\t100\t1\t999\t0;
\t2\t0\t0\t10\t-100\t1.05\t100\t1\t100\t0;
\t3\t0\t0\t100\t-5\t1.00\t100\t1\t100\t0;
];
mpc.branch = [
\t1\t2\t0.01\t0.1\t0\t0\t0\t0\t0\t0\t1\t-360\t360;
\t2\t3\t0.001\t0.01\t0\t0\t0\t0\t0\t0\t1\t-360\t360;
];
";

#[test]
fn a_generator_at_a_limit_holds_its_voltage_again_when_it_has_room() {
    let case = parse_matpower(TUG_OF_WAR).unwrap();
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        ..Default::default()
    };
    let result = solve(&case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let bus = |n: u32| result.case.buses.iter().find(|b| b.id == n).unwrap();
    let g2 = &result.case.generators[1];
    let g3 = &result.case.generators[2];
    // Bus 2 stays at its Qmax, below its setpoint.
    assert!((g2.qg - g2.qmax).abs() < 1e-6, "Q2 {}", g2.qg);
    assert!(bus(2).vm < 1.05);
    // Bus 3 holds 1.00 again, inside its range.
    assert_eq!(bus(3).kind, BusType::Pv, "bus 3 holds its voltage");
    assert!((bus(3).vm - 1.0).abs() < 1e-6, "V3 {}", bus(3).vm);
    assert!(g3.qg > g3.qmin && g3.qg < g3.qmax, "Q3 {}", g3.qg);
}

/// A three-bus case with 450 MW at bus 2, started far from its solution:
/// both load buses at 1.3 p.u. and 50° ahead of the reference.
fn badly_started() -> String {
    "function mpc = start
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
\t1\t3\t0\t0\t0\t0\t1\t1\t0\t230\t1\t1.1\t0.9;
\t2\t1\t450\t0\t0\t0\t1\t1.3\t50\t230\t1\t1.1\t0.9;
\t3\t1\t50\t10\t0\t0\t1\t1.3\t50\t230\t1\t1.1\t0.9;
];
mpc.gen = [
\t1\t0\t0\t9999\t-9999\t1\t100\t1\t9999\t0;
];
mpc.branch = [
\t1\t2\t0.01\t0.1\t0\t0\t0\t0\t0\t0\t1\t-360\t360;
\t2\t3\t0.02\t0.2\t0\t0\t0\t0\t0\t0\t1\t-360\t360;
\t1\t3\t0.02\t0.3\t0\t0\t0\t0\t0\t0\t1\t-360\t360;
];
"
    .into()
}

#[test]
fn the_line_search_keeps_a_bad_start_from_wandering() {
    // Full Newton steps from this start overshoot again and again and run
    // out of iterations; shortening the steps that raise the mismatch
    // reaches the high-voltage solution in a few.
    let case = parse_matpower(&badly_started()).unwrap();
    let opt = PowerFlowOptions {
        initialization: Initialization::Case,
        max_iterations: 20,
        ..Default::default()
    };
    let result = solve(&case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!(result.iterations <= 10, "{} iterations", result.iterations);
    assert!((result.case.buses[1].vm - 0.8278).abs() < 1e-3);
}
