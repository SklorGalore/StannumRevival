//! Shared element ids agree between rs-power rows and nodal equipment.
use stannum_core::Workspace;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn open(path: PathBuf) -> Workspace {
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    ws
}

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../nodal")
        .join(name)
}

/// Every rs-power branch and generator names a nodal element, and every
/// nodal line and transformer is found among the rs-power rows.
fn check(path: PathBuf) -> (usize, usize) {
    let ws = open(path);
    let case = ws.case.as_ref().unwrap();
    let power = case.power.as_ref().unwrap();
    let topo = case.topology().as_ref().unwrap();

    let equipment: HashSet<&str> = topo.case.equipment.iter().map(|e| e.id.as_str()).collect();
    let generators: HashSet<&str> = topo.case.generators.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(power.elements.branches.len(), power.case.branches.len());
    assert_eq!(power.elements.generators.len(), power.case.generators.len());

    let mut rows: HashMap<&str, usize> = HashMap::new();
    for b in &power.elements.branches {
        assert!(
            equipment.contains(b.element.as_str()),
            "{} not in nodal",
            b.element
        );
        *rows.entry(&b.element).or_default() += 1;
    }
    for g in &power.elements.generators {
        assert!(
            generators.contains(g.element.as_str()),
            "{} not in nodal",
            g.element
        );
    }
    for e in &topo.case.equipment {
        let expected = match e.kind {
            _ if e.buses.len() == 3 => 3,
            _ => 1,
        };
        assert_eq!(rows.get(e.id.as_str()).copied(), Some(expected), "{}", e.id);
    }
    let parallel = power
        .elements
        .branches
        .iter()
        .filter(|b| b.circuit != "1" && b.winding == 0)
        .count();
    let three_winding = power
        .elements
        .branches
        .iter()
        .filter(|b| b.winding > 0)
        .count();
    (parallel, three_winding)
}

#[test]
fn texas2k_ids_match_nodal() {
    // Three-winding legs are covered by rs-power's RAW tests; this sample has none.
    let (parallel, _) = check(sample("Texas2k_series25_case1_summerpeak.RAW"));
    assert!(parallel > 0, "sample should exercise circuit ids");
}

#[test]
fn memphis_ids_match_nodal() {
    check(sample("MemphisCase2026_Mar7.RAW"));
}

#[test]
fn matpower_parallel_branches_are_numbered() {
    let ws = open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/case9.m"));
    let power = ws.case.as_ref().unwrap().power.as_ref().unwrap();
    let ids: HashSet<&str> = power
        .elements
        .branches
        .iter()
        .map(|b| b.element.as_str())
        .collect();
    assert_eq!(ids.len(), power.case.branches.len());
    assert!(ids.contains("line:1:4:1"));
    assert_eq!(power.elements.generators[0].element, "generator:1:1");
}

/// Case diff records name the same elements as nodal, so a selection can
/// jump between Topology and Case diff.
#[test]
fn diff_records_match_nodal_ids() {
    let path = sample("MemphisCase2026_Mar7.RAW");
    let mut ws = open(path.clone());
    let topo = &ws.case.as_ref().unwrap().topology().as_ref().unwrap().case;
    let mut known: HashSet<String> = topo
        .buses
        .iter()
        .map(|b| format!("bus:{}", b.number))
        .collect();
    known.extend(topo.equipment.iter().map(|e| e.id.clone()));
    known.extend(topo.loads.iter().map(|e| e.id.clone()));
    known.extend(topo.generators.iter().map(|e| e.id.clone()));
    ws.compare(&path, &path).unwrap();
    let cmp = ws.comparison().unwrap();
    let mut mapped = 0;
    for section in [
        "BUS",
        "LOAD",
        "GENERATOR",
        "BRANCH",
        "TRANSFORMER",
        "SYSTEM SWITCHING DEVICE",
    ] {
        for row in cmp.section_records(section) {
            let element = row.element.expect("record has an element id");
            assert!(
                known.contains(&element),
                "{section}: {element} not in nodal"
            );
            mapped += 1;
        }
    }
    assert_eq!(mapped, known.len());
    assert!(
        cmp.section_records("AREA")
            .iter()
            .all(|r| r.element.is_none())
    );
}

fn rs_power_raw(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs-power/tests/fixtures/raw")
        .join(name)
}

#[test]
fn dc_lines_reach_the_diagram_network() {
    let ws = open(rs_power_raw("t_psse_case3.raw"));
    let net = ws.current().unwrap().diagram_network().unwrap();
    let lines: Vec<_> = net
        .dc_lines
        .iter()
        .map(|d| (d.element.as_str(), d.kind, d.from, d.to, d.online))
        .collect();
    assert_eq!(
        lines,
        [
            ("dcline:301:3021:1", "two_terminal", 301, 3021, true),
            ("dcline:301:3022:2", "two_terminal", 301, 3022, true),
            ("dcline:3005:3008:VDCLINE1", "vsc", 3005, 3008, true),
            ("dcline:203:205:VDCLINE2", "vsc", 203, 205, true),
            ("dcline:212:402:1", "multi_terminal", 402, 212, true),
            ("dcline:212:401:1", "multi_terminal", 401, 212, true),
            ("dcline:212:213:1", "multi_terminal", 212, 213, true),
        ]
    );
    assert!(net.dc_lines.iter().all(|d| d.mw > 0.0));

    // Names keep their spaces, and MATPOWER-style numbering is not used for RAW.
    let ws = open(rs_power_raw("dc_modes.raw"));
    let net = ws.current().unwrap().diagram_network().unwrap();
    let names: Vec<_> = net.dc_lines.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["DC 0", "DC 1", "DC 2"]);
    assert!(!net.dc_lines[0].online, "mode 0 is blocked");
}

#[test]
fn solve_report_lists_dc_lines_with_absorbing_converters() {
    let mut ws = open(rs_power_raw("t_psse_case3.raw"));
    for enforce_q_limits in [false, true] {
        let report = ws
            .current_mut()
            .unwrap()
            .solve(stannum_core::power::SolveSettings {
                enforce_q_limits,
                // Each pass solves from the file, so the taps move again.
                start: stannum_core::power::StartChoice::Case,
                ..Default::default()
            })
            .unwrap();
        assert!(report.converged, "Q limits {enforce_q_limits}");
        let dc = &report.dc_lines;
        assert_eq!(dc.len(), 7);
        assert_eq!(dc[0].element, "dcline:301:3021:1");
        assert_eq!(
            (dc[0].kind, dc[0].from, dc[0].to),
            ("two_terminal", 301, 3021)
        );
        assert_eq!(dc[2].kind, "vsc");
        // Line-commutated converters absorb within negative ranges.
        assert!(dc[0].qmaxf <= 0.0 && dc[0].qf < 0.0);
        assert!(dc.iter().all(|d| d.online));
        // Two-terminal lines use the detailed model by default: they lose
        // I²R and report their converter state. The others stay lossless.
        for d in &dc[..2] {
            let detail = d.detail.as_ref().unwrap();
            assert_eq!(detail.control, "rectifier");
            assert!(detail.current_ka > 2.0 && detail.alpha > 0.0);
            assert!((d.loss_mw - detail.line_loss_mw - detail.converter_loss_mw).abs() < 1e-6);
            assert!(d.loss_mw > 50.0 && d.qt < 0.0);
        }
        // VSC lines lose converter and I²R power.
        for d in &dc[2..4] {
            let v = d.vsc_detail.as_ref().unwrap();
            let losses = v.from_loss_mw + v.to_loss_mw + v.line_loss_mw;
            assert!(d.detail.is_none() && (d.loss_mw - losses).abs() < 1e-6 && losses > 0.5);
        }
        // The multi-terminal rows share the system's DC network losses.
        let [system] = &report.dc_systems[..] else {
            panic!()
        };
        assert_eq!((system.buses.len(), system.links.len()), (5, 4));
        let mt_loss: f64 = dc[4..].iter().map(|d| d.loss_mw).sum();
        assert!((mt_loss - system.line_loss_mw - system.converter_loss_mw).abs() < 1e-6);
        assert!(mt_loss > 20.0, "{mt_loss}");
        for d in &dc[4..] {
            let m = d.mtdc_detail.as_ref().unwrap();
            assert_eq!(m.control, "order");
            assert_eq!(m.at_from, m.rectifier);
            assert!(d.vsc_detail.is_none() && d.detail.is_none());
        }
        let held = system.converters.iter().find(|c| c.bus == 212).unwrap();
        assert_eq!(held.control, "voltage");
        assert!((held.dc_kv - 500.0).abs() < 1e-9);
        if enforce_q_limits {
            assert!(dc.iter().all(|d| d.q_violation < 1e-3));
        }
        assert!(report.dc_tap_passes > 0);
    }
    // The simplified model keeps MATPOWER's lossless rows.
    let report = ws
        .current_mut()
        .unwrap()
        .solve(stannum_core::power::SolveSettings {
            dc_model: stannum_core::power::DcModelChoice::Simplified,
            ..Default::default()
        })
        .unwrap();
    assert!(report.dc_lines.iter().all(|d| d.detail.is_none()
        && d.vsc_detail.is_none()
        && d.mtdc_detail.is_none()
        && d.loss_mw.abs() < 1e-9));
    assert!(report.dc_systems.is_empty());
}

#[test]
fn area_interchange_setting_holds_the_raw_areas() {
    use stannum_core::power::{InterchangeChoice, SolveSettings};
    let mut ws = open(rs_power_raw("t_psse_case3.raw"));
    let mut solve = |area_interchange| {
        ws.current_mut()
            .unwrap()
            .solve(SolveSettings {
                enforce_q_limits: true,
                area_interchange,
                ..Default::default()
            })
            .unwrap()
    };
    let off = solve(InterchangeChoice::Off);
    assert!(off.converged);
    assert_eq!(off.area_interchange, InterchangeChoice::Off);
    assert_eq!(off.areas.len(), 6);
    assert_eq!(off.areas[0].name, "CENTRAL");
    assert!(off.areas.iter().all(|a| !a.controlled));
    // EAST is 260 MW short of its PDES with control off.
    let east = &off.areas[1];
    assert!(east.error_mw.unwrap().abs() > 100.0, "{east:?}");
    let on = solve(InterchangeChoice::TieLines);
    assert!(on.converged);
    assert!(on.interchange_passes > 0);
    for a in on.areas.iter().filter(|a| a.number <= 2) {
        assert!(a.controlled, "{a:?}");
        assert!(a.error_mw.unwrap().abs() <= a.tolerance_mw, "{a:?}");
        assert!(a.slack_change_mw != 0.0);
    }
    assert_eq!(on.areas[1].slack_bus_name, "URBGEN");
}

#[test]
fn solve_settings_switch_voltage_controls() {
    let mut ws = open(rs_power_raw("t_psse_case3.raw"));
    let mut solve = |switch_shunts: bool, adjust_transformer_taps: bool| {
        ws.current_mut()
            .unwrap()
            .solve(stannum_core::power::SolveSettings {
                enforce_q_limits: true,
                switch_shunts,
                adjust_transformer_taps,
                ..Default::default()
            })
            .unwrap()
    };
    let off = solve(false, false);
    assert!(off.converged);
    assert_eq!(off.voltage_control_passes, 0);
    assert_eq!((off.shunts_switched, off.taps_moved), (0, 0));
    let on = solve(true, true);
    assert!(on.converged);
    assert!(on.voltage_control_passes > 0);
    assert!(on.shunts_switched + on.taps_moved > 0);
    // Locked after that solve, they stay where it left them rather than
    // going back to their case values.
    let locked = solve(false, false);
    assert!(locked.converged);
    assert_eq!(locked.voltage_control_passes, 0);
    assert_eq!(
        (locked.shunts_switched, locked.taps_moved),
        (on.shunts_switched, on.taps_moved)
    );
    let worst = on
        .buses
        .iter()
        .zip(locked.buses.iter())
        .map(|(a, b)| (a.vm - b.vm).abs())
        .fold(0.0, f64::max);
    assert!(worst < 1e-3, "{worst}");
    // From a flat start they are locked at their case values.
    let flat = ws
        .current_mut()
        .unwrap()
        .solve(stannum_core::power::SolveSettings {
            enforce_q_limits: true,
            switch_shunts: false,
            adjust_transformer_taps: false,
            start: stannum_core::power::StartChoice::Flat,
            ..Default::default()
        })
        .unwrap();
    assert!(flat.converged);
    assert_eq!((flat.shunts_switched, flat.taps_moved), (0, 0));
}

#[test]
fn every_solve_method_reaches_the_same_solution() {
    use stannum_core::power::{MethodChoice, SolveSettings, StartChoice};
    let mut ws = open(rs_power_raw("t_psse_case3.raw"));
    let mut solve = |method: MethodChoice| {
        ws.current_mut()
            .unwrap()
            .solve(SolveSettings {
                method,
                start: StartChoice::Case,
                tolerance_mva: 1e-4,
                enforce_q_limits: true,
                ..Default::default()
            })
            .unwrap()
    };
    let newton = solve(MethodChoice::Newton);
    assert!(newton.converged);
    assert_eq!(newton.method, MethodChoice::Newton);
    assert_eq!(newton.decoupled_iterations, 0);
    for method in [
        MethodChoice::FixedSlope,
        MethodChoice::FastDecoupledXb,
        MethodChoice::FastDecoupledBx,
        MethodChoice::Auto,
    ] {
        let report = solve(method);
        assert!(report.converged, "{method:?}");
        assert_eq!(report.method, method);
        assert!(report.decoupled_iterations > 0, "{method:?}");
        assert!(
            (report.total_losses_mw - newton.total_losses_mw).abs() < 1e-3,
            "{method:?}"
        );
    }
}

#[test]
fn solve_report_lists_facts_devices() {
    let mut ws = open(rs_power_raw("t_psse_case3.raw"));
    let mut solve = |model| {
        ws.current_mut()
            .unwrap()
            .solve(stannum_core::power::SolveSettings {
                enforce_q_limits: model == stannum_core::power::ModelChoice::Ac,
                model,
                ..Default::default()
            })
            .unwrap()
    };
    let ac = solve(stannum_core::power::ModelChoice::Ac);
    assert!(ac.converged);
    let names: Vec<_> = ac
        .facts
        .iter()
        .map(|f| (f.name.as_str(), f.bus, f.terminal_bus))
        .collect();
    assert_eq!(names, vec![("1", 153, None), ("2", 153, Some(155))]);
    assert!(ac.facts.iter().all(|f| f.at_limit && !f.held));
    // The UPFC sends 350 MW into bus 155, which passes it on to 154, also
    // in a DC solve, where the shunts are held.
    let dc = solve(stannum_core::power::ModelChoice::Dc);
    assert_eq!(dc.facts[1].series_mw, 350.0);
    assert!(dc.facts.iter().all(|f| f.held));
    let flow = |r: &stannum_core::power::SolveReport| {
        r.branches
            .iter()
            .find(|b| (b.from, b.to) == (154, 155))
            .map(|b| b.pt)
            .unwrap()
    };
    assert!((flow(&dc) - 350.0).abs() < 1.0, "{}", flow(&dc));
    assert!((flow(&ac) - 350.0).abs() < 1.0, "{}", flow(&ac));
}
