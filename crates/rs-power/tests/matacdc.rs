//! MatACDC's own test cases. MatACDC may not be redistributed, so its case
//! files are not in this repository: point `STANNUM_MATACDC` at an unpacked
//! MatACDC 1.0 folder (see `scripts/fetch-matacdc.ps1`) and run
//! `cargo test -p rs-power --test matacdc -- --ignored`.

use rs_power::{
    Case, DcModel, PowerFlowOptions, PowerFlowResult, SolveStatus, VscGrids, load_matacdc,
    load_matpower, solve,
};
use std::path::PathBuf;

fn folder() -> PathBuf {
    PathBuf::from(
        std::env::var("STANNUM_MATACDC").expect("set STANNUM_MATACDC to the MatACDC1.0 folder"),
    )
}

fn load(ac: &str, dc: &str) -> (Case, VscGrids) {
    let root = folder();
    let case = load_matpower(root.join("Cases/PowerflowAC").join(format!("{ac}.m"))).unwrap();
    let grids = load_matacdc(root.join("Cases/PowerflowDC").join(format!("{dc}.m"))).unwrap();
    (case, grids)
}

fn run(case: &Case, grids: &VscGrids) -> PowerFlowResult {
    let options = PowerFlowOptions {
        tolerance: 1e-10,
        dc_model: DcModel::Detailed {
            lines: Vec::new(),
            vsc_lines: Vec::new(),
            mtdc: Vec::new(),
            adjust_taps: false,
            vsc_grids: Some(grids.clone()),
        },
        ..PowerFlowOptions::default()
    };
    let result = solve(case, &options).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    result
}

/// The results printed in section 6.1.1 of the MatACDC 1.0 manual.
#[test]
#[ignore = "needs MatACDC's case files (STANNUM_MATACDC)"]
fn manual_slack_case() {
    let (case, grids) = load("case5_stagg", "case5_stagg_MTDCslack");
    let result = run(&case, &grids);
    let s = result.vsc_grids.unwrap();
    let vdc: Vec<f64> = s.buses.iter().map(|b| b.vdc).collect();
    for (v, want) in vdc.iter().zip([1.008, 1.000, 0.998]) {
        assert!((v - want).abs() < 6e-4, "{vdc:?}");
    }
    let c = &s.converters;
    for (got, want) in [
        (c[1].ps_mw, 20.76),
        (c[1].qs_mvar, 7.14),
        (c[0].loss_mw, 1.29),
        (c[1].loss_mw, 1.14),
        (c[2].loss_mw, 1.17),
        // Station totals, with the transformer and reactor losses.
        (c[0].pdc_mw - c[0].ps_mw, 1.37),
        (c[1].pdc_mw - c[1].ps_mw, 1.14),
        (c[2].pdc_mw - c[2].ps_mw, 1.19),
        (c[0].pc_mw, -59.92),
        (c[0].qc_mvar, -32.63),
        (c[0].vc, 0.890),
        (c[0].vc_deg, -13.017),
        (s.buses[0].p_mw, -58.627),
        (s.buses[1].p_mw, 21.901),
        (s.buses[2].p_mw, 36.186),
        (s.branches[0].from_mw, 30.66),
        (s.branches[2].from_mw, 27.96),
    ] {
        assert!((got - want).abs() < 0.006, "got {got}, manual {want}");
    }
    let generation: f64 = result.case.generators.iter().map(|g| g.pg).sum();
    assert!((generation - 173.64).abs() < 0.006, "{generation}");
}

/// Section 4.1.3.1 of the manual: the point-to-point link's slack converter.
#[test]
#[ignore = "needs MatACDC's case files (STANNUM_MATACDC)"]
fn manual_point_to_point_case() {
    let (case, grids) = load("case5_stagg", "case5_stagg_HVDCptp");
    let s = run(&case, &grids).vsc_grids.unwrap();
    let c = &s.converters;
    assert!((c[1].ps_mw - 56.4959).abs() < 6e-5, "{}", c[1].ps_mw);
    assert!((c[1].qs_mvar - 0.0041).abs() < 6e-5, "{}", c[1].qs_mvar);
}

/// Droop control and the three-zone RTS-96 case solve; MatACDC's own
/// numbers come from `scripts/matacdc_reference.m`.
#[test]
#[ignore = "needs MatACDC's case files (STANNUM_MATACDC)"]
fn droop_and_three_zone_cases_solve() {
    for (ac, dc) in [
        ("case5_stagg", "case5_stagg_MTDCdroop"),
        ("case24_ieee_rts1996_3zones", "case24_ieee_rts1996_MTDC"),
    ] {
        let (case, grids) = load(ac, dc);
        let s = run(&case, &grids).vsc_grids.unwrap();
        println!("{ac} + {dc}: {} passes", s.passes);
        for b in &s.buses {
            println!("  DC bus {} {:.6} p.u. {:.4} MW", b.number, b.vdc, b.p_mw);
        }
        for c in &s.converters {
            println!(
                "  conv {} Ps {:.4} Qs {:.4} loss {:.4}",
                c.dc_bus, c.ps_mw, c.qs_mvar, c.loss_mw
            );
        }
    }
}

/// Figure 18 of the manual: converter 3 out, with power control (b) and
/// with droop control (c). The figure gives one decimal, and directions as
/// arrows, so only magnitudes are compared.
#[test]
#[ignore = "needs MatACDC's case files (STANNUM_MATACDC)"]
fn manual_outage_cases() {
    for (dc, ps, qs) in [
        ("case5_stagg_MTDCslack", [-60.0_f64, 56.7], [-40.0_f64, 0.1]),
        ("case5_stagg_MTDCdroop", [-38.8, 36.1], [-40.0, 7.1]),
    ] {
        let (case, mut grids) = load("case5_stagg", dc);
        grids.converters[2].in_service = false;
        let s = run(&case, &grids).vsc_grids.unwrap();
        for k in 0..2 {
            let c = &s.converters[k];
            assert!(
                (c.ps_mw.abs() - ps[k].abs()).abs() < 0.06,
                "{dc} converter {}: Ps {}",
                k + 1,
                c.ps_mw
            );
            assert!(
                (c.qs_mvar.abs() - qs[k].abs()).abs() < 0.06,
                "{dc} converter {}: Qs {}",
                k + 1,
                c.qs_mvar
            );
        }
    }
}
