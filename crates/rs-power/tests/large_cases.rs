//! Real-scale cases, ignored by default. Fetch them with
//! `pwsh scripts/fetch-test-cases.ps1`, then run
//! `cargo test --release -p rs-power --test large_cases -- --ignored`.
use rs_power::{DcLineKind, DcModel, LccMode, PowerFlowOptions, RawOptions, SolveStatus, solve};
use std::path::PathBuf;

fn case_text(file: &str) -> String {
    let dir = std::env::var_os("STANNUM_LARGE_CASES").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/large"),
        PathBuf::from,
    );
    let path = dir.join(file);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{} is missing ({e}); run pwsh scripts/fetch-test-cases.ps1",
            path.display()
        )
    });
    String::from_utf8_lossy(&bytes).into_owned()
}

fn options(enforce_q_limits: bool) -> PowerFlowOptions {
    PowerFlowOptions {
        enforce_q_limits,
        ..Default::default()
    }
}

fn stannum_raw(text: &str) -> rs_power::RawImport {
    rs_power::parse_raw_with_options(
        text,
        &RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            remote_regulation: true,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
#[ignore = "needs tests/large/SyntheticUSA.RAW"]
fn synthetic_usa_two_terminal_dc_lines() {
    let imported = stannum_raw(&case_text("SyntheticUSA.RAW"));
    assert_eq!(imported.revision, 33);
    assert_eq!(imported.case.buses.len(), 82_000);
    assert_eq!(imported.dc_line_kinds, [DcLineKind::TwoTerminal; 9]);
    let lines = imported.case.dcline.as_ref().unwrap();
    let mw: Vec<f64> = lines.iter().map(|l| l[3]).collect();
    assert_eq!(
        mw,
        [
            500.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0
        ]
    );
    // Firing angles fixed at 15 degrees: the converters absorb at least
    // P * tan(15 deg).
    for line in lines {
        let q = line[3] * 15f64.to_radians().tan();
        assert!((line[12] + q).abs() < 1e-9 && line[11] < line[12]);
    }

    let result = solve(&imported.case, &options(false)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    // Today's lossless model: every MW sent arrives.
    for line in result.case.dcline.as_ref().unwrap() {
        assert_eq!(line[3], line[4]);
    }
    // With Q limits on, 2,730 generators hold remote buses. Applied at each
    // generator's own terminal instead, the limit loop diverged.
    let limited = solve(&imported.case, &options(true)).unwrap();
    assert_eq!(limited.status, SolveStatus::Converged);
    let (mean, _) = voltage_error(&imported.case, &limited.case);
    assert!(mean < 0.002, "{mean}");

    // It also converges with every DC line switched off.
    let mut off = imported.case.clone();
    for line in off.dcline.as_mut().unwrap() {
        line[2] = 0.0;
    }
    assert_eq!(
        solve(&off, &options(true)).unwrap().status,
        SolveStatus::Converged
    );
}

/// Mean voltage magnitude difference (p.u.) and largest angle difference
/// (degrees) between a solution and the solved case it started from.
fn voltage_error(case: &rs_power::Case, solved: &rs_power::Case) -> (f64, f64) {
    let pairs = || case.buses.iter().zip(&solved.buses);
    let mean = pairs().map(|(a, b)| (a.vm - b.vm).abs()).sum::<f64>() / case.buses.len() as f64;
    let angle = pairs()
        .map(|(a, b)| (a.va - b.va).abs())
        .fold(0.0, f64::max);
    (mean, angle)
}

#[test]
#[ignore = "needs tests/large/SyntheticUSA.RAW"]
fn synthetic_usa_detailed_lcc_model() {
    let imported = stannum_raw(&case_text("SyntheticUSA.RAW"));
    assert_eq!(imported.lcc_lines.len(), 9);
    let result = solve(
        &imported.case,
        &PowerFlowOptions {
            dc_model: DcModel::Detailed {
                lines: imported.lcc_lines.clone(),
                vsc_lines: Vec::new(),
                mtdc: Vec::new(),
                adjust_taps: true,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    let mut total_loss = 0.0;
    for (s, line) in result.lcc.iter().zip(&imported.lcc_lines) {
        assert_eq!(s.mode, LccMode::RectifierControl);
        assert!((s.rectifier_kv * s.current_ka - line.setpoint).abs() < 1e-6);
        assert!(s.rectifier_mvar > 0.0 && s.inverter_mvar > 0.0);
        // α is held at its fixed 15° to within a tap step.
        assert!(s.alpha >= 15.0 && s.alpha < 21.0, "{}", s.alpha);
        total_loss += s.line_loss_mw;
    }
    // The inverter taps reach their 1.5 limit before VSCHD = 250 kV, so the
    // lines run near 315 to 345 kV: about 23 MW lost on the 500 MW line and
    // 0.8 to 0.9 MW on each 100 MW line, against 35 and 1.6 MW at 250 kV.
    let big = result.lcc[0].line_loss_mw;
    assert!(big > 20.0 && big < 26.0, "{big}");
    assert!(total_loss > 26.0 && total_loss < 34.0, "{total_loss}");

    // With Q limits and remote regulation the solution lands on the saved
    // PowerWorld one: under a thousandth of a p.u. apart on average.
    let limited = solve(
        &imported.case,
        &PowerFlowOptions {
            enforce_q_limits: true,
            dc_model: DcModel::Detailed {
                lines: imported.lcc_lines.clone(),
                vsc_lines: Vec::new(),
                mtdc: Vec::new(),
                adjust_taps: true,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(limited.status, SolveStatus::Converged);
    let (mean, angle) = voltage_error(&imported.case, &limited.case);
    assert!(mean < 0.001, "{mean}");
    assert!(angle < 3.0, "{angle}");
}

#[test]
#[ignore = "needs tests/large/Synthetic_East_West_US.RAW"]
fn synthetic_east_west_revision_34() {
    let imported = stannum_raw(&case_text("Synthetic_East_West_US.RAW"));
    assert_eq!(imported.revision, 34);
    assert_eq!(imported.case.buses.len(), 80_000);
    assert!(imported.case.dcline.is_none());
    let extra = imported
        .warnings
        .iter()
        .filter(|w| w.message.contains("additional section"))
        .count();
    assert_eq!(extra, 1, "substation data is reported once");
    for q in [false, true] {
        assert_eq!(
            solve(&imported.case, &options(q)).unwrap().status,
            SolveStatus::Converged
        );
    }
}

#[test]
#[ignore = "needs tests/large/case_ACTIVSg70k.m"]
fn activsg70k_matpower() {
    let case = rs_power::parse_matpower(&case_text("case_ACTIVSg70k.m")).unwrap();
    assert_eq!(case.buses.len(), 70_000);
    assert!(case.dcline.is_none());
    let result = solve(&case, &options(false)).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!(result.residual < 1e-8);
}
