//! Detailed two-terminal LCC DC line model.
use rs_power::{
    DcModel, LccControl, LccConverter, LccEnd, LccLine, LccMode, Model, PowerFlowOptions,
    RawOptions, SolveStatus, parse_matpower, parse_raw_with_options, solve,
};

/// Rectifier at bus 1 and inverter at bus 2, each a reference bus held at
/// 1.0 p.u. in its own AC island, so the DC operating point can be checked
/// by hand.
const TWO_ISLANDS: &str = "
function mpc = two_islands
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
    1 3 0 0 0 0 1 1 0 500 1 1.1 0.9;
    2 3 0 0 0 0 1 1 0 500 1 1.1 0.9;
    3 1 600 50 0 0 1 1 0 500 1 1.1 0.9;
];
mpc.gen = [
    1 0 0 9999 -9999 1 100 1 9999 -9999;
    2 0 0 9999 -9999 1 100 1 9999 -9999;
];
mpc.branch = [
    2 3 0.001 0.01 0 0 0 0 0 0 1 -360 360;
];
mpc.dcline = [
    1 2 1 500 490 0 0 1 1 0 1000 -500 500 -500 500 0 0;
];
";

fn converter(bus: u32, tap: f64) -> LccConverter {
    LccConverter {
        bus,
        bridges: 2,
        angle_max: 20.0,
        angle_min: 15.0,
        resistance: 0.0,
        reactance: 10.0,
        base_kv: 500.0,
        ratio: 0.4,
        tap,
        tap_max: 1.5,
        tap_min: 0.5,
        tap_step: 0.00625,
        commutating_bus: 0,
        transformer: None,
        capacitor: 0.0,
    }
}

fn line() -> LccLine {
    LccLine {
        row: 0,
        name: "hand".into(),
        control: LccControl::Power,
        resistance: 10.0,
        setpoint: 500.0,
        scheduled_kv: 500.0,
        mode_switch_kv: 0.0,
        compounding_resistance: 0.0,
        current_margin: 0.1,
        metered_end: LccEnd::Rectifier,
        min_compounded_kv: 0.0,
        ccc_iterations: 20,
        ccc_acceleration: 1.0,
        rectifier: converter(1, 0.97),
        inverter: converter(2, 1.0),
    }
}

fn detailed(lines: Vec<LccLine>, adjust_taps: bool) -> PowerFlowOptions {
    PowerFlowOptions {
        dc_model: DcModel::Detailed { lines, adjust_taps },
        ..Default::default()
    }
}

#[test]
fn hand_checked_operating_point() {
    // By hand: Vdo = 3√2/π · 2 · 500 kV · 0.4 / tap, so 556.90 kV at the
    // rectifier and 540.19 kV at the inverter; the commutation drop is
    // 2 · 3/π · 10 Ω = 19.099 Ω. With γ = 15°, (Vdi + 10 Ω · Id) · Id = 500 MW
    // gives Id = 0.974823 kA, Vdi = 503.1655 kV, Vdr = 512.9138 kV and
    // α = 17.3593°. With no commutating resistance, Q = Id · √(Vdo² − Vd²):
    // 211.4566 and 191.5954 Mvar.
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let result = solve(&case, &detailed(vec![line()], false)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let s = &result.lcc[0];
    assert_eq!(s.mode, LccMode::RectifierControl);
    let close = |a: f64, b: f64, tol: f64| assert!((a - b).abs() < tol, "{a} vs {b}");
    close(s.current_ka, 0.974823, 1e-6);
    close(s.rectifier_kv, 512.9138, 1e-4);
    close(s.inverter_kv, 503.1655, 1e-4);
    close(s.alpha, 17.3593, 1e-4);
    close(s.gamma, 15.0, 1e-9);
    close(s.rectifier_mw, 500.0, 0.01);
    close(s.rectifier_mvar, 211.4566, 0.01);
    close(s.inverter_mw, 490.4972, 0.01);
    close(s.inverter_mvar, 191.5954, 0.01);
    close(s.line_loss_mw, 10.0 * 0.974823f64.powi(2), 1e-4);
    // The rectifier's island has nothing else, so its generator supplies
    // exactly what the converter draws.
    close(result.case.generators[0].pg, 500.0, 0.01);
    close(result.case.generators[0].qg, 211.4566, 0.01);
    // The dcline row reports MATPOWER-style injections: Q absorbed is negative.
    let row = &result.case.dcline.as_ref().unwrap()[0];
    close(row[3], 500.0, 0.01);
    close(row[4], 490.4972, 0.01);
    close(row[5], -211.4566, 0.01);
    close(row[6], -191.5954, 0.01);
    assert_eq!(result.dc_tap_passes, 0);
}

#[test]
fn tap_adjustment_brings_alpha_into_range() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let mut l = line();
    l.rectifier.tap = 1.2;
    let fixed = solve(&case, &detailed(vec![l.clone()], false)).unwrap();
    // At tap 1.2 the rectifier cannot reach the voltage it needs.
    assert_eq!(fixed.lcc[0].mode, LccMode::InverterControl);
    let adjusted = solve(&case, &detailed(vec![l], true)).unwrap();
    assert_eq!(adjusted.status, SolveStatus::Converged);
    let s = &adjusted.lcc[0];
    assert_eq!(s.mode, LccMode::RectifierControl);
    assert!(s.alpha >= 15.0 && s.alpha <= 20.0, "{}", s.alpha);
    assert!(adjusted.dc_tap_passes > 0);
    // The inverter tap moves to hold VSCHD = 500 kV within one tap step.
    assert!(
        (s.inverter_kv - 500.0).abs() < 0.00625 * 540.0,
        "{}",
        s.inverter_kv
    );
}

#[test]
fn power_order_at_the_inverter() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let mut l = line();
    l.setpoint = -400.0;
    let s = &solve(&case, &detailed(vec![l], false)).unwrap().lcc[0];
    assert!((s.inverter_kv * s.current_ka - 400.0).abs() < 1e-6);
}

#[test]
fn current_order_and_low_voltage_mode_switch() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let mut l = line();
    l.control = LccControl::Current;
    l.setpoint = 800.0;
    let s = &solve(&case, &detailed(vec![l.clone()], false)).unwrap().lcc[0];
    assert!((s.current_ka - 0.8).abs() < 1e-12);
    // Power control with VCMOD above the DC voltage orders SETVL / VSCHD.
    l.control = LccControl::Power;
    l.setpoint = 500.0;
    l.mode_switch_kv = 600.0;
    let s = &solve(&case, &detailed(vec![l], false)).unwrap().lcc[0];
    assert!(s.reduced_voltage);
    assert!((s.current_ka - 1.0).abs() < 1e-12);
}

#[test]
fn switched_off_line_carries_nothing_and_simplified_model_is_unchanged() {
    let mut case = parse_matpower(TWO_ISLANDS).unwrap();
    case.dcline.as_mut().unwrap()[0][2] = 0.0;
    let result = solve(&case, &detailed(vec![line()], false)).unwrap();
    assert_eq!(result.lcc[0].mode, LccMode::Blocked);
    assert_eq!(result.case.generators[0].pg, 0.0);
    // An empty detailed list, or a DC solve, uses the simplified rows.
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let simple = solve(&case, &PowerFlowOptions::default()).unwrap();
    let empty = solve(&case, &detailed(vec![], true)).unwrap();
    assert_eq!(simple.case, empty.case);
    let mut dc = detailed(vec![line()], true);
    dc.model = Model::Dc;
    assert!(solve(&case, &dc).unwrap().lcc.is_empty());
}

#[test]
fn rejects_bad_detailed_data() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let mut l = line();
    l.row = 3;
    assert!(solve(&case, &detailed(vec![l], false)).is_err());
    let mut l = line();
    l.inverter.bridges = 0;
    assert!(solve(&case, &detailed(vec![l], false)).is_err());
    assert!(solve(&case, &detailed(vec![line(), line()], false)).is_err());
}

#[test]
fn psse_case3_two_terminal_lines_lose_power_and_absorb_reactive() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let text = std::fs::read_to_string(path).unwrap();
    let imported = parse_raw_with_options(
        &text,
        &RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(imported.lcc_lines.len(), 2);
    let first = &imported.lcc_lines[0];
    assert_eq!((first.rectifier.bus, first.inverter.bus), (301, 3021));
    assert_eq!(first.control, LccControl::Power);
    assert_eq!(first.metered_end, LccEnd::Inverter);
    assert_eq!(first.inverter.transformer, Some((152, 3021, "T4".into())));
    assert!((first.compounding_resistance - 3.942).abs() < 1e-12);
    for enforce_q_limits in [false, true] {
        let mut options = detailed(imported.lcc_lines.clone(), true);
        options.enforce_q_limits = enforce_q_limits;
        let result = solve(&imported.case, &options).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        for (s, l) in result.lcc.iter().zip(&imported.lcc_lines) {
            assert_eq!(s.mode, LccMode::RectifierControl);
            // The order is DC power at the rectifier.
            assert!((s.rectifier_kv * s.current_ka - l.setpoint).abs() < 1e-6);
            // About 2.8 kA through about 8 Ω: 60 to 64 MW of I²R loss.
            assert!(
                s.line_loss_mw > 55.0 && s.line_loss_mw < 70.0,
                "{}",
                s.line_loss_mw
            );
            assert!(s.rectifier_mvar > 0.3 * l.setpoint && s.inverter_mvar > 0.3 * l.setpoint);
            // Compounded DC voltage within a tap step of VSCHD.
            let compounded = s.inverter_kv + l.compounding_resistance * s.current_ka;
            assert!((compounded - l.scheduled_kv).abs() < 5.0, "{compounded}");
            assert!(s.alpha >= l.rectifier.angle_min && s.alpha <= l.rectifier.angle_max);
            assert!((s.gamma - l.inverter.angle_min).abs() < 1e-9);
        }
    }
}
