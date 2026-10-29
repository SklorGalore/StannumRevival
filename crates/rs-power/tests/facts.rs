//! FACTS devices: shunt elements holding a voltage within a current limit,
//! series elements sending a set power, and switched shunts watching them.
use rs_power::{
    Case, DcModel, FactsDevice, Model, PowerFlowOptions, RawImport, RawOptions, ShuntMode,
    SolveStatus, SwitchedShunt, VoltageControls, parse_matpower, parse_raw_with_options, solve,
};

fn case9() -> Case {
    parse_matpower(include_str!("fixtures/case9.m")).unwrap()
}

fn vm(case: &Case, id: u32) -> f64 {
    case.buses.iter().find(|b| b.id == id).unwrap().vm
}

fn statcom(bus: u32, vset: f64, shunt_max: f64) -> FactsDevice {
    FactsDevice {
        name: "SC1".into(),
        bus,
        terminal_bus: None,
        regulated_bus: None,
        vset,
        shunt_max,
        p_desired: 0.0,
        q_desired: 0.0,
        reactive_share: 100.0,
        initial_mvar: 0.0,
        line: 0,
    }
}

fn with(facts: Vec<FactsDevice>, shunts: Vec<SwitchedShunt>) -> PowerFlowOptions {
    PowerFlowOptions {
        voltage_controls: VoltageControls {
            switched_shunts: shunts,
            taps: vec![],
            facts,
        },
        ..Default::default()
    }
}

fn psse_case3(text: &str) -> RawImport {
    parse_raw_with_options(
        text,
        &RawOptions {
            remote_regulation: true,
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            zip_loads: true,
            ..Default::default()
        },
    )
    .unwrap()
}

const CASE3: &str = include_str!("fixtures/raw/t_psse_case3.raw");

#[test]
fn statcom_holds_its_bus_at_the_setpoint() {
    let case = case9();
    let before = vm(&solve(&case, &PowerFlowOptions::default()).unwrap().case, 5);
    assert!(before < 1.02, "{before}");
    let result = solve(&case, &with(vec![statcom(5, 1.04, 100.0)], vec![])).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((vm(&result.case, 5) - 1.04).abs() < 1e-8);
    let s = &result.facts[0];
    assert!(s.shunt_mvar > 0.0 && !s.at_limit && !s.held, "{s:?}");
    // The helper generator is gone and the bus keeps its own data.
    assert_eq!(result.case.generators.len(), case.generators.len());
    assert_eq!(result.case.buses[4].bs, case.buses[4].bs);
}

#[test]
fn statcom_stops_at_its_current_limit() {
    let case = case9();
    let free = solve(&case, &with(vec![statcom(5, 1.04, 100.0)], vec![])).unwrap();
    let needed = free.facts[0].shunt_mvar;
    // SHMX is a current at 1 pu: the range at VSET 1.04 is ±10.4 Mvar.
    let result = solve(&case, &with(vec![statcom(5, 1.04, 10.0)], vec![])).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let s = &result.facts[0];
    assert!(needed > 10.4, "{needed}");
    assert!(s.at_limit && (s.shunt_mvar - 10.4).abs() < 1e-6, "{s:?}");
    assert!(vm(&result.case, 5) < 1.04);
}

#[test]
fn statcom_at_a_generator_bus_is_held() {
    let result = solve(&case9(), &with(vec![statcom(2, 1.1, 100.0)], vec![])).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!(result.facts[0].held);
    assert_eq!(result.facts[0].shunt_mvar, 0.0);
    assert!((vm(&result.case, 2) - 1.025).abs() < 1e-8);
}

#[test]
fn series_element_sends_its_set_power_in_ac_and_dc() {
    let case = case9();
    // Send 40 MW and 10 Mvar from bus 4 to bus 9 through a UPFC.
    let mut upfc = statcom(4, 1.0, 50.0);
    upfc.terminal_bus = Some(9);
    (upfc.p_desired, upfc.q_desired) = (40.0, 10.0);
    for model in [Model::Ac, Model::Dc] {
        let mut opt = with(vec![upfc.clone()], vec![]);
        opt.model = model;
        let plain = solve(
            &case,
            &PowerFlowOptions {
                model,
                ..Default::default()
            },
        )
        .unwrap();
        let result = solve(&case, &opt).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        // Bus 9's feeds from 8 and 6 carry 40 MW less into it.
        let into_9 = |flows: &[[f64; 4]]| {
            case.branches
                .iter()
                .zip(flows)
                .map(|(b, f)| {
                    if b.to == 9 {
                        -f[2]
                    } else if b.from == 9 {
                        -f[0]
                    } else {
                        0.0
                    }
                })
                .sum::<f64>()
        };
        let drop = into_9(&plain.branch_flows) - into_9(&result.branch_flows);
        assert!((drop - 40.0).abs() < 1e-6, "{model:?}: {drop}");
        assert_eq!(result.facts[0].series_mw, 40.0);
        // Loads are as given.
        assert_eq!(result.case.buses[3].pd, case.buses[3].pd);
        assert_eq!(result.case.buses[8].pd, case.buses[8].pd);
        if model == Model::Dc {
            assert!(result.facts[0].held);
        }
    }
}

#[test]
fn shunt_keeps_a_statcom_inside_its_band() {
    // Holding bus 5 at 1.04 pu takes more than 10.4 Mvar of the STATCOM's
    // ±52 Mvar. A band of 0.4-0.6 of that range is -10.4 to 10.4 Mvar, so
    // capacitors at bus 5 switch in and take output off the STATCOM.
    let case = case9();
    let shunt = SwitchedShunt {
        bus: 5,
        mode: ShuntMode::FactsReactive { device: 0 },
        regulated_bus: None,
        vmin: 0.4,
        vmax: 0.6,
        initial: 0.0,
        blocks: vec![(10, 5.0)],
        line: 0,
    };
    let alone = solve(&case, &with(vec![statcom(5, 1.04, 50.0)], vec![])).unwrap();
    assert!(alone.facts[0].shunt_mvar > 10.4, "{:?}", alone.facts);
    let result = solve(
        &case,
        &with(vec![statcom(5, 1.04, 50.0)], vec![shunt.clone()]),
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let q = result.facts[0].shunt_mvar;
    assert!((-10.4..=10.4).contains(&q), "{q}");
    assert!(result.switched_shunts[0] > 0.0);
    // Locked shunts leave the STATCOM alone.
    let mut locked = with(vec![statcom(5, 1.04, 50.0)], vec![shunt]);
    locked.lock_switched_shunts = true;
    let held = solve(&case, &locked).unwrap();
    assert_eq!(held.switched_shunts[0], 0.0);
}

#[test]
fn raw_import_reads_the_psse_sample_facts_devices() {
    let imported = psse_case3(CASE3);
    let facts = &imported.voltage_controls.facts;
    assert_eq!(facts.len(), 2);
    assert_eq!(
        (facts[0].name.as_str(), facts[0].bus, facts[0].terminal_bus),
        ("1", 153, None)
    );
    assert_eq!((facts[0].vset, facts[0].shunt_max), (1.015, 50.0));
    assert_eq!((facts[1].bus, facts[1].terminal_bus), (153, Some(155)));
    assert_eq!((facts[1].p_desired, facts[1].q_desired), (350.0, 40.0));
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.line == facts[1].line && w.message.contains("series element"))
    );
    assert!(
        !imported
            .warnings
            .iter()
            .any(|w| w.message.contains("records of FACTS"))
    );
}

#[test]
fn psse_sample_facts_devices_reproduce_the_case_solution() {
    let imported = psse_case3(CASE3);
    let error = |facts: bool| {
        let mut controls = imported.voltage_controls.clone();
        if !facts {
            controls.facts.clear();
        }
        let opt = PowerFlowOptions {
            enforce_q_limits: true,
            voltage_controls: controls,
            dc_model: DcModel::Detailed {
                lines: imported.lcc_lines.clone(),
                vsc_lines: imported.vsc_lines.clone(),
                mtdc: imported.mtdc.clone(),
                adjust_taps: true,
            },
            ..Default::default()
        };
        let result = solve(&imported.case, &opt).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        let pairs = || result.case.buses.iter().zip(&imported.case.buses);
        let mean_vm =
            pairs().map(|(a, b)| (a.vm - b.vm).abs()).sum::<f64>() / pairs().count() as f64;
        let max_va = pairs()
            .map(|(a, b)| (a.va - b.va).abs())
            .fold(0.0, f64::max);
        (mean_vm, max_va, result)
    };
    let (mean_without, va_without, _) = error(false);
    let (mean_with, va_with, result) = error(true);
    // Without the UPFC's 350 MW, bus 155 is 10.8 degrees off.
    assert!(
        mean_without > 0.002 && va_without > 10.0,
        "{mean_without} {va_without}"
    );
    assert!(mean_with < 0.0015 && va_with < 1.0, "{mean_with} {va_with}");
    assert!((vm(&result.case, 155) - 1.01723).abs() < 0.001);
    // Bus 153 sits above VSET in the case: both shunt elements are at their limits.
    assert!(
        result
            .facts
            .iter()
            .all(|s| s.at_limit && s.shunt_mvar < 0.0)
    );
}

#[test]
fn raw_import_resolves_modsw_6_by_facts_device_name() {
    // Make the shunt at bus 93002 watch FACTS device 1 (RMIDNT '1').
    let text = CASE3.replace(
        " 93002,1,0.99628,0.99628,     0,  100.00,'            ',",
        " 93002,6,0.80000,0.20000,   153,  100.00,'1           ',",
    );
    let imported = psse_case3(&text);
    let shunt = imported
        .voltage_controls
        .switched_shunts
        .iter()
        .find(|s| s.bus == 93002)
        .unwrap();
    assert_eq!(shunt.mode, ShuntMode::FactsReactive { device: 0 });
    assert_eq!((shunt.vmin, shunt.vmax), (0.2, 0.8));
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        voltage_controls: imported.voltage_controls.clone(),
        ..Default::default()
    };
    assert_eq!(
        solve(&imported.case, &opt).unwrap().status,
        SolveStatus::Converged
    );
}
