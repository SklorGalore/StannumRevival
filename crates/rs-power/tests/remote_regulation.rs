//! Generators holding the voltage of a bus other than their own (PSS/E IREG).
use rs_power::{BusType, Case, PowerFlowOptions, RawOptions, SolveStatus, parse_matpower, solve};

fn case9() -> Case {
    parse_matpower(include_str!("fixtures/case9.m")).unwrap()
}

fn limited() -> PowerFlowOptions {
    PowerFlowOptions {
        enforce_q_limits: true,
        ..Default::default()
    }
}

fn bus(case: &Case, id: u32) -> &rs_power::Bus {
    case.buses.iter().find(|b| b.id == id).unwrap()
}

#[test]
fn generator_holds_the_remote_bus() {
    let mut case = case9();
    case.generators[1].regulated_bus = Some(7);
    let result = solve(&case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((bus(&result.case, 7).vm - 1.025).abs() < 1e-8);
    assert!((bus(&result.case, 2).vm - 1.025).abs() > 1e-3);
    assert_eq!(bus(&result.case, 2).kind, BusType::Pv);
    // The other generator still holds its own bus.
    assert!((bus(&result.case, 3).vm - 1.025).abs() < 1e-8);
}

#[test]
fn generators_on_different_buses_share_by_reactive_share() {
    let mut case = case9();
    for g in &mut case.generators[1..] {
        g.regulated_bus = Some(7);
        g.vg = 1.03;
    }
    case.generators[2].reactive_share = 50.0;
    let result = solve(&case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((bus(&result.case, 7).vm - 1.03).abs() < 1e-8);
    let (q2, q3) = (result.case.generators[1].qg, result.case.generators[2].qg);
    assert!((q2 - 2.0 * q3).abs() < 1e-6, "{q2} {q3}");
}

#[test]
fn without_reactive_limits_generators_hold_their_own_bus() {
    let mut case = case9();
    case.generators[1].regulated_bus = Some(7);
    let result = solve(&case, &PowerFlowOptions::default()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((bus(&result.case, 2).vm - 1.025).abs() < 1e-8);
}

#[test]
fn reactive_limit_releases_the_remote_bus() {
    let mut case = case9();
    case.generators[1].regulated_bus = Some(7);
    let needed = solve(&case, &limited()).unwrap().case.generators[1].qg;
    case.generators[1].qmax = needed - 10.0;
    let result = solve(&case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((result.case.generators[1].qg - (needed - 10.0)).abs() < 1e-9);
    assert_eq!(bus(&result.case, 2).kind, BusType::Pq);
    assert!(bus(&result.case, 7).vm < 1.025 - 1e-4);
}

#[test]
fn generator_starting_at_a_limit_returns_to_voltage_control() {
    let mut case = case9();
    case.generators[1].regulated_bus = Some(7);
    let held = solve(&case, &limited()).unwrap();
    let needed = held.case.generators[1].qg;
    // A solved case can show the generator at its limit; with headroom left
    // it starts there and takes the remote bus back.
    case.generators[1].qmax = needed + 50.0;
    case.generators[1].qg = needed + 50.0;
    let result = solve(&case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(bus(&result.case, 2).kind, BusType::Pv);
    assert!((bus(&result.case, 7).vm - 1.025).abs() < 1e-8);
    assert!((result.case.generators[1].qg - needed).abs() < 1e-6);
}

#[test]
fn reference_bus_targets_fall_back_to_local_control() {
    let mut case = case9();
    case.generators[1].regulated_bus = Some(1);
    let result = solve(&case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((bus(&result.case, 2).vm - 1.025).abs() < 1e-8);
}

#[test]
fn raw_import_keeps_remote_regulation_on_request() {
    let text = include_str!("fixtures/raw/t_psse_case3.raw");
    let plain = rs_power::parse_raw_with_options(text, &RawOptions::default()).unwrap();
    assert!(
        plain
            .case
            .generators
            .iter()
            .all(|g| g.regulated_bus.is_none())
    );
    assert!(
        plain
            .warnings
            .iter()
            .any(|w| w.message.contains("remote generator regulation ignored"))
    );
    let remote = rs_power::parse_raw_with_options(
        text,
        &RawOptions {
            remote_regulation: true,
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let wind = remote
        .case
        .generators
        .iter()
        .find(|g| g.bus == 9154)
        .unwrap();
    assert_eq!(wind.regulated_bus, Some(154));
    assert_eq!(wind.reactive_share, 100.0);
    assert!(
        !remote
            .warnings
            .iter()
            .any(|w| w.message.contains("remote generator regulation ignored"))
    );
    let result = solve(&remote.case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
}

#[test]
fn memphis_remote_regulation_tracks_the_case_voltages() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("_cases/MemphisCase2026_Mar7.RAW");
    let mean_error = |remote_regulation: bool| {
        let imported = rs_power::load_raw_with_options(
            &path,
            &RawOptions {
                remote_regulation,
                ..Default::default()
            },
        )
        .unwrap();
        let result = solve(&imported.case, &limited()).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        result
            .case
            .buses
            .iter()
            .zip(&imported.case.buses)
            .map(|(a, b)| (a.vm - b.vm).abs())
            .sum::<f64>()
            / imported.case.buses.len() as f64
    };
    // The case was saved solved; holding the regulated buses reproduces its
    // voltages far more closely than holding each generator's terminal.
    let (local, remote) = (mean_error(false), mean_error(true));
    assert!(remote < 0.002, "{remote}");
    assert!(local > 0.01, "{local}");
}
