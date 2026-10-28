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
        facts: vec![],
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
        facts: vec![],
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
        facts: vec![],
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
        facts: vec![],
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
        facts: vec![],
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
        facts: vec![],
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
    assert_eq!(controls.switched_shunts.len(), 6);
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
    assert_eq!(result.switched_shunts.len(), 6);
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
                facts: vec![],
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
        facts: vec![],
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

#[test]
fn psse_sample_shunt_watches_its_vsc_converter() {
    // Bus 3005's shunt in the PSS/E sample case is MODSW 4: one 33.35 Mvar
    // block keeps the VDCLINE1 converter at 3005 within 0.64-0.98 of its
    // -110 to 100 Mvar range, 24.4 to 95.8 Mvar.
    // The case runs that converter at a fixed power factor, where no shunt
    // can move its output; put it in AC voltage control (MODE 1) at 1.02 pu.
    let text = include_str!("fixtures/raw/t_psse_case3.raw").replace(
        "  3005,2,2,   -209.00, 0.95000,",
        "  3005,2,1,   -209.00, 1.02000,",
    );
    let imported = rs_power::parse_raw_with_options(
        &text,
        &RawOptions {
            remote_regulation: true,
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    // The FACTS devices at bus 153 sit at their limits and leave the
    // converter at its own limit at any setpoint near 1.02 pu, where no
    // shunt moves it either; this test is about the converter's band.
    let mut controls = imported.voltage_controls.clone();
    controls.facts.clear();
    let controls = &controls;
    let k = controls
        .switched_shunts
        .iter()
        .position(|s| s.bus == 3005)
        .unwrap();
    let ShuntMode::ConverterReactive { row, bus } = controls.switched_shunts[k].mode else {
        panic!("{:?}", controls.switched_shunts[k].mode);
    };
    assert_eq!(bus, 3005);
    assert_eq!(imported.dc_line_names[row], "VDCLINE1");
    let converter_q = |controls: &VoltageControls| {
        let opt = PowerFlowOptions {
            enforce_q_limits: true,
            voltage_controls: controls.clone(),
            dc_model: rs_power::DcModel::Detailed {
                lines: imported.lcc_lines.clone(),
                vsc_lines: imported.vsc_lines.clone(),
                mtdc: imported.mtdc.clone(),
                adjust_taps: true,
            },
            ..Default::default()
        };
        let result = solve(&imported.case, &opt).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        let vsc = result.vsc.iter().find(|v| v.row == row).unwrap();
        let q = if result.case.dcline.as_ref().unwrap()[row][0] == 3005.0 {
            vsc.from_mvar
        } else {
            vsc.to_mvar
        };
        (q, result.switched_shunts[k])
    };
    // As given, the converter is inside its band and the block stays out.
    let (q, b) = converter_q(controls);
    assert!((24.4..=95.8).contains(&q), "{q}");
    assert_eq!(b, 0.0);
    // Narrow the band to 0.4-0.6 of the range, -26 to 16 Mvar: the block
    // switches in and takes reactive output off the converter.
    let mut narrow = controls.clone();
    (
        narrow.switched_shunts[k].vmin,
        narrow.switched_shunts[k].vmax,
    ) = (0.4, 0.6);
    let (q2, b2) = converter_q(&narrow);
    assert_eq!(b2, 33.35);
    assert!((-26.0..=16.0).contains(&q2), "{q} -> {q2}");
}

fn gen_q(case: &Case, bus: u32) -> f64 {
    case.generators
        .iter()
        .filter(|g| g.bus == bus)
        .map(|g| g.qg)
        .sum()
}

#[test]
fn shunt_keeps_a_plant_inside_its_reactive_band() {
    // Generator 2 of case 9 makes about 7 Mvar of its -300 to 300. A band of
    // 0.44-0.46 of that range is -36 to -24 Mvar, so capacitors at bus 8
    // switch in until the plant absorbs that much.
    let mut s = shunt(
        8,
        ShuntMode::PlantReactive { bus: 2 },
        (0.44, 0.46),
        vec![(20, 10.0)],
    );
    let controls = VoltageControls {
        facts: vec![],
        switched_shunts: vec![s.clone()],
        taps: vec![],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let q = gen_q(&result.case, 2);
    assert!((-36.0..=-24.0).contains(&q), "{q}");
    assert!(result.switched_shunts[0] > 0.0);
    // Reactors bring it the other way, into 0.52-0.54 (12 to 24 Mvar).
    s.blocks = vec![(20, -10.0)];
    (s.vmin, s.vmax) = (0.52, 0.54);
    let controls = VoltageControls {
        facts: vec![],
        switched_shunts: vec![s],
        taps: vec![],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let q = gen_q(&result.case, 2);
    assert!((12.0..=24.0).contains(&q), "{q}");
    assert!(result.switched_shunts[0] < 0.0);
}

#[test]
fn plant_watch_with_no_generator_holds_the_shunt() {
    let controls = VoltageControls {
        facts: vec![],
        switched_shunts: vec![shunt(
            8,
            ShuntMode::PlantReactive { bus: 5 },
            (0.4, 0.6),
            vec![(8, 10.0)],
        )],
        taps: vec![],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(result.switched_shunts[0], 0.0);
}

#[test]
fn shunt_relieves_a_continuous_shunt_it_watches() {
    // A continuous shunt at bus 9 holds 1.03 pu with part of its 0-200 Mvar.
    let continuous = shunt(9, ShuntMode::Continuous, (1.02, 1.04), vec![(1, 200.0)]);
    let alone = solve(
        &case9(),
        &with(VoltageControls {
            facts: vec![],
            switched_shunts: vec![continuous.clone()],
            taps: vec![],
        }),
    )
    .unwrap();
    let used = alone.switched_shunts[0];
    assert!(used > 30.0, "{used}");
    // A MODSW 5 bank at bus 4 keeps it within 0-0.1 of its range (0 to 20
    // Mvar) by switching 10 Mvar steps in nearby.
    let controls = VoltageControls {
        facts: vec![],
        switched_shunts: vec![
            continuous,
            shunt(
                4,
                ShuntMode::ShuntSetting { shunt: 0 },
                (0.0, 0.1),
                vec![(20, 10.0)],
            ),
        ],
        taps: vec![],
    };
    let result = solve(&case9(), &with(controls)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let [watched, bank] = result.switched_shunts[..] else {
        panic!()
    };
    assert!(watched <= 20.0 + 0.01, "{watched}");
    assert!(bank > 0.0, "{bank}");
    assert!((vm(&result.case, 9) - 1.03).abs() < 1e-6);
}

#[test]
fn a_shunt_cannot_watch_itself() {
    let controls = VoltageControls {
        facts: vec![],
        switched_shunts: vec![shunt(
            4,
            ShuntMode::ShuntSetting { shunt: 0 },
            (0.0, 0.1),
            vec![(2, 10.0)],
        )],
        taps: vec![],
    };
    assert!(solve(&case9(), &with(controls)).is_err());
}

#[test]
fn raw_import_reads_reactive_switched_shunt_modes() {
    // Rework the PSS/E sample case's shunts: 154 watches the plant at 301
    // (MODSW 3), 152 the continuous shunt at 3021 (MODSW 5), and 93002 a
    // FACTS device (MODSW 6), which stays frozen.
    let text = include_str!("fixtures/raw/t_psse_case3.raw")
        .replace(
            "   154,1,1.04480,0.96500,     0,",
            "   154,3,0.90000,0.75000,   301,",
        )
        .replace(
            "   152,1,1.04500,0.95500,     0,",
            "   152,5,0.50000,0.10000,  3021,",
        )
        .replace(
            " 93002,1,0.99628,0.99628,     0,  100.00,'            ',",
            " 93002,6,0.80000,0.20000,     0,  100.00,'FACTS 1     ',",
        );
    let imported = rs_power::parse_raw_with_options(
        &text,
        &RawOptions {
            remote_regulation: true,
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let shunts = &imported.voltage_controls.switched_shunts;
    let at = |bus: u32| shunts.iter().position(|s| s.bus == bus);
    assert_eq!(shunts.len(), 5);
    let plant = &shunts[at(154).unwrap()];
    assert_eq!(plant.mode, ShuntMode::PlantReactive { bus: 301 });
    assert_eq!((plant.vmin, plant.vmax), (0.75, 0.9));
    assert_eq!(
        shunts[at(152).unwrap()].mode,
        ShuntMode::ShuntSetting {
            shunt: at(3021).unwrap()
        }
    );
    assert_eq!(at(93002), None);
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.message.contains("watches FACTS device FACTS 1"))
    );
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        voltage_controls: imported.voltage_controls.clone(),
        ..Default::default()
    };
    let result = solve(&imported.case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    // The plant at 301 is below its band, but bus 154's blocks barely move
    // it, so the one block tried goes back in and the shunt holds.
    assert_eq!(result.switched_shunts[at(154).unwrap()], 124.0);
}

#[test]
fn raw_import_freezes_shunts_whose_watched_device_is_missing() {
    // MODSW 5 at a bus with no other shunt in control, and MODSW 4 naming a
    // VSC line that is not in the case.
    let text = include_str!("fixtures/raw/t_psse_case3.raw")
        .replace(
            "   152,1,1.04500,0.95500,     0,",
            "   152,5,0.50000,0.10000,   101,",
        )
        .replace(
            "'VDCLINE1    ',    0.00, 1,  33.35",
            "'NO SUCH     ',    0.00, 1,  33.35",
        );
    let imported = rs_power::parse_raw_with_options(
        &text,
        &RawOptions {
            all_dc_lines: true,
            ..Default::default()
        },
    )
    .unwrap();
    let shunts = &imported.voltage_controls.switched_shunts;
    assert!(shunts.iter().all(|s| s.bus != 152 && s.bus != 3005));
    let frozen = imported
        .warnings
        .iter()
        .filter(|w| {
            w.message
                .starts_with("switched shunt frozen at BINIT; it watches")
        })
        .count();
    assert_eq!(frozen, 2);
}
