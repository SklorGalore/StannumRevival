//! The decoupled AC methods and the automatic choice between them and Newton.
use rs_power::{
    AcMethod, Case, DcModel, Initialization, PowerFlowOptions, PowerFlowResult, RawOptions,
    SolveStatus, parse_matpower, parse_raw_with_options, solve,
};

const DECOUPLED: [AcMethod; 3] = [
    AcMethod::FixedSlope,
    AcMethod::FastDecoupledXb,
    AcMethod::FastDecoupledBx,
];

fn case(name: &str) -> Case {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.m"));
    parse_matpower(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// `case` with every load and dispatch scaled by `factor`.
fn loaded(mut case: Case, factor: f64) -> Case {
    for b in &mut case.buses {
        b.pd *= factor;
        b.qd *= factor;
    }
    for g in &mut case.generators {
        g.pg *= factor;
    }
    case
}

fn flat(method: AcMethod) -> PowerFlowOptions {
    PowerFlowOptions {
        method,
        initialization: Initialization::Flat,
        max_iterations: 20,
        ..Default::default()
    }
}

/// Largest voltage magnitude (p.u.) and angle (degrees) difference.
fn difference(a: &PowerFlowResult, b: &PowerFlowResult) -> (f64, f64) {
    a.case
        .buses
        .iter()
        .zip(&b.case.buses)
        .fold((0.0, 0.0), |(m, d), (x, y)| {
            (m.max((x.vm - y.vm).abs()), d.max((x.va - y.va).abs()))
        })
}

#[test]
fn decoupled_methods_reach_the_newton_solution_of_a_stressed_case() {
    let case = loaded(case("case118"), 2.0);
    let newton = solve(&case, &flat(AcMethod::Newton)).unwrap();
    assert_eq!(newton.status, SolveStatus::Converged);
    assert_eq!(newton.decoupled_iterations, 0);
    for method in DECOUPLED {
        let result = solve(&case, &flat(method)).unwrap();
        assert_eq!(result.status, SolveStatus::Converged, "{method:?}");
        assert!(result.residual < 1e-8);
        // Cheaper iterations, more of them.
        assert!(result.iterations > newton.iterations, "{method:?}");
        assert_eq!(result.decoupled_iterations, result.iterations);
        assert_eq!(result.newton_fallbacks, 0);
        let (vm, va) = difference(&result, &newton);
        assert!(
            vm < 1e-7 && va < 1e-5,
            "{method:?}: {vm:e} p.u., {va:e} degrees"
        );
    }
}

#[test]
fn auto_finishes_with_newton_where_decoupled_iterations_stall() {
    // Heavily loaded: Newton converges, the decoupled methods do not.
    for (name, factor) in [("case9", 2.5), ("case118", 3.0)] {
        let case = loaded(case(name), factor);
        let newton = solve(&case, &flat(AcMethod::Newton)).unwrap();
        assert_eq!(newton.status, SolveStatus::Converged, "{name}");
        for method in DECOUPLED {
            let result = solve(&case, &flat(method)).unwrap();
            assert_eq!(
                result.status,
                SolveStatus::IterationLimit,
                "{name} {method:?}"
            );
        }
        let auto = solve(&case, &flat(AcMethod::Auto)).unwrap();
        assert_eq!(auto.status, SolveStatus::Converged, "{name}");
        assert_eq!(auto.newton_fallbacks, 1, "{name}");
        assert!(auto.decoupled_iterations > 0);
        assert!(auto.iterations > auto.decoupled_iterations);
        let (vm, va) = difference(&auto, &newton);
        assert!(
            vm < 1e-7 && va < 1e-5,
            "{name}: {vm:e} p.u., {va:e} degrees"
        );
    }
}

#[test]
fn auto_stays_decoupled_when_that_converges() {
    let case = case("case300");
    let auto = solve(&case, &flat(AcMethod::Auto)).unwrap();
    assert_eq!(auto.status, SolveStatus::Converged);
    assert_eq!(auto.newton_fallbacks, 0);
    assert_eq!(auto.decoupled_iterations, auto.iterations);
}

#[test]
fn decoupled_iteration_limit_is_its_own() {
    let case = case("case30");
    let options = PowerFlowOptions {
        decoupled_iterations: 3,
        // Ignored by the decoupled methods.
        max_iterations: 0,
        ..flat(AcMethod::FastDecoupledXb)
    };
    let result = solve(&case, &options).unwrap();
    assert_eq!(result.status, SolveStatus::IterationLimit);
    assert_eq!(result.iterations, 3);
}

#[test]
fn every_method_solves_dc_lines_shunts_and_reactive_limits_alike() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let imported = parse_raw_with_options(
        &std::fs::read_to_string(path).unwrap(),
        &RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            remote_regulation: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!imported.lcc_lines.is_empty() && !imported.vsc_lines.is_empty());
    let options = |method| PowerFlowOptions {
        method,
        enforce_q_limits: true,
        dc_model: DcModel::Detailed {
            lines: imported.lcc_lines.clone(),
            vsc_lines: imported.vsc_lines.clone(),
            mtdc: imported.mtdc.clone(),
            adjust_taps: true,
        },
        voltage_controls: imported.voltage_controls.clone(),
        ..Default::default()
    };
    let newton = solve(&imported.case, &options(AcMethod::Newton)).unwrap();
    assert_eq!(newton.status, SolveStatus::Converged);
    for method in DECOUPLED.into_iter().chain([AcMethod::Auto]) {
        let result = solve(&imported.case, &options(method)).unwrap();
        assert_eq!(result.status, SolveStatus::Converged, "{method:?}");
        assert!(result.decoupled_iterations > 0, "{method:?}");
        let (vm, va) = difference(&result, &newton);
        assert!(
            vm < 1e-6 && va < 1e-4,
            "{method:?}: {vm:e} p.u., {va:e} degrees"
        );
        // Discrete blocks land on the same steps; continuous shunts within
        // the solve's accuracy.
        for (a, b) in result.switched_shunts.iter().zip(&newton.switched_shunts) {
            assert!((a - b).abs() < 1e-4, "{method:?}: {a} and {b} MVAr");
        }
        for (a, b) in result.lcc.iter().zip(&newton.lcc) {
            assert!((a.line_loss_mw - b.line_loss_mw).abs() < 1e-4, "{method:?}");
        }
    }
}
