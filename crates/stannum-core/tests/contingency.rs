//! N-1 contingency analysis: the linear screen against DC solves, the AC
//! check against solving the same outage as a study edit, and cancelling.
use rs_power::{DcFactors, OutageFlows};
use stannum_core::Workspace;
use stannum_core::contingency::{ContingencySettings, Method};
use stannum_core::power::{ModelChoice, SolveSettings};
use stannum_core::study::Edit;
use std::path::PathBuf;

fn case(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs-power/tests/fixtures")
        .join(name)
}

fn settings(method: Method) -> ContingencySettings {
    ContingencySettings {
        elements: None,
        branches: true,
        generators: true,
        method,
        ac_count: 20,
        limit_pct: 100.0,
        solve: SolveSettings::default(),
    }
}

fn solved(name: &str) -> Workspace {
    let mut ws = Workspace::default();
    ws.open(&case(name)).unwrap();
    let report = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings::default())
        .unwrap();
    assert!(report.converged);
    ws
}

/// The linear trip of each meshed branch equals a DC solve without it.
#[test]
fn linear_trips_match_dc_solves() {
    let text = std::fs::read_to_string(case("case30.m")).unwrap();
    let base = rs_power::parse_matpower(&text).unwrap();
    let dc = rs_power::PowerFlowOptions {
        model: rs_power::Model::Dc,
        ..Default::default()
    };
    let flows: Vec<f64> = rs_power::solve(&base, &dc)
        .unwrap()
        .branch_flows
        .iter()
        .map(|f| f[0])
        .collect();
    let factors = DcFactors::new(&base).unwrap();
    let mut meshed = 0;
    for row in 0..base.branches.len() {
        let OutageFlows::Flows(after) = factors.trip(&[row], &flows) else {
            continue;
        };
        meshed += 1;
        let mut out = base.clone();
        out.branches[row].online = false;
        let expected = rs_power::solve(&out, &dc).unwrap();
        for (m, f) in expected.branch_flows.iter().enumerate() {
            assert!(
                (after[m] - f[0]).abs() < 1e-6,
                "trip {row}, branch {m}: {} vs {}",
                after[m],
                f[0]
            );
        }
    }
    assert!(meshed > 30, "only {meshed} meshed branches");
}

/// Every AC-checked row matches solving that outage as a study edit.
#[test]
fn ac_rows_match_a_study_outage() {
    // case30 has branch ratings (case300 has none).
    let mut ws = solved("case30.m");
    let summary = ws
        .current_mut()
        .unwrap()
        .run_contingencies(settings(Method::Screen))
        .unwrap();
    assert!(summary.total > 40, "{} outages", summary.total);
    assert_eq!(summary.ac_checked, 20);
    assert!(!summary.cancelled);
    let rows = ws.current().unwrap().contingency_rows(0, 1000).unwrap();
    assert_eq!(rows.len(), summary.total);
    assert!(rows[0].worst.is_some());
    assert!(rows[..20].iter().all(|r| r.checked == "ac"));
    assert!(rows[20..].iter().all(|r| r.checked == "linear"));
    // Ranked worst first within each group.
    for pair in rows[..20].windows(2) {
        assert!(pair[0].severity >= pair[1].severity);
    }

    // Check a converged, non-islanding AC row against a study solve.
    let row = rows[..20]
        .iter()
        .find(|r| r.converged == Some(true) && r.cut_off_buses == 0 && r.worst.is_some())
        .expect("an ordinary AC row")
        .clone();
    let detail = ws
        .current_mut()
        .unwrap()
        .contingency_detail(row.rank)
        .unwrap();
    assert_eq!(detail.row.worst, row.worst);
    let open = ws.current_mut().unwrap();
    open.set_edits(vec![Edit::Status {
        element: row.element.clone(),
        online: false,
    }])
    .unwrap();
    assert!(open.contingencies.is_none(), "edits clear the results");
    let report = open.solve(SolveSettings::default()).unwrap();
    assert!(report.converged);
    let worst = row.worst.unwrap();
    let branch = report
        .branches
        .iter()
        .filter(|b| b.element == worst.element && b.from == worst.from && b.to == worst.to)
        .find_map(|b| b.loading_pct)
        .unwrap();
    assert!(
        (branch - worst.pct).abs() < 1.0,
        "{}: {branch:.2}% in the study vs {:.2}% in the run",
        row.element,
        worst.pct
    );
}

#[test]
fn islanding_outages_report_what_they_cut_off() {
    let mut ws = solved("case30.m");
    ws.current_mut()
        .unwrap()
        .run_contingencies(ContingencySettings {
            method: Method::Ac,
            ..settings(Method::Ac)
        })
        .unwrap();
    let rows = ws.current().unwrap().contingency_rows(0, 1000).unwrap();
    // case30's radial line 12-13 feeds a generator bus.
    let radial = rows
        .iter()
        .find(|r| r.element == "line:12:13:1")
        .expect("line 12-13");
    assert_eq!(radial.cut_off_buses, 1);
    assert!(radial.lost_generation_mw > 0.0);
    assert_eq!(radial.converged, Some(true));
    assert!(rows.iter().all(|r| r.checked == "ac"));
}

#[test]
fn needs_an_ac_solve_first() {
    let mut ws = Workspace::default();
    ws.open(&case("case30.m")).unwrap();
    let error = ws
        .current_mut()
        .unwrap()
        .run_contingencies(settings(Method::Screen))
        .unwrap_err();
    assert!(error.contains("Solve"), "{error}");
    ws.current_mut()
        .unwrap()
        .solve(SolveSettings {
            model: ModelChoice::Dc,
            ..SolveSettings::default()
        })
        .unwrap();
    assert!(
        ws.current_mut()
            .unwrap()
            .run_contingencies(settings(Method::Screen))
            .is_err()
    );
}

#[test]
fn a_run_can_be_cancelled() {
    let mut ws = solved("case300.m");
    let job = ws
        .current()
        .unwrap()
        .contingency_job(settings(Method::Ac))
        .unwrap();
    let total = job.len();
    let handle = job.handle();
    let canceller = std::thread::spawn(move || {
        while handle.progress().phase != "ac" || handle.progress().done < 5 {
            std::thread::yield_now();
        }
        handle.cancel();
    });
    let results = job.run().unwrap();
    canceller.join().unwrap();
    let summary = ws.current_mut().unwrap().set_contingencies(results);
    assert!(summary.cancelled);
    assert!(
        summary.ac_checked < total,
        "{} of {total}",
        summary.ac_checked
    );
    assert_eq!(summary.total, total, "screened rows are kept");
}
