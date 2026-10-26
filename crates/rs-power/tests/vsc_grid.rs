//! Meshed VSC DC grids in MatACDC's model, on Stannum's own test case.

use rs_power::{
    Case, DcModel, GridAcControl, GridDcControl, PowerFlowOptions, PowerFlowResult, SolveStatus,
    VscGrids, load_matacdc, load_matpower, parse_matacdc, solve,
};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn case14() -> (Case, VscGrids) {
    (
        load_matpower(fixture("case14.m")).unwrap(),
        load_matacdc(fixture("matacdc/case14_two_grids.m")).unwrap(),
    )
}

fn options(grids: &VscGrids) -> PowerFlowOptions {
    PowerFlowOptions {
        tolerance: 1e-10,
        dc_model: DcModel::Detailed {
            lines: Vec::new(),
            vsc_lines: Vec::new(),
            mtdc: Vec::new(),
            adjust_taps: false,
            vsc_grids: Some(grids.clone()),
        },
        ..PowerFlowOptions::default()
    }
}

fn run(case: &Case, grids: &VscGrids) -> PowerFlowResult {
    let result = solve(case, &options(grids)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    result
}

#[test]
fn reads_the_matacdc_format() {
    let (_, grids) = case14();
    assert_eq!(
        (grids.base_mva_ac, grids.base_mva_dc, grids.poles),
        (100.0, 100.0, 2.0)
    );
    assert_eq!(grids.buses.len(), 7);
    assert_eq!(grids.converters.len(), 6);
    assert_eq!(grids.branches.len(), 7);
    let c = &grids.converters[3];
    assert_eq!(
        (c.dc_bus, c.dc_control, c.ac_control),
        (4, GridDcControl::Droop, GridAcControl::ReactivePower)
    );
    assert_eq!(
        (c.droop, c.droop_p_mw, c.droop_v, c.droop_deadband),
        (0.004, 25.0, 1.0, 0.002)
    );
    // Struct fields, no droop columns, trailing result columns ignored.
    let text = "mpc.baseMVAac = 100;\nmpc.baseMVAdc = 200;\nmpc.pol = 1;\n\
        mpc.busdc = [1 2 1 0 1 345 1.1 0.9 0; 2 3 1 0 1 345 1.1 0.9 0];\n\
        mpc.convdc = [1 2 2 0 0 1 0 0 0 0 0.1 345 1.1 0.9 1.1 1 0 0 0 0;\n\
                      2 1 1 50 0 1 0 0 0 0 0.1 345 1.1 0.9 1.1 0 0 0 0 0];\n\
        mpc.branchdc = [1 2 0.01 0 0 100 100 100 1 3 4];\n";
    let g = parse_matacdc(text).unwrap();
    assert_eq!((g.base_mva_dc, g.poles), (200.0, 1.0));
    assert!(!g.converters[1].in_service);
    assert_eq!(g.converters[0].droop, 0.0);
    assert!(parse_matacdc("busdc = [1 2 1 0 1 345 1.1 0.9 0];").is_err());
}

#[test]
fn solves_two_grids_with_slack_droop_and_power_control() {
    let (case, grids) = case14();
    let result = run(&case, &grids);
    let s = result.vsc_grids.as_ref().unwrap();
    assert!(s.converged);
    let conv = |dc: u32| s.converters.iter().find(|c| c.dc_bus == dc).unwrap();
    let bus = |dc: u32| s.buses.iter().find(|b| b.number == dc).unwrap();

    // The slack holds its DC voltage and the power converter its order.
    assert!((bus(2).vdc - 1.0).abs() < 1e-12);
    assert!((conv(3).ps_mw + 40.0).abs() < 1e-9 && (conv(3).qs_mvar - 5.0).abs() < 1e-9);
    assert!((conv(7).ps_mw - 15.0).abs() < 1e-9);

    // Droop lines, outside and inside the deadband.
    let droop = |dc: u32, k: f64, p: f64, v: f64, band: f64| {
        let u = bus(dc).vdc;
        let dv = u - v;
        let shifted = if dv.abs() <= band {
            0.0
        } else {
            dv - band.copysign(dv)
        };
        let expected = p + shifted / k;
        assert!(
            (conv(dc).pdc_mw - expected).abs() < 1e-6,
            "droop at DC bus {dc}: {} MW at {u} p.u., expected {expected}",
            conv(dc).pdc_mw
        );
    };
    droop(1, 0.005, -20.0, 1.0, 0.0);
    droop(4, 0.004, 25.0, 1.0, 0.002);
    droop(6, 0.01, -17.0, 1.0, 0.0);

    // Each DC grid balances: converter powers out equal branch losses.
    for (grid, buses) in [(1, vec![1, 2, 3, 4]), (2, vec![6, 7])] {
        let taken: f64 = buses.iter().map(|&b| conv(b).pdc_mw).sum();
        let lost: f64 = s
            .branches
            .iter()
            .filter(|b| buses.contains(&b.from) || buses.contains(&b.to) || b.from == 5)
            .filter(|b| (grid == 1) == (b.from <= 5))
            .map(|b| b.loss_mw)
            .sum();
        assert!(
            (taken + lost).abs() < 1e-8,
            "grid {grid}: {taken} MW taken, {lost} MW lost"
        );
    }
    // The junction bus carries no converter power.
    assert_eq!(bus(5).p_mw, 0.0);

    // AC side: voltage-controlling converters hold their buses, and the AC
    // network balances with the converters' injections.
    let vm = |id: u32| result.case.buses.iter().find(|b| b.id == id).unwrap().vm;
    assert!((vm(5) - 1.02).abs() < 1e-9);
    assert!((vm(14) - 1.03).abs() < 1e-9);
    let generation: f64 = result.case.generators.iter().map(|g| g.pg).sum();
    let load: f64 = result.case.buses.iter().map(|b| b.pd).sum();
    let ac_loss: f64 = result.branch_flows.iter().map(|f| f[0] + f[2]).sum();
    let converters: f64 = s.converters.iter().map(|c| c.ps_mw).sum();
    assert!((generation + converters - load - ac_loss).abs() < 1e-6);

    // The case comes back as given: no added generators, loads unchanged.
    assert_eq!(result.case.generators.len(), case.generators.len());
    for (b, original) in result.case.buses.iter().zip(&case.buses) {
        assert_eq!(
            (b.kind, b.pd, b.qd),
            (original.kind, original.pd, original.qd)
        );
    }
}

#[test]
fn droop_converters_share_an_outage() {
    let (case, mut grids) = case14();
    // Without a slack, the three droop converters hold grid 1 between them.
    let slack = &mut grids.converters[1];
    (
        slack.dc_control,
        slack.droop,
        slack.droop_p_mw,
        slack.droop_v,
    ) = (GridDcControl::Droop, 0.005, 30.0, 1.0);
    let before = run(&case, &grids).vsc_grids.unwrap();
    // Take the 40 MW rectifier out: the droop converters deliver less, and
    // the DC voltage falls.
    grids.converters[2].in_service = false;
    let after = run(&case, &grids).vsc_grids.unwrap();
    assert!(!after.converters[2].in_service);
    assert_eq!(after.converters[2].ps_mw, 0.0);
    let mut shed = 0.0;
    for k in [0, 1, 3] {
        let change = before.converters[k].ps_mw - after.converters[k].ps_mw;
        assert!(change > 5.0, "converter {k} changed by {change} MW");
        shed += change;
    }
    assert!((shed - 40.0).abs() < 2.0, "{shed}");
    for k in 0..4 {
        assert!(after.buses[k].vdc < before.buses[k].vdc);
    }
}

#[test]
fn rejects_grids_that_do_not_fit_the_case() {
    let (case, grids) = case14();
    let mut bad = grids.clone();
    bad.buses[0].ac_bus = 99;
    assert!(
        bad.check(&case)
            .unwrap_err()
            .to_string()
            .contains("AC bus 99")
    );
    let mut bad = grids.clone();
    for c in &mut bad.converters {
        if c.dc_bus == 6 {
            c.dc_control = GridDcControl::Power;
        }
    }
    assert!(
        bad.check(&case)
            .unwrap_err()
            .to_string()
            .contains("DC grid 2")
    );
    assert!(grids.check(&case).is_ok());
}

#[test]
fn starts_from_a_flat_profile_and_repeats_from_the_solution() {
    let (case, grids) = case14();
    let first = run(&case, &grids);
    let mut opt = options(&grids);
    opt.initialization = rs_power::Initialization::Solution;
    let again = solve(&first.case, &opt).unwrap();
    let (a, b) = (first.vsc_grids.unwrap(), again.vsc_grids.unwrap());
    for (x, y) in a.converters.iter().zip(&b.converters) {
        assert!((x.ps_mw - y.ps_mw).abs() < 1e-6);
    }
    let mut flat = options(&grids);
    flat.initialization = rs_power::Initialization::Flat;
    let c = solve(&case, &flat).unwrap().vsc_grids.unwrap();
    for (x, y) in a.converters.iter().zip(&c.converters) {
        assert!((x.ps_mw - y.ps_mw).abs() < 1e-6);
    }
}

#[test]
fn reads_version_1_matpower_cases() {
    // MatACDC's AC cases are MATPOWER version 1 functions.
    let text = "function [baseMVA, bus, gen, branch] = two_bus\n\
        baseMVA = 100;\n\
        bus = [1 3 0 0 0 0 1 1.06 0 345 1 1.1 0.9; 2 1 40 10 0 0 1 1 0 345 1 1.1 0.9];\n\
        gen = [1 0 0 500 -500 1.06 100 1 250 10];\n\
        branch = [1 2 0.02 0.06 0.06 100 100 100 0 0 1];\n\
        return;\n";
    let case = rs_power::parse_matpower(text).unwrap();
    assert_eq!(case.branches[0].angmin, -360.0);
    assert_eq!(case.branches[0].angmax, 360.0);
    let solved = solve(&case, &PowerFlowOptions::default()).unwrap();
    assert_eq!(solved.status, SolveStatus::Converged);
}
