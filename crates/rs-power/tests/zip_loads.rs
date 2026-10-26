//! Voltage-dependent (ZIP) loads: constant power, constant current and
//! constant impedance parts.
use rs_power::{
    BusType, Case, Initialization, Model, PowerFlowOptions, RawOptions, SolveStatus,
    format_matpower, parse_matpower, parse_raw_with_options, solve,
};
use serde_json::Value;

fn case30() -> Case {
    parse_matpower(include_str!("fixtures/case30.m")).unwrap()
}

/// Split every load into constant power, current and impedance shares.
fn split(case: &mut Case, p: [f64; 3], q: [f64; 3]) {
    for b in &mut case.buses {
        let (pd, qd) = (b.pd, b.qd);
        (b.pd, b.pd_i, b.pd_z) = (p[0] * pd, p[1] * pd, p[2] * pd);
        (b.qd, b.qd_i, b.qd_z) = (q[0] * qd, q[1] * qd, q[2] * qd);
    }
}

fn tight() -> PowerFlowOptions {
    PowerFlowOptions {
        tolerance: 1e-12,
        max_iterations: 20,
        initialization: Initialization::Flat,
        ..Default::default()
    }
}

#[test]
fn case30_zip_loads_match_pandapower() {
    // fixtures/zip/pandapower_case30.py solves the same split with pandapower.
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/zip/case30_zip_pandapower.json")).unwrap();
    let mut case = case30();
    split(&mut case, [0.4, 0.3, 0.3], [0.3, 0.2, 0.5]);
    let result = solve(&case, &tight()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    // Newton converges quadratically only with the load terms in the Jacobian.
    assert!(result.iterations <= 5, "{}", result.iterations);
    let column = |name: &str| -> Vec<f64> {
        reference[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect()
    };
    for ((b, vm), va) in result.case.buses.iter().zip(column("vm")).zip(column("va")) {
        assert!((b.vm - vm).abs() < 1e-9, "bus {} vm {} vs {vm}", b.id, b.vm);
        assert!((b.va - va).abs() < 1e-7, "bus {} va {} vs {va}", b.id, b.va);
    }
    let slack = &result.case.generators[0];
    assert!((slack.pg - reference["slack_p_mw"].as_f64().unwrap()).abs() < 1e-6);
    assert!((slack.qg - reference["slack_q_mvar"].as_f64().unwrap()).abs() < 1e-6);

    // The split changes the answer: constant power alone lands elsewhere.
    let constant = solve(&case30(), &tight()).unwrap();
    assert!((constant.case.generators[0].pg - slack.pg).abs() > 1.0);
}

#[test]
fn zip_solution_balances_its_loads_at_the_solved_voltage() {
    let mut case = case30();
    split(&mut case, [0.2, 0.5, 0.3], [0.1, 0.3, 0.6]);
    let zip = solve(&case, &tight()).unwrap();
    // Freeze each load at what it draws at the ZIP solution: constant power
    // must then reproduce that same operating point.
    let mut frozen = case.clone();
    for (b, solved) in frozen.buses.iter_mut().zip(&zip.case.buses) {
        b.vm = solved.vm;
        b.freeze_load();
    }
    let constant = solve(&frozen, &tight()).unwrap();
    for (a, b) in zip.case.buses.iter().zip(&constant.case.buses) {
        assert!((a.vm - b.vm).abs() < 1e-10 && (a.va - b.va).abs() < 1e-8);
    }
    for (a, b) in zip.case.generators.iter().zip(&constant.case.generators) {
        assert!((a.pg - b.pg).abs() < 1e-8 && (a.qg - b.qg).abs() < 1e-8);
    }
    // Bus::load reports what the solved bus draws.
    let b = zip.case.buses.iter().find(|b| b.id == 30).unwrap();
    let (p, q) = b.load();
    let input = case.buses.iter().find(|b| b.id == 30).unwrap();
    assert!((p - (input.pd + input.pd_i * b.vm + input.pd_z * b.vm * b.vm)).abs() < 1e-12);
    assert!((q - (input.qd + input.qd_i * b.vm + input.qd_z * b.vm * b.vm)).abs() < 1e-12);
}

#[test]
fn constant_impedance_load_is_a_shunt() {
    let mut zip = case30();
    let mut shunt = case30();
    for (z, s) in zip.buses.iter_mut().zip(&mut shunt.buses) {
        (z.pd_z, z.qd_z) = (z.pd, z.qd);
        (z.pd, z.qd) = (0.0, 0.0);
        s.gs += s.pd;
        s.bs -= s.qd;
        (s.pd, s.qd) = (0.0, 0.0);
    }
    let a = solve(&zip, &tight()).unwrap();
    let b = solve(&shunt, &tight()).unwrap();
    for (x, y) in a.case.buses.iter().zip(&b.case.buses) {
        assert!((x.vm - y.vm).abs() < 1e-12 && (x.va - y.va).abs() < 1e-10);
    }
}

#[test]
fn dc_power_flow_takes_zip_loads_at_one_per_unit() {
    let mut zip = case30();
    split(&mut zip, [0.4, 0.3, 0.3], [1.0, 0.0, 0.0]);
    let opt = PowerFlowOptions {
        model: Model::Dc,
        ..Default::default()
    };
    let a = solve(&zip, &opt).unwrap();
    let b = solve(&case30(), &opt).unwrap();
    for (x, y) in a.case.buses.iter().zip(&b.case.buses) {
        assert!((x.va - y.va).abs() < 1e-9);
    }
    assert!((a.case.generators[0].pg - b.case.generators[0].pg).abs() < 1e-9);
}

#[test]
fn matpower_export_writes_the_load_drawn_at_the_bus_voltage() {
    let mut case = case30();
    let b = &mut case.buses[3];
    (b.pd, b.pd_i, b.pd_z, b.qd, b.qd_i, b.qd_z, b.vm) = (1.0, 2.0, 4.0, 0.5, 1.0, -3.0, 0.9);
    let back = parse_matpower(&format_matpower(&case).unwrap()).unwrap();
    let b = &back.buses[3];
    assert!((b.pd - (1.0 + 2.0 * 0.9 + 4.0 * 0.81)).abs() < 1e-12);
    assert!((b.qd - (0.5 + 0.9 - 3.0 * 0.81)).abs() < 1e-12);
    assert!(!b.has_voltage_dependent_load());
}

fn psse_case3(zip_loads: bool) -> rs_power::RawImport {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let text = std::fs::read_to_string(path).unwrap();
    parse_raw_with_options(
        &text,
        &RawOptions {
            zip_loads,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn raw_zip_loads_keep_their_parts_on_request() {
    let zip = psse_case3(true);
    let bus = |case: &Case, id: u32| case.buses.iter().find(|b| b.id == id).unwrap().clone();
    // Bus 152: PL 1200, QL 360, IP 868.34, IQ 360.502, YP 837.794, YQ -351.338
    // (negative YQ is inductive, so it consumes 351.338 Mvar at 1 p.u.).
    let b = bus(&zip.case, 152);
    assert_eq!((b.pd, b.qd), (1200.0, 360.0));
    assert_eq!((b.pd_i, b.qd_i), (868.34, 360.502));
    assert_eq!((b.pd_z, b.qd_z), (837.794, 351.338));
    // Bus 205 sums loads B and C (and the constant-power-only load A).
    let b = bus(&zip.case, 205);
    assert_eq!((b.pd_i, b.qd_i), (110.0 + 45.0, 25.0 + 5.0));
    assert_eq!((b.pd_z, b.qd_z), (20.0 + 35.0, -10.0 - 30.0));
    assert!(
        !zip.warnings
            .iter()
            .any(|w| w.message.contains("ZIP load converted"))
    );

    // Without the option, MATPOWER's conversion: constant power drawn at the
    // file's bus voltage, with a warning.
    let plain = psse_case3(false);
    assert_eq!(
        plain
            .warnings
            .iter()
            .filter(|w| w.message.contains("ZIP load converted"))
            .count(),
        3
    );
    for (p, z) in plain.case.buses.iter().zip(&zip.case.buses) {
        assert!(!p.has_voltage_dependent_load());
        let (pd, qd) = z.load();
        assert!(
            (p.pd - pd).abs() < 1e-9 && (p.qd - qd).abs() < 1e-9,
            "{}",
            p.id
        );
    }
    // Strict mode accepts ZIP loads once they are modelled.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let strict = parse_raw_with_options(
        &std::fs::read_to_string(path).unwrap(),
        &RawOptions {
            zip_loads: true,
            strict: true,
            ..Default::default()
        },
    );
    if let Err(e) = strict {
        assert!(!e.to_string().contains("ZIP"), "{e}");
    }

    let solved = solve(&zip.case, &PowerFlowOptions::default()).unwrap();
    assert_eq!(solved.status, SolveStatus::Converged);
    let b = solved.case.buses.iter().find(|b| b.id == 152).unwrap();
    assert_eq!(b.kind, BusType::Pq);
}

#[test]
fn memphis_with_zip_loads_reproduces_the_case_voltages() {
    // Memphis has no ZIP loads, so give every load a 40/30/30 split that
    // draws exactly its constant power at the file's (solved) voltage. The
    // file's solution is then still a solution, and the ZIP solve must land
    // on it as closely as the constant-power solve does.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("_cases/MemphisCase2026_Mar7.RAW");
    let imported = rs_power::load_raw_with_options(
        &path,
        &RawOptions {
            remote_regulation: true,
            zip_loads: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut zip = imported.case.clone();
    for b in &mut zip.buses {
        let (pd, qd, vm) = (b.pd, b.qd, b.vm);
        (b.pd, b.pd_i, b.pd_z) = (0.4 * pd, 0.3 * pd / vm, 0.3 * pd / (vm * vm));
        (b.qd, b.qd_i, b.qd_z) = (0.4 * qd, 0.3 * qd / vm, 0.3 * qd / (vm * vm));
    }
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        voltage_controls: imported.voltage_controls.clone(),
        ..Default::default()
    };
    let mean_error = |case: &Case| {
        let result = solve(case, &opt).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        let error = (result.case.buses.iter().zip(&imported.case.buses))
            .map(|(a, b)| (a.vm - b.vm).abs())
            .sum::<f64>()
            / imported.case.buses.len() as f64;
        (error, result)
    };
    let (constant, a) = mean_error(&imported.case);
    let (with_zip, b) = mean_error(&zip);
    assert!(with_zip < 0.0002, "{with_zip}");
    assert!((with_zip - constant).abs() < 1e-5, "{constant} {with_zip}");
    let worst = (a.case.buses.iter().zip(&b.case.buses))
        .map(|(x, y)| (x.vm - y.vm).abs())
        .fold(0.0, f64::max);
    assert!(worst < 1e-3, "{worst}");
}
