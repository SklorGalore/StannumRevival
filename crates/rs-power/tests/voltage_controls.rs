//! Switched shunts and transformer taps holding bus voltages within a band.
use rs_power::{
    Case, PowerFlowOptions, RawOptions, ShuntMode, SolveStatus, SwitchedShunt, TapControl,
    VoltageControls, parse_matpower, solve,
};

fn case9() -> Case {
    parse_matpower(include_str!("fixtures/case9.m")).unwrap()
}

fn vm(case: &Case, id: u32) -> f64 {
    case.buses.iter().find(|b| b.id == id).unwrap().vm
}

fn shunt(bus: u32, mode: ShuntMode, band: (f64, f64), blocks: Vec<(u32, f64)>) -> SwitchedShunt {
    SwitchedShunt {
        bus,
        mode,
        regulated_bus: None,
        vmin: band.0,
        vmax: band.1,
        initial: 0.0,
        blocks,
        line: 0,
    }
}

fn with(controls: VoltageControls) -> PowerFlowOptions {
    PowerFlowOptions {
        voltage_controls: controls,
        ..Default::default()
    }
}

#[test]
fn shunt_levels_list_reactor_and_capacitor_steps() {
    let s = shunt(
        5,
        ShuntMode::Discrete,
        (0.95, 1.05),
        vec![(2, -10.0), (1, 25.0), (2, 5.0)],
    );
    assert_eq!(s.levels(), vec![-20.0, -10.0, 0.0, 25.0, 30.0, 35.0]);
}

#[test]
fn discrete_shunt_switches_blocks_until_the_bus_is_in_band() {
    let case = case9();
    let before = vm(&solve(&case, &PowerFlowOptions::default()).unwrap().case, 5);
    assert!(before < 1.03, "{before}");
    let controls = VoltageControls {
        switched_shunts: vec![shunt(5, ShuntMode::Discrete, (1.03, 1.04), vec![(8, 10.0)])],
        taps: vec![],
    };
    let result = solve(&case, &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let after = vm(&result.case, 5);
    assert!((1.03..=1.04).contains(&after), "{after}");
    let b = result.switched_shunts[0];
    assert!(b > 0.0 && b % 10.0 == 0.0, "{b}");
    assert!(result.voltage_control_passes > 0);
    // The solved case carries the new susceptance.
    let bus = result.case.buses.iter().find(|b| b.id == 5).unwrap();
    assert!((bus.bs - b).abs() < 1e-9);
}

#[test]
fn continuous_shunt_holds_the_middle_of_its_band() {
    let controls = VoltageControls {
        switched_shunts: vec![shunt(
            9,
            ShuntMode::Continuous,
            (1.0, 1.02),
            vec![(1, 200.0)],
        )],
        taps: vec![],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((vm(&result.case, 9) - 1.01).abs() < 1e-8);
    let b = result.switched_shunts[0];
    assert!(b > 0.0 && b < 200.0, "{b}");
    // No helper generator is left in the result, and the bus is a load bus
    // again.
    assert_eq!(result.case.generators.len(), 3);
    let bus = result.case.buses.iter().find(|b| b.id == 9).unwrap();
    assert_eq!(bus.kind, rs_power::BusType::Pq);
}

#[test]
fn sensitivities_run_on_the_controlled_solution() {
    let controls = VoltageControls {
        switched_shunts: vec![shunt(
            9,
            ShuntMode::Continuous,
            (1.0, 1.02),
            vec![(1, 200.0)],
        )],
        taps: vec![],
    };
    let s = rs_power::calculate_sensitivities(&case9(), &with(controls)).unwrap();
    assert!(!s.state_variables.is_empty());
}

#[test]
fn continuous_shunt_stops_at_its_largest_block() {
    let controls = VoltageControls {
        switched_shunts: vec![shunt(9, ShuntMode::Continuous, (1.0, 1.02), vec![(1, 5.0)])],
        taps: vec![],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((result.switched_shunts[0] - 5.0).abs() < 1e-6);
    assert!(vm(&result.case, 9) < 1.01 - 1e-4);
}

#[test]
fn transformer_tap_moves_the_far_bus_into_band() {
    let case = case9();
    let before = vm(&solve(&case, &PowerFlowOptions::default()).unwrap().case, 4);
    assert!(before < 1.035, "{before}");
    // Branch 0 is the generator step-up from bus 1 to bus 4; a lower ratio
    // raises bus 4.
    let tap = TapControl {
        branch: 0,
        regulated_bus: 4,
        raises_voltage: false,
        vmax: 1.045,
        vmin: 1.035,
        ratio: 1.0,
        ratio_max: 1.1,
        ratio_min: 0.9,
        positions: 33,
        tap_per_ratio: 1.0,
        line: 0,
    };
    let controls = VoltageControls {
        switched_shunts: vec![],
        taps: vec![tap.clone()],
    };
    let result = solve(&case, &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let after = vm(&result.case, 4);
    assert!((1.035..=1.045).contains(&after), "{after}");
    let ratio = result.tap_ratios[0];
    assert!(ratio < 1.0);
    let steps = (ratio - tap.ratio_min) / tap.step();
    assert!((steps - steps.round()).abs() < 1e-9, "{ratio}");
    assert!((result.case.branches[0].tap - ratio).abs() < 1e-12);
}

#[test]
fn tap_stops_at_its_ratio_limit() {
    let tap = TapControl {
        branch: 0,
        regulated_bus: 4,
        raises_voltage: false,
        vmax: 1.2,
        vmin: 1.15,
        ratio: 1.0,
        ratio_max: 1.1,
        ratio_min: 0.95,
        positions: 13,
        tap_per_ratio: 1.0,
        line: 0,
    };
    let controls = VoltageControls {
        switched_shunts: vec![],
        taps: vec![tap],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((result.tap_ratios[0] - 0.95).abs() < 1e-12);
}

#[test]
fn without_controls_nothing_moves() {
    let result = solve(&case9(), &PowerFlowOptions::default()).unwrap();
    assert!(result.switched_shunts.is_empty());
    assert!(result.tap_ratios.is_empty());
    assert_eq!(result.voltage_control_passes, 0);
}

#[test]
fn raw_import_reads_switched_shunts_and_tap_controls() {
    let imported = rs_power::parse_raw_with_options(
        include_str!("fixtures/raw/t_psse_case3.raw"),
        &RawOptions {
            remote_regulation: true,
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let controls = &imported.voltage_controls;
    assert_eq!(controls.switched_shunts.len(), 5);
    let first = &controls.switched_shunts[0];
    assert_eq!(first.bus, 152);
    assert_eq!(first.mode, ShuntMode::Discrete);
    assert_eq!((first.vmin, first.vmax), (0.955, 1.045));
    assert_eq!(first.initial, -233.0);
    assert_eq!(first.blocks[0], (1, -15.0));
    assert_eq!(controls.taps.len(), 5);
    let tap = &controls.taps[0];
    assert_eq!(tap.regulated_bus, 101);
    assert!(tap.raises_voltage);
    assert_eq!(
        (tap.ratio_min, tap.ratio_max, tap.positions),
        (0.95, 1.05, 25)
    );
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        voltage_controls: controls.clone(),
        ..Default::default()
    };
    let result = solve(&imported.case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(result.switched_shunts.len(), 5);
    assert_eq!(result.tap_ratios.len(), 5);
}

#[test]
fn memphis_switched_shunts_reproduce_the_case_voltages() {
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
    let mean_error = |controls: VoltageControls| {
        let opt = PowerFlowOptions {
            enforce_q_limits: true,
            voltage_controls: controls,
            ..Default::default()
        };
        let result = solve(&imported.case, &opt).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        (result.case.buses.iter().zip(&imported.case.buses))
            .map(|(a, b)| (a.vm - b.vm).abs())
            .sum::<f64>()
            / imported.case.buses.len() as f64
    };
    let (frozen, switched) = (
        mean_error(VoltageControls::default()),
        mean_error(imported.voltage_controls.clone()),
    );
    assert!(frozen > 0.001, "{frozen}");
    assert!(switched < 0.0002, "{switched}");
}

#[test]
fn shunt_with_a_band_narrower_than_a_step_settles_nearest_the_band() {
    // Bus 5 sits near 0.975 pu with nothing switched in, and one 60 Mvar block
    // lifts it by about 0.03 pu, so a zero-width band (VSWLO = VSWHI) between
    // the two can never be met. The shunt tries a step, sees that it jumped
    // over the band, and holds whichever level lands nearer.
    let case = case9();
    let off = vm(&solve(&case, &PowerFlowOptions::default()).unwrap().case, 5);
    let mut on_case = case.clone();
    on_case.buses.iter_mut().find(|b| b.id == 5).unwrap().bs += 60.0;
    let on = vm(
        &solve(&on_case, &PowerFlowOptions::default()).unwrap().case,
        5,
    );
    assert!(on - off > 0.01, "{off} {on}");
    for (target, expected) in [
        (off + 0.3 * (on - off), 0.0),
        (off + 0.7 * (on - off), 60.0),
    ] {
        for initial in [0.0, 60.0] {
            let mut s = shunt(5, ShuntMode::Discrete, (target, target), vec![(1, 60.0)]);
            s.initial = initial;
            let mut start = case.clone();
            start.buses.iter_mut().find(|b| b.id == 5).unwrap().bs += initial;
            let controls = VoltageControls {
                switched_shunts: vec![s],
                taps: vec![],
            };
            let result = solve(&start, &with(controls)).unwrap();
            assert_eq!(result.status, SolveStatus::Converged);
            assert_eq!(
                result.switched_shunts[0], expected,
                "target {target}, start {initial}"
            );
            // At most a trial step and a step back, never hunting until held.
            assert!(
                result.voltage_control_passes <= 2,
                "target {target}, start {initial}: {} passes",
                result.voltage_control_passes
            );
        }
    }
}

#[test]
fn psse_case3_zero_width_band_shunt_keeps_its_starting_block() {
    // The shunt at bus 93002 has VSWLO = VSWHI = 0.99628 and one 1.44 Mvar
    // block. It used to toggle on and off until held, ending off; the block
    // in (as in the case) is nearer the setpoint.
    let imported = rs_power::parse_raw_with_options(
        include_str!("fixtures/raw/t_psse_case3.raw"),
        &RawOptions {
            remote_regulation: true,
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let controls = imported.voltage_controls.clone();
    let k = controls
        .switched_shunts
        .iter()
        .position(|s| s.bus == 93002)
        .unwrap();
    assert_eq!(
        controls.switched_shunts[k].vmin,
        controls.switched_shunts[k].vmax
    );
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        voltage_controls: controls,
        ..Default::default()
    };
    let result = solve(&imported.case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(result.switched_shunts[k], 1.44);
    assert!(
        result.voltage_control_passes <= 2,
        "{}",
        result.voltage_control_passes
    );
}

#[test]
fn locked_shunts_and_taps_stay_at_their_starting_values() {
    let case = case9();
    let mut shunt = shunt(5, ShuntMode::Discrete, (1.03, 1.04), vec![(8, 10.0)]);
    shunt.initial = 20.0;
    let tap = TapControl {
        branch: 0,
        regulated_bus: 4,
        raises_voltage: false,
        vmax: 1.045,
        vmin: 1.035,
        ratio: 1.0125,
        ratio_max: 1.1,
        ratio_min: 0.9,
        positions: 33,
        tap_per_ratio: 1.0,
        line: 0,
    };
    let mut started = case.clone();
    started.branches[0].tap = tap.ratio;
    started.buses[4].bs += shunt.initial;
    let controls = VoltageControls {
        switched_shunts: vec![shunt.clone()],
        taps: vec![tap.clone()],
    };
    let free = solve(&started, &with(controls.clone())).unwrap();
    assert!(free.voltage_control_passes > 0);
    assert_ne!(free.switched_shunts[0], shunt.initial);
    assert_ne!(free.tap_ratios[0], tap.ratio);
    let locked = solve(
        &started,
        &PowerFlowOptions {
            lock_switched_shunts: true,
            lock_taps: true,
            ..with(controls.clone())
        },
    )
    .unwrap();
    assert_eq!(locked.status, SolveStatus::Converged);
    assert_eq!(locked.voltage_control_passes, 0);
    // Still listed, at their starting values, as in the solved case.
    assert_eq!(locked.switched_shunts, [shunt.initial]);
    assert_eq!(locked.tap_ratios, [tap.ratio]);
    assert_eq!(locked.case.branches[0].tap, tap.ratio);
    assert_eq!(locked.case.buses[4].bs, started.buses[4].bs);
    // Locking one leaves the other free.
    let taps_only = solve(
        &started,
        &PowerFlowOptions {
            lock_taps: true,
            ..with(controls)
        },
    )
    .unwrap();
    assert_eq!(taps_only.tap_ratios, [tap.ratio]);
    assert_ne!(taps_only.switched_shunts[0], shunt.initial);
}
