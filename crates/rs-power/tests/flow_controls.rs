//! Phase shifters holding transformer active power flows within a band.
use rs_power::{
    Asymmetric, Case, FlowControls, PhaseShifter, PowerFlowOptions, ReactiveTap, SolveStatus,
    parse_matpower, solve,
};

/// case9 with branch 4-5 (index 1) as a phase shifter.
fn case9() -> Case {
    let mut case = parse_matpower(include_str!("fixtures/case9.m")).unwrap();
    case.branches[1].tap = 1.0;
    case
}

fn shifter(band: (f64, f64), asymmetric: Option<Asymmetric>) -> PhaseShifter {
    PhaseShifter {
        branch: 1,
        mw_min: band.0,
        mw_max: band.1,
        angle_min: -30.0,
        angle_max: 30.0,
        angle: 0.0,
        asymmetric,
        line: 0,
    }
}

fn with(shifters: Vec<PhaseShifter>, lock: bool) -> PowerFlowOptions {
    PowerFlowOptions {
        flow_controls: FlowControls {
            phase_shifters: shifters,
            ..Default::default()
        },
        lock_phase_shifters: lock,
        ..Default::default()
    }
}

fn flow(case: &Case, opt: &PowerFlowOptions) -> (f64, rs_power::PowerFlowResult) {
    let result = solve(case, opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    (result.branch_flows[1][0], result)
}

#[test]
fn a_shifter_inside_its_band_stays_put() {
    let case = case9();
    let (before, _) = flow(&case, &PowerFlowOptions::default());
    let opt = with(vec![shifter((before - 10.0, before + 10.0), None)], false);
    let (after, result) = flow(&case, &opt);
    assert_eq!(result.phase_shift_angles, [0.0]);
    assert_eq!(result.flow_control_passes, 0);
    assert!((after - before).abs() < 1e-6);
}

#[test]
fn a_shifter_moves_its_flow_into_the_band() {
    let case = case9();
    let (before, _) = flow(&case, &PowerFlowOptions::default());
    // Push 30 MW more through 4-5, then 30 MW less.
    for target in [before + 30.0, before - 30.0] {
        let opt = with(vec![shifter((target - 2.0, target + 2.0), None)], false);
        let (after, result) = flow(&case, &opt);
        assert!(
            (target - 2.1..=target + 2.1).contains(&after),
            "{after} vs {target}"
        );
        let angle = result.phase_shift_angles[0];
        // More flow from 4 to 5 takes a negative shift (MATPOWER's sign:
        // the from side leads by the shift).
        assert_eq!(angle < 0.0, target > before, "{angle}");
        assert_eq!(result.case.branches[1].shift, angle);
        assert!(
            result.flow_control_passes <= 4,
            "{}",
            result.flow_control_passes
        );
    }
}

#[test]
fn a_shifter_stops_at_its_angle_limit() {
    let case = case9();
    let (before, _) = flow(&case, &PowerFlowOptions::default());
    let mut s = shifter((before + 500.0, before + 510.0), None);
    (s.angle_min, s.angle_max) = (-3.0, 3.0);
    let (after, result) = flow(&case, &with(vec![s], false));
    assert_eq!(result.phase_shift_angles, [-3.0]);
    assert!(after > before && after < before + 500.0, "{after}");
}

#[test]
fn a_locked_shifter_keeps_its_angle_and_is_still_listed() {
    let case = case9();
    let (before, _) = flow(&case, &PowerFlowOptions::default());
    let mut s = shifter((before + 30.0, before + 40.0), None);
    s.angle = 2.0;
    let mut case = case;
    case.branches[1].shift = 2.0;
    let (_, result) = flow(&case, &with(vec![s], true));
    assert_eq!(result.phase_shift_angles, [2.0]);
    assert_eq!(result.flow_control_passes, 0);
}

#[test]
fn an_asymmetric_shifter_moves_its_ratio_with_its_angle() {
    let case = case9();
    let (before, _) = flow(&case, &PowerFlowOptions::default());
    let target = before + 20.0;
    let quadrature = Asymmetric {
        connection_angle: 90.0,
        tap: 1.0,
    };
    let opt = with(
        vec![shifter((target - 1.0, target + 1.0), Some(quadrature))],
        false,
    );
    let (after, result) = flow(&case, &opt);
    assert!((target - 1.1..=target + 1.1).contains(&after), "{after}");
    let angle = result.phase_shift_angles[0];
    // A quadrature booster's ratio is 1 / cos(angle).
    let tap = result.case.branches[1].tap;
    assert!(
        (tap - 1.0 / angle.to_radians().cos()).abs() < 1e-12,
        "{tap}"
    );
    assert!(tap > 1.0);
}

#[test]
fn asymmetric_magnitude_follows_the_connection_angle() {
    let a = Asymmetric {
        connection_angle: 60.0,
        tap: 1.0,
    };
    assert!((a.magnitude(0.0) - 1.0).abs() < 1e-12);
    // 1 + k e^{j60°} with k = 1: angle 30°, magnitude sqrt(3).
    assert!((a.magnitude(30.0) - 3f64.sqrt()).abs() < 1e-12);
    assert_eq!(a.reach(), (-115.0, 55.0));
}

#[test]
fn an_invalid_shifter_is_rejected() {
    let case = case9();
    let mut s = shifter((0.0, 10.0), None);
    s.branch = 99;
    assert!(solve(&case, &with(vec![s], false)).is_err());
}

#[test]
fn raw_import_reads_phase_shifters() {
    // Two-winding 10-20 winding 1 in COD 3, 100-120 MW band, ±30°; a
    // second in COD -3 (suppressed) and a third in COD 5 with CNXA 90.
    let text = "0,100,33,0,0,60
phase shifters
test
10,'A',230,3,1,1,1,1,0,1.1,.9
20,'B',230,1,1,1,1,1,0,1.1,.9
0 / END OF BUS DATA
20,1,1,1,1,150,10,0,0,0,0
0 / END OF LOAD DATA
0 / END OF FIXED SHUNT DATA
10,1,150,0,1000,-1000,1,0,100,0,1,0,0,1,1,100,1000,0
0 / END OF GENERATOR DATA
10,20,'1',0.01,0.1,0.02,250,250,250,0,0,0,0,1,1,0,1,1
0 / END OF BRANCH DATA
10,20,0,'2',1,1,1,0,0,2,'ps',1,1,1
0.001,0.05,100
1,230,-5,250,250,250,3,0,30,-30,120,100,33,0,0,0,0
1,230
10,20,0,'3',1,1,1,0,0,2,'off',1,1,1
0.001,0.05,100
1,230,0,250,250,250,-3,0,30,-30,120,100,33,0,0,0,0
1,230
10,20,0,'4',1,1,1,0,0,2,'asym',1,1,1
0.001,0.05,100
1,230,4,250,250,250,5,0,30,-30,60,40,33,0,0,0,90
1,230
0 / END OF TRANSFORMER DATA
Q
";
    let imported =
        rs_power::parse_raw_with_options(text, &rs_power::RawOptions::default()).unwrap();
    let shifters = &imported.flow_controls.phase_shifters;
    assert_eq!(shifters.len(), 2);
    let p = &shifters[0];
    assert_eq!(p.branch, 1);
    assert_eq!((p.mw_min, p.mw_max), (100.0, 120.0));
    assert_eq!((p.angle_min, p.angle_max, p.angle), (-30.0, 30.0, -5.0));
    assert!(p.asymmetric.is_none());
    let q = &shifters[1];
    assert_eq!(q.branch, 3);
    assert_eq!(q.angle, 4.0);
    assert_eq!(q.asymmetric.unwrap().connection_angle, 90.0);
    assert!(
        !imported
            .warnings
            .iter()
            .any(|w| w.message.contains("power or impedance-correction")),
        "{:?}",
        imported.warnings
    );
    let result = solve(
        &imported.case,
        &PowerFlowOptions {
            flow_controls: imported.flow_controls.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    for p in shifters {
        let f = result.branch_flows[p.branch][0];
        let (lo, hi) = p.band();
        assert!(
            f > lo - 0.2 && f < hi + 0.2,
            "{f} outside {lo}-{hi}: {:?} {}",
            result.phase_shift_angles,
            result.flow_control_passes
        );
    }
}

/// A reactive flow tap on case9's transformer 8-2 (index 6), from bus 8.
fn reactive_tap(band: (f64, f64)) -> ReactiveTap {
    ReactiveTap {
        branch: 6,
        mvar_min: band.0,
        mvar_max: band.1,
        ratio: 1.0,
        ratio_max: 1.1,
        ratio_min: 0.9,
        positions: 33,
        tap_per_ratio: 1.0,
        line: 0,
    }
}

fn with_taps(taps: Vec<ReactiveTap>, lock: bool) -> PowerFlowOptions {
    PowerFlowOptions {
        flow_controls: FlowControls {
            reactive_taps: taps,
            ..Default::default()
        },
        lock_taps: lock,
        ..Default::default()
    }
}

#[test]
fn a_reactive_flow_tap_steps_its_mvar_into_the_band() {
    let mut case = case9();
    case.branches[6].tap = 1.0;
    let before = solve(&case, &PowerFlowOptions::default())
        .unwrap()
        .branch_flows[6][1];
    for target in [before + 15.0, before - 15.0] {
        let t = reactive_tap((target - 3.0, target + 3.0));
        let step = t.step();
        let result = solve(&case, &with_taps(vec![t], false)).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        let after = result.branch_flows[6][1];
        assert!(
            (target - 3.1..=target + 3.1).contains(&after),
            "{after} vs {target}"
        );
        let ratio = result.reactive_tap_ratios[0];
        // Whole tap steps from the starting ratio.
        let steps = (ratio - 1.0) / step;
        assert!(
            (steps - steps.round()).abs() < 1e-9 && steps != 0.0,
            "{steps}"
        );
        assert_eq!(result.case.branches[6].tap, ratio);
        assert!(
            result.flow_control_passes <= 4,
            "{}",
            result.flow_control_passes
        );
    }
}

#[test]
fn a_locked_reactive_flow_tap_keeps_its_ratio() {
    let mut case = case9();
    case.branches[6].tap = 1.0;
    let before = solve(&case, &PowerFlowOptions::default())
        .unwrap()
        .branch_flows[6][1];
    let t = reactive_tap((before + 10.0, before + 12.0));
    let result = solve(&case, &with_taps(vec![t], true)).unwrap();
    assert_eq!(result.reactive_tap_ratios, [1.0]);
    assert_eq!(result.flow_control_passes, 0);
}

#[test]
fn raw_import_reads_reactive_flow_taps() {
    // Winding 10-20 in COD 2 holding 5-15 Mvar, ratio 0.9-1.1 in 33 steps.
    let text = "0,100,33,0,0,60
reactive flow tap
test
10,'A',230,3,1,1,1,1,0,1.1,.9
20,'B',230,1,1,1,1,1,0,1.1,.9
0 / END OF BUS DATA
20,1,1,1,1,100,40,0,0,0,0
0 / END OF LOAD DATA
0 / END OF FIXED SHUNT DATA
10,1,100,0,1000,-1000,1,0,100,0,1,0,0,1,1,100,1000,0
0 / END OF GENERATOR DATA
10,20,'1',0.01,0.1,0.02,250,250,250,0,0,0,0,1,1,0,1,1
0 / END OF BRANCH DATA
10,20,0,'2',1,1,1,0,0,2,'q',1,1,1
0.001,0.05,100
1,230,0,250,250,250,2,0,1.1,0.9,15,5,33,0,0,0,0
1,230
0 / END OF TRANSFORMER DATA
Q
";
    let imported =
        rs_power::parse_raw_with_options(text, &rs_power::RawOptions::default()).unwrap();
    let taps = &imported.flow_controls.reactive_taps;
    assert_eq!(taps.len(), 1);
    let t = &taps[0];
    assert_eq!((t.branch, t.positions), (1, 33));
    assert_eq!((t.mvar_min, t.mvar_max), (5.0, 15.0));
    assert_eq!((t.ratio_min, t.ratio_max, t.ratio), (0.9, 1.1, 1.0));
    assert!(
        !imported
            .warnings
            .iter()
            .any(|w| w.message.contains("power or impedance-correction")),
        "{:?}",
        imported.warnings
    );
    let result = solve(
        &imported.case,
        &PowerFlowOptions {
            flow_controls: imported.flow_controls.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let q = result.branch_flows[1][1];
    // Whole steps of 0.00625 can overshoot a little; the band is 10 Mvar.
    assert!(
        (4.0..=16.0).contains(&q),
        "{q} with ratio {:?}",
        result.reactive_tap_ratios
    );
}

#[test]
fn a_shifter_and_a_reactive_flow_tap_move_together() {
    let mut case = case9();
    case.branches[6].tap = 1.0;
    let base = solve(&case, &PowerFlowOptions::default()).unwrap();
    let (p0, q0) = (base.branch_flows[1][0], base.branch_flows[6][1]);
    let opt = PowerFlowOptions {
        flow_controls: FlowControls {
            phase_shifters: vec![shifter((p0 + 20.0, p0 + 24.0), None)],
            reactive_taps: vec![reactive_tap((q0 - 15.0, q0 - 10.0))],
        },
        ..Default::default()
    };
    let result = solve(&case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let (p, q) = (result.branch_flows[1][0], result.branch_flows[6][1]);
    assert!((p0 + 19.9..=p0 + 24.1).contains(&p), "{p}");
    assert!((q0 - 15.1..=q0 - 9.9).contains(&q), "{q}");
}
