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
    // Limit passes solve loosely, but the answer meets the full tolerance.
    assert!(result.residual < limited().tolerance);
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

#[test]
fn memphis_sensitivities_follow_remote_regulation() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("_cases/MemphisCase2026_Mar7.RAW");
    let imported = rs_power::load_raw_with_options(
        &path,
        &RawOptions {
            remote_regulation: true,
            ..Default::default()
        },
    )
    .unwrap();
    let s = rs_power::calculate_sensitivities(&imported.case, &limited()).unwrap();
    let solved = &s.power_flow.case;
    let kind = |id: u32| solved.buses.iter().find(|b| b.id == id).unwrap().kind;
    // A load bus held by a generator elsewhere keeps its magnitude, so it
    // has no magnitude state; the generator's bus has one instead.
    let (generator_bus, held) = solved
        .generators
        .iter()
        .filter(|g| g.online && kind(g.bus) == BusType::Pv)
        .find_map(|g| {
            let r = g.regulated_bus.filter(|&r| r != g.bus)?;
            (kind(r) == BusType::Pq).then_some((g.bus, r))
        })
        .expect("Memphis has generators holding remote load buses");
    let states = &s.state_variables;
    assert!(!states.contains(&rs_power::StateVariable::VoltageMagnitude(held)));
    assert!(states.contains(&rs_power::StateVariable::VoltageMagnitude(generator_bus)));
    assert!(
        s.state_sensitivities
            .iter()
            .flatten()
            .all(|x| x.is_finite())
    );
}

/// One plant substation from the 80,000-bus synthetic case: a big unit at
/// the reference bus holding its own terminal, and two small units holding
/// the 500 kV bus by equal shares. Holding it takes far more than the small
/// units have. On that case the first solve left the 10 Mvar unit's
/// terminal at 0.15 pu, nearly in antiphase; bus 3 starts there.
const FIGHTING_PLANT: &str = "function mpc = fighting_plant
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
  1 3 0 0 0 0 1 1.03 0 22 1 1.1 0.9;
  2 1 0 0 0 0 1 1.0347 -13.87 500 1 1.1 0.9;
  3 2 0 0 0 0 1 0.15 167 13.8 1 1.1 0.9;
  4 2 0 0 0 0 1 1.0058 -9.08 18 1 1.1 0.9;
  5 1 9000 0 0 1500 1 1.0 -20 500 1 1.1 0.9;
];
mpc.gen = [
  1 9600 0 1477 -738 1.03 2735 1 9999 0;
  3 15 0 10 -5 1.03 18 1 16 0;
  4 143 0 91 -46 1.03 169 1 152 0;
];
mpc.branch = [
  1 2 0.0002 0.008 0 1500 0 0 1 0 1 -360 360;
  1 2 0.0002 0.008 0 1500 0 0 1 0 1 -360 360;
  1 2 0.0002 0.008 0 1500 0 0 1 0 1 -360 360;
  3 2 0.003 0.12 0 100 0 0 1 0 1 -360 360;
  4 2 0.0015 0.06 0 200 0 0 1 0 1 -360 360;
  2 5 0.0001 0.0036 0.34 4000 0 0 0 0 1 -360 360;
  2 5 0.0001 0.0036 0.34 4000 0 0 0 0 1 -360 360;
  2 5 0.0001 0.0036 0.34 4000 0 0 0 0 1 -360 360;
];
";

#[test]
fn unit_held_at_its_limit_does_not_collapse_its_terminal() {
    let mut case = parse_matpower(FIGHTING_PLANT).unwrap();
    for g in &mut case.generators[1..] {
        g.regulated_bus = Some(2);
    }
    let result = solve(&case, &limited()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    // Both small units end at their limits. Before the unit held at its
    // limit restarted from the bus it held, bus 3 settled at 0.02 pu.
    assert_eq!(result.case.generators[1].qg, 10.0);
    assert_eq!(result.case.generators[2].qg, 91.0);
    assert_eq!(bus(&result.case, 3).kind, BusType::Pq);
    assert!(
        result.case.buses.iter().all(|b| b.vm > 0.95),
        "{:?}",
        result.case.buses.iter().map(|b| b.vm).collect::<Vec<_>>()
    );
}
