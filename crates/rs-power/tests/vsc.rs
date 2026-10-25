//! Detailed two-terminal VSC DC line model.
use rs_power::{
    DcModel, PowerFlowOptions, RawOptions, SolveStatus, VscAcControl, VscConverter, VscDcControl,
    VscLine, parse_matpower, parse_raw_with_options, solve,
};

/// Island A: bus 1 (reference) feeds bus 2, the VSC's power converter.
/// Island B: bus 3 (reference) and bus 4, the VSC's DC voltage converter,
/// which serves a load with a large reactive demand.
const TWO_ISLANDS: &str = "
function mpc = vsc_islands
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
    1 3 0 0 0 0 1 1 0 100 1 1.1 0.9;
    2 1 0 0 0 0 1 1 0 100 1 1.1 0.9;
    3 3 0 0 0 0 1 1 0 100 1 1.1 0.9;
    4 1 200 150 0 0 1 1 0 100 1 1.1 0.9;
];
mpc.gen = [
    1 0 0 9999 -9999 1 100 1 9999 -9999;
    3 0 0 9999 -9999 1 100 1 9999 -9999;
];
mpc.branch = [
    1 2 0.001 0.02 0 0 0 0 0 0 1 -360 360;
    3 4 0.01 0.1 0 0 0 0 0 0 1 -360 360;
];
mpc.dcline = [
    2 4 1 150 150 0 0 1 1 0 1000 -100 100 -100 100 0 0;
];
";

fn converter(bus: u32, dc_control: VscDcControl, dc_setpoint: f64) -> VscConverter {
    VscConverter {
        bus,
        dc_control,
        ac_control: VscAcControl::Voltage,
        dc_setpoint,
        ac_setpoint: 1.0,
        loss_kw: 100.0,
        loss_kw_per_amp: 0.1,
        min_loss_kw: 50.0,
        smax_mva: 0.0,
        imax_amps: 0.0,
        power_weighting: 1.0,
        qmax: 300.0,
        qmin: -300.0,
        remote_bus: 0,
        remote_percent: 100.0,
    }
}

fn line() -> VscLine {
    let mut power = converter(2, VscDcControl::Power, -150.0);
    power.ac_control = VscAcControl::PowerFactor;
    power.ac_setpoint = 0.9;
    VscLine {
        row: 0,
        name: "vsc".into(),
        in_service: true,
        resistance: 0.5,
        converters: [power, converter(4, VscDcControl::DcVoltage, 150.0)],
        from: 0,
    }
}

fn detailed(vsc_lines: Vec<VscLine>, enforce_q_limits: bool) -> PowerFlowOptions {
    PowerFlowOptions {
        enforce_q_limits,
        dc_model: DcModel::Detailed {
            lines: Vec::new(),
            vsc_lines,
            adjust_taps: true,
        },
        ..Default::default()
    }
}

#[test]
fn hand_checked_dc_side() {
    // 150 MW drawn at bus 2: (150 kV + 0.5 Ω · I) · I = 150 MW − loss, with
    // loss = 0.1 MW + 0.1 MW/kA · I, gives I by the quadratic formula.
    let (a, b, r, v, p): (f64, f64, f64, f64, f64) = (0.1, 0.1, 0.5, 150.0, 150.0);
    let i = (-(v + b) + ((v + b) * (v + b) + 4.0 * r * (p - a)).sqrt()) / (2.0 * r);
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let result = solve(&case, &detailed(vec![line()], false)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let s = &result.vsc[0];
    assert!((s.current_ka - i).abs() < 1e-12);
    assert!((s.from_kv - (v + r * i)).abs() < 1e-9);
    assert!((s.to_kv - v).abs() < 1e-12);
    assert!((s.from_mw - 150.0).abs() < 1e-9);
    // The DC voltage end delivers what arrives less its own loss.
    assert!((s.to_mw - (v * i - (0.1 + 0.1 * i))).abs() < 1e-9);
    assert!((s.from_mw - s.to_mw - s.from_loss_mw - s.to_loss_mw - s.line_loss_mw).abs() < 1e-9);
    // Power factor 0.9 at the power converter; the voltage end holds 1.0 p.u.
    assert!((s.from_mvar - 150.0 * 0.9f64.acos().tan()).abs() < 1e-9);
    let bus4 = result.case.buses.iter().find(|b| b.id == 4).unwrap();
    assert!((bus4.vm - 1.0).abs() < 1e-9);
    // The dcline row carries the same values.
    let row = &result.case.dcline.as_ref().unwrap()[0];
    assert!((row[3] - s.from_mw).abs() < 1e-9 && (row[4] - s.to_mw).abs() < 1e-9);
    assert_eq!(result.vsc_limit_passes, 0);
}

#[test]
fn current_limit_at_a_power_factor_end_scales_the_order() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let mut l = line();
    // 120 MVA at 100 kV is 692.8 A.
    l.converters[0].imax_amps = 120.0 / (3f64.sqrt() * 100.0) * 1000.0;
    let result = solve(&case, &detailed(vec![l], false)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let s = &result.vsc[0];
    assert!(s.from_limited && !s.to_limited);
    assert!(s.order_scale < 1.0);
    let bus2 = result.case.buses.iter().find(|b| b.id == 2).unwrap();
    assert!(s.from_mw.hypot(s.from_mvar) <= 120.0 * bus2.vm * (1.0 + 1e-5));
    assert!(s.from_mw.hypot(s.from_mvar) > 110.0);
    // The power factor is kept.
    assert!((s.from_mvar / s.from_mw - 0.9f64.acos().tan()).abs() < 1e-9);
    assert!(result.vsc_limit_passes > 0);
}

#[test]
fn mva_limit_at_a_voltage_end_caps_reactive_power() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let free = solve(&case, &detailed(vec![line()], true)).unwrap();
    let q_free = free.vsc[0].to_mvar;
    let p = free.vsc[0].to_mw;
    assert!(q_free > 60.0, "{q_free}");
    // PWF = 1 cuts only reactive power.
    let mut l = line();
    l.converters[1].smax_mva = 160.0;
    let limited = solve(&case, &detailed(vec![l], true)).unwrap();
    assert_eq!(limited.status, SolveStatus::Converged);
    let s = &limited.vsc[0];
    assert!(s.to_limited);
    assert!((s.order_scale - 1.0).abs() < 1e-12);
    assert!((s.to_mw - p).abs() < 1e-9);
    assert!(s.to_mw.hypot(s.to_mvar) <= 160.0 + 1e-3);
    assert!(s.to_mvar < q_free);
}

#[test]
fn rejects_lines_without_a_voltage_and_power_pair() {
    let case = parse_matpower(TWO_ISLANDS).unwrap();
    let mut l = line();
    l.converters[1].dc_control = VscDcControl::Power;
    assert!(solve(&case, &detailed(vec![l], false)).is_err());
    let mut l = line();
    l.in_service = false;
    let off = solve(&case, &detailed(vec![l], false)).unwrap();
    assert_eq!(off.vsc[0].from_mw, 0.0);
    assert_eq!(off.case.generators[0].pg, 0.0);
}

#[test]
fn psse_case3_vsc_lines_balance_losses() {
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
    assert_eq!(
        imported
            .vsc_lines
            .iter()
            .map(|l| (l.row, l.name.as_str()))
            .collect::<Vec<_>>(),
        [(2, "VDCLINE1"), (3, "VDCLINE2")]
    );
    let first = &imported.vsc_lines[0];
    assert_eq!(first.converters[0].dc_control, VscDcControl::Power);
    assert_eq!(first.converters[0].ac_control, VscAcControl::PowerFactor);
    assert!((first.converters[1].loss_kw_per_amp - 0.15).abs() < 1e-12);
    for enforce_q_limits in [false, true] {
        let mut options = detailed(imported.vsc_lines.clone(), enforce_q_limits);
        if let DcModel::Detailed { lines, .. } = &mut options.dc_model {
            *lines = imported.lcc_lines.clone();
        }
        let result = solve(&imported.case, &options).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        assert_eq!(result.lcc.len(), 2);
        let rows = result.case.dcline.as_ref().unwrap();
        for (s, l) in result.vsc.iter().zip(&imported.vsc_lines) {
            let losses = s.from_loss_mw + s.to_loss_mw + s.line_loss_mw;
            assert!((s.from_mw - s.to_mw - losses).abs() < 1e-9);
            assert!(losses > 0.5, "{losses}");
            assert!((rows[l.row][3] - rows[l.row][4] - losses).abs() < 1e-9);
        }
        // VDCLINE1: 209 MW at 100 kV is about 2.06 kA, so about 3 MW of I²R.
        let s = &result.vsc[0];
        assert!((s.from_mw - 209.0).abs() < 1e-9);
        assert!(s.line_loss_mw > 2.9 && s.line_loss_mw < 3.1);
        assert!((s.from_mvar - 209.0 * 0.95f64.acos().tan()).abs() < 1e-9);
    }
}
