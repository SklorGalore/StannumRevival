//! Cross-check of the VSC grid solver against MatACDC (J. Beerten, KU Leuven).
//!
//! `fixtures/vsc_grids/benchmark/matacdc_results.json` holds MatACDC 1.0's
//! results on Stannum's own two-area case, written by
//! `scripts/matacdc_benchmark.m`. MatACDC itself is not in this repository.
//! The solver follows the published paper; this benchmark checks it, and
//! where the two differ by design the difference is stated here and in the
//! verification docs, not tuned away.

use rs_power::{AcDcResult, DcGridSettings, PowerFlowOptions, load_matpower, solve_dc_grids};
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/vsc_grids")
        .join(name)
}

fn reference(scenario: &str) -> Value {
    let text = std::fs::read_to_string(fixture("benchmark/matacdc_results.json")).unwrap();
    let all: Value = serde_json::from_str(&text).unwrap();
    all[scenario].clone()
}

fn solve(file: &str, enforce_limits: bool) -> AcDcResult {
    let case = load_matpower(fixture("two_area.m")).unwrap();
    let grids =
        stannum_core::dc_grids::parse(&std::fs::read_to_string(fixture(file)).unwrap()).unwrap();
    let opt = PowerFlowOptions {
        tolerance: 1e-10,
        max_iterations: 30,
        ..Default::default()
    };
    let settings = DcGridSettings {
        max_iterations: 100,
        enforce_limits,
    };
    solve_dc_grids(&case, &opt, &grids, &settings).unwrap()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

/// Largest differences per quantity between Stannum and MatACDC, over the
/// converters `which` (indices in file order) and every DC and AC bus.
struct Diff {
    ps_mw: f64,
    qs_mvar: f64,
    loss_mw: f64,
    uc_pu: f64,
    vdc_pu: f64,
    vm_pu: f64,
}

fn compare(ours: &AcDcResult, theirs: &Value, which: &[usize]) -> Diff {
    let convs: Vec<_> = ours.grids.iter().flat_map(|g| &g.converters).collect();
    let refs = theirs["converters"].as_array().unwrap();
    assert_eq!(convs.len(), refs.len());
    let mut d = Diff {
        ps_mw: 0.0,
        qs_mvar: 0.0,
        loss_mw: 0.0,
        uc_pu: 0.0,
        vdc_pu: 0.0,
        vm_pu: 0.0,
    };
    for &k in which {
        let (c, r) = (convs[k], &refs[k]);
        assert_eq!(c.dc_bus as f64, num(&r["dc_bus"]));
        d.ps_mw = d.ps_mw.max((c.ps_mw - num(&r["ps_mw"])).abs());
        d.qs_mvar = d.qs_mvar.max((c.qs_mvar - num(&r["qs_mvar"])).abs());
        d.loss_mw = d.loss_mw.max((c.loss_mw - num(&r["loss_mw"])).abs());
        if c.in_service {
            d.uc_pu = d.uc_pu.max((c.uc_pu - num(&r["uc_pu"])).abs());
        }
    }
    let buses: Vec<_> = ours.grids.iter().flat_map(|g| &g.buses).collect();
    for (b, r) in buses.iter().zip(theirs["dc_buses"].as_array().unwrap()) {
        assert_eq!(b.number as f64, num(&r["number"]));
        d.vdc_pu = d.vdc_pu.max((b.v_pu - num(&r["v_pu"])).abs());
    }
    for r in theirs["ac_buses"].as_array().unwrap() {
        let bus = ours
            .ac
            .case
            .buses
            .iter()
            .find(|b| b.id as f64 == num(&r["id"]))
            .unwrap();
        d.vm_pu = d.vm_pu.max((bus.vm - num(&r["vm"])).abs());
    }
    d
}

fn assert_close(d: &Diff) {
    assert!(d.ps_mw < 1e-5, "Ps differs by {} MW", d.ps_mw);
    assert!(d.qs_mvar < 1e-5, "Qs differs by {} Mvar", d.qs_mvar);
    assert!(d.loss_mw < 1e-6, "loss differs by {} MW", d.loss_mw);
    assert!(d.uc_pu < 1e-7, "Uc differs by {}", d.uc_pu);
    assert!(d.vdc_pu < 1e-8, "Vdc differs by {}", d.vdc_pu);
    assert!(d.vm_pu < 1e-7, "AC |V| differs by {}", d.vm_pu);
}

#[test]
fn matches_matacdc_with_slack_droop_and_voltage_control() {
    let ours = solve("two_area.dcgrid.json", false);
    assert_close(&compare(&ours, &reference("base"), &[0, 1, 2, 3, 4, 5, 6]));
}

#[test]
fn matches_matacdc_when_droop_converters_take_up_an_outage() {
    let ours = solve("benchmark/two_area_a2_out.dcgrid.json", false);
    assert_close(&compare(
        &ours,
        &reference("a2_out"),
        &[0, 1, 2, 3, 4, 5, 6],
    ));
}

/// With limits the two differ by design. Stannum puts a limited converter
/// exactly on its limit circle (paper, section III-C); MatACDC 1.0 stops
/// inside it. Here South, ordered to 120 MW against a 1.0 p.u. current
/// limit, gets 101.85 MW at exactly 1.0 p.u. in Stannum and 100.0 MW at
/// about 0.98 p.u. in MatACDC, and Offshore's converter voltage sits at its
/// 1.08 p.u. cap in Stannum and at about 1.078 p.u. in MatACDC. The
/// four-terminal grid, which reaches no limit, still agrees exactly.
#[test]
fn differs_from_matacdc_at_limits_only_by_reaching_them_exactly() {
    let ours = solve("benchmark/two_area_limits.dcgrid.json", true);
    let theirs = reference("limits");
    let refs = theirs["converters"].as_array().unwrap();
    let three = &ours.grids[0].converters;

    let south = &three[1];
    assert!(south.p_limited);
    assert!(
        (south.current_pu - 1.0).abs() < 1e-6,
        "{}",
        south.current_pu
    );
    let gap = south.ps_mw - num(&refs[1]["ps_mw"]);
    assert!(gap > 0.0 && gap < 2.5, "South gets {gap} MW more");

    let offshore = &three[2];
    assert!((offshore.uc_pu - 1.08).abs() < 1e-6, "{}", offshore.uc_pu);
    assert!(num(&refs[2]["uc_pu"]) < 1.08);
    assert!((offshore.qs_mvar - num(&refs[2]["qs_mvar"])).abs() < 1.0);

    // The slack converter makes up the rest, so it moves by about as much.
    let north = &three[0];
    assert!((north.ps_mw - num(&refs[0]["ps_mw"]) + gap).abs() < 0.1);

    // The four-terminal grid reaches no limit and agrees as before (the AC
    // islands differ slightly because the three-terminal grid does).
    let d = compare(&ours, &theirs, &[3, 4, 5, 6]);
    assert!(d.ps_mw < 1e-2, "{}", d.ps_mw);
    assert!(d.loss_mw < 1e-3, "{}", d.loss_mw);
}
