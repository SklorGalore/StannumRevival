//! N-1 contingency analysis: the linear screen against DC solves, the AC
//! check against solving the same outage as a study edit, and cancelling.
use rs_power::{DcFactors, OutageFlows};
use stannum_core::Workspace;
use stannum_core::contingency::{ContingencySettings, Method, Rating};
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
        planning: None,
        ..Default::default()
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
    assert!(error.message.contains("Solve"), "{error}");
    assert_eq!(error.kind, stannum_core::ErrorKind::NotReady);
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

/// Trip and compare gives the same outcome as that outage in a full AC run,
/// and leaves the run's results alone.
#[test]
fn trip_and_compare_matches_the_ac_run() {
    let mut ws = solved("case30.m");
    let case = ws.current_mut().unwrap();
    let mut all = settings(Method::Ac);
    all.branches = true;
    all.generators = false;
    case.run_contingencies(all).unwrap();
    let rows = case.contingency_rows(0, 1).unwrap();
    let element = rows[0].element.clone();
    let tripped = case
        .trip_and_compare(&element, 100.0, Default::default())
        .unwrap();
    let ranked = case.contingency_detail(1).unwrap();
    assert_eq!(tripped.row.element, element);
    assert_eq!(tripped.row.checked, "ac");
    assert_eq!(tripped.overloads.len(), ranked.overloads.len());
    for (a, b) in tripped.branches.iter().zip(&ranked.branches) {
        assert!(
            (a.mw - b.mw).abs() < 1e-6,
            "{} {} vs {}",
            a.element,
            a.mw,
            b.mw
        );
    }
    // The run is still there to page through.
    assert!(case.contingency_summary().is_some());
    // An element that is already out can't be tripped.
    case.set_edits(vec![Edit::Status {
        element: element.clone(),
        online: false,
    }])
    .unwrap();
    case.solve(SolveSettings::default()).unwrap();
    let err = case
        .trip_and_compare(&element, 100.0, Default::default())
        .unwrap_err();
    assert!(err.message.contains("already out"), "{err}");
}

/// Every named result is a full solve report: a contingency or a trip
/// matches solving the case with that element out as a study edit.
#[test]
fn named_results_match_a_study_solve() {
    let mut ws = solved("case30.m");
    let case = ws.current_mut().unwrap();
    let base = case.result("base").unwrap();
    assert!(base.converged);
    assert!(
        case.result("study")
            .unwrap_err()
            .message
            .contains("\"base\"")
    );
    assert!(
        case.result("contingency 1")
            .unwrap_err()
            .message
            .contains("Run contingencies")
    );

    let mut all = settings(Method::Ac);
    all.generators = false;
    case.run_contingencies(all).unwrap();
    let element = case.contingency_rows(0, 1).unwrap()[0].element.clone();
    let outage = case.result("contingency 1").unwrap();
    assert!(outage.converged);
    let out = outage
        .branches
        .iter()
        .find(|b| b.element == element)
        .unwrap();
    assert!(!out.online && out.pf == 0.0, "{out:?}");

    case.trip_and_compare(&element, 100.0, Default::default())
        .unwrap();
    let tripped = case.result(&format!("trip {element}")).unwrap();
    assert!(
        case.result("trip line:999-998-1")
            .unwrap_err()
            .message
            .contains("only the last trip")
    );

    case.set_edits(vec![Edit::Status {
        element: element.clone(),
        online: false,
    }])
    .unwrap();
    // A study change drops results that described the case before it.
    assert!(case.result("trip x").is_err());
    assert!(
        case.result("study")
            .unwrap_err()
            .message
            .contains("Solve the case first")
    );
    let study = case.solve(SolveSettings::default()).unwrap();
    assert!(
        case.result("base")
            .unwrap_err()
            .message
            .contains("\"study\"")
    );
    assert_eq!(
        case.result("study").unwrap().branches.len(),
        study.branches.len()
    );
    for ((a, b), c) in outage
        .branches
        .iter()
        .zip(&tripped.branches)
        .zip(&study.branches)
    {
        assert!(
            (a.pf - c.pf).abs() < 1e-3 && (b.pf - c.pf).abs() < 1e-3,
            "{} {} {} {}",
            a.element,
            a.pf,
            b.pf,
            c.pf
        );
    }
}

/// case30 with rate B twice rate A and no rate C, written out and opened.
fn solved_with_rate_b() -> Workspace {
    let text = std::fs::read_to_string(case("case30.m")).unwrap();
    let mut c = rs_power::parse_matpower(&text).unwrap();
    for b in &mut c.branches {
        b.rate_b = 2.0 * b.rate_a;
        b.rate_c = 0.0;
    }
    let path = std::env::temp_dir().join(format!("stannum-ratings-{}.m", std::process::id()));
    std::fs::write(&path, rs_power::format_matpower(&c).unwrap()).unwrap();
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(
        ws.current_mut()
            .unwrap()
            .solve(SolveSettings::default())
            .unwrap()
            .converged
    );
    ws
}

/// Loadings follow the chosen ratings: rate B (twice rate A) halves every
/// post-outage loading, the base rating sets the "before" figures, and an
/// unset rate C leaves nothing rated.
#[test]
fn loadings_follow_the_chosen_ratings() {
    let mut ws = solved_with_rate_b();
    let run = |ws: &mut Workspace, rating: Rating, base_rating: Rating| {
        let open = ws.current_mut().unwrap();
        let summary = open
            .run_contingencies(ContingencySettings {
                rating,
                base_rating,
                ..settings(Method::Ac)
            })
            .unwrap();
        assert_eq!((summary.rating, summary.base_rating), (rating, base_rating));
        let rows = open.contingency_rows(0, 1000).unwrap();
        (summary, rows)
    };
    let (a, rows_a) = run(&mut ws, Rating::A, Rating::A);
    assert!(a.with_overloads > 0, "case30 has overloads on rate A");
    let (b, rows_b) = run(&mut ws, Rating::B, Rating::A);
    assert!(b.with_overloads < a.with_overloads);
    // Same outages, same flows: each worst branch is half as loaded on B.
    let by_element = |rows: &[stannum_core::contingency::ContingencyRow], element: &str| {
        rows.iter().find(|r| r.element == element).cloned().unwrap()
    };
    let mut compared = 0;
    for row in rows_a.iter().filter(|r| r.converged == Some(true)) {
        let (Some(wa), Some(wb)) = (&row.worst, &by_element(&rows_b, &row.element).worst) else {
            continue;
        };
        if wa.element == wb.element && wa.from == wb.from {
            assert!(
                (wa.pct - 2.0 * wb.pct).abs() < 1e-6,
                "{}: {} vs {}",
                row.element,
                wa.pct,
                wb.pct
            );
            // The base figure stays on rate A.
            assert!((wa.base_pct - wb.base_pct).abs() < 1e-9);
            compared += 1;
        }
    }
    assert!(compared > 10, "only {compared} rows compared");

    // Base rating B halves the "before" figures too.
    let (_, rows_bb) = run(&mut ws, Rating::B, Rating::B);
    let row = rows_bb.iter().find(|r| r.worst.is_some()).unwrap();
    let w = row.worst.as_ref().unwrap();
    let wa = by_element(&rows_b, &row.element).worst.unwrap();
    if wa.element == w.element && wa.from == w.from {
        assert!((wa.base_pct - 2.0 * w.base_pct).abs() < 1e-6);
    }

    // No branch has a rate C: nothing is rated, so nothing overloads.
    let (c, rows_c) = run(&mut ws, Rating::C, Rating::C);
    assert_eq!(c.with_overloads, 0);
    assert!(rows_c.iter().all(|r| r.overloads == 0));
}

/// Trip and compare takes the same ratings as a run.
#[test]
fn trip_and_compare_uses_the_chosen_ratings() {
    let mut ws = solved_with_rate_b();
    let open = ws.current_mut().unwrap();
    let summary = open.run_contingencies(settings(Method::Ac)).unwrap();
    let worst = summary.worst.expect("a worst outage");
    let on_a = open
        .trip_and_compare(&worst.element, 100.0, (Rating::A, Rating::A))
        .unwrap();
    let on_b = open
        .trip_and_compare(&worst.element, 100.0, (Rating::B, Rating::A))
        .unwrap();
    // Every rated branch is half as loaded on rate B, and the same before.
    assert_eq!(on_a.branches.len(), on_b.branches.len());
    let mut rated = 0;
    for (a, b) in on_a.branches.iter().zip(&on_b.branches) {
        assert_eq!((&a.element, a.from, a.to), (&b.element, b.from, b.to));
        assert_eq!(a.base_pct, b.base_pct);
        if let (Some(pa), Some(pb)) = (a.pct, b.pct) {
            assert!((pa - 2.0 * pb).abs() < 1e-6, "{}: {pa} vs {pb}", a.element);
            rated += 1;
        }
    }
    assert!(rated > 0);
}

/// A bus outage takes out every branch at the bus: the same outcome as
/// listing those branches together, and a load bus at the end of a radial
/// line is cut off with its load.
#[test]
fn bus_outages_take_out_everything_at_the_bus() {
    let mut ws = solved("case30.m");
    let open = ws.current_mut().unwrap();
    let summary = open
        .run_contingencies(ContingencySettings {
            branches: false,
            generators: false,
            buses: true,
            ..settings(Method::Ac)
        })
        .unwrap();
    assert_eq!(summary.total, 30, "one outage per bus");
    let rows = open.contingency_rows(0, 1000).unwrap();
    assert!(
        rows.iter()
            .all(|r| r.kind == "bus" && r.element.starts_with("bus:"))
    );
    // Bus 26 hangs off bus 25 by one line: taking it out loses its load.
    let b26 = rows.iter().find(|r| r.element == "bus:26").unwrap();
    assert_eq!(b26.cut_off_buses, 1);
    assert!(b26.lost_load_mw > 3.0, "{} MW", b26.lost_load_mw);

    // Bus 10's outage equals its branches listed together.
    let detail = |open: &mut stannum_core::OpenCase, element: &str| {
        open.run_contingencies(ContingencySettings {
            elements: Some(vec![element.to_string()]),
            ..settings(Method::Ac)
        })
        .unwrap();
        open.contingency_detail(1).unwrap()
    };
    let by_bus = detail(open, "bus:10");
    let lines: Vec<String> = by_bus
        .branches
        .iter()
        .filter(|b| b.tripped)
        .map(|b| b.element.clone())
        .collect();
    assert!(lines.len() >= 4, "{lines:?}");
    let together = detail(open, &lines.join("&"));
    assert_eq!(together.row.kind, "pair");
    assert_eq!(by_bus.row.worst, together.row.worst);
    assert_eq!(by_bus.row.cut_off_buses, together.row.cut_off_buses);
}

/// N-2 takes every pair of the chosen elements once, and each pair matches
/// the same two elements listed with "&".
#[test]
fn pairs_take_every_two_elements_once() {
    let mut ws = solved("case30.m");
    let open = ws.current_mut().unwrap();
    let chosen = [
        "line:1:2:1",
        "line:2:4:1",
        "line:6:8:1",
        "line:27:30:1",
        "generator:22:1",
    ];
    let summary = open
        .run_contingencies(ContingencySettings {
            elements: Some(chosen.iter().map(|s| s.to_string()).collect()),
            pairs: true,
            ..settings(Method::Ac)
        })
        .unwrap();
    assert_eq!(summary.total, 10, "5 choose 2");
    let rows = open.contingency_rows(0, 1000).unwrap();
    assert!(
        rows.iter()
            .all(|r| r.kind == "pair" && r.element.matches('&').count() == 1)
    );
    let pair = rows
        .iter()
        .find(|r| r.element == "line:6:8:1&generator:22:1")
        .expect("the pair, in list order")
        .clone();
    open.run_contingencies(ContingencySettings {
        elements: Some(vec!["line:6:8:1&generator:22:1".into()]),
        ..settings(Method::Ac)
    })
    .unwrap();
    let alone = open.contingency_rows(0, 1).unwrap();
    assert_eq!(alone[0].worst, pair.worst);
    assert_eq!(alone[0].overloads, pair.overloads);
}

/// 801 buses in a ring of 801 lines: 320,400 pairs, over the limit.
#[test]
fn too_many_pairs_are_refused() {
    let n = 801;
    let mut text =
        String::from("function mpc = ring\nmpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [\n");
    for b in 1..=n {
        let kind = if b == 1 { 3 } else { 1 };
        text += &format!("{b} {kind} 0.1 0 0 0 1 1 0 230 1 1.1 0.9;\n");
    }
    text += "];\nmpc.gen = [\n1 80 0 999 -999 1 100 1 999 0 0 0 0 0 0 0 0 0 0 0 0;\n];\nmpc.branch = [\n";
    for b in 1..=n {
        let to = b % n + 1;
        text += &format!("{b} {to} 0.001 0.01 0 100 100 100 0 0 1 -360 360;\n");
    }
    text += "];\n";
    let path = std::env::temp_dir().join(format!("stannum-ring-{}.m", std::process::id()));
    std::fs::write(&path, text).unwrap();
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let open = ws.current_mut().unwrap();
    assert!(open.solve(SolveSettings::default()).unwrap().converged);
    let err = open
        .run_contingencies(ContingencySettings {
            generators: false,
            pairs: true,
            ..settings(Method::Screen)
        })
        .unwrap_err();
    assert!(err.to_string().contains("320400 pairs"), "{err}");
    assert!(err.to_string().contains("Narrow the outages"), "{err}");
}

/// Leaving out zero-impedance lines drops every line with no resistance
/// and a reactance at or below the threshold, in N-1, N-2 and listed runs.
#[test]
fn zero_impedance_lines_can_be_left_out() {
    let text = std::fs::read_to_string(case("case30.m")).unwrap();
    let c = rs_power::parse_matpower(&text).unwrap();
    // case30's lossless lines have 0.1 to 0.56 p.u.: a 0.25 threshold
    // takes some of them.
    let threshold = 0.25;
    let zero: Vec<String> = c
        .branches
        .iter()
        .filter(|b| b.online && b.r == 0.0 && b.x.abs() <= threshold)
        .map(|b| format!("line:{}:{}:1", b.from.min(b.to), b.from.max(b.to)))
        .collect();
    assert!(zero.len() >= 3, "{zero:?}");
    let mut ws = solved("case30.m");
    let open = ws.current_mut().unwrap();
    let run = |open: &mut stannum_core::OpenCase, s: ContingencySettings| {
        let summary = open.run_contingencies(s).unwrap();
        let rows = open.contingency_rows(0, 2000).unwrap();
        (summary.total, rows)
    };
    let skip = |s: ContingencySettings| ContingencySettings {
        skip_zero_impedance: true,
        zero_impedance_pu: threshold,
        ..s
    };
    let (all, _) = run(open, settings(Method::Screen));
    let (fewer, rows) = run(open, skip(settings(Method::Screen)));
    assert_eq!(fewer, all - zero.len());
    assert!(rows.iter().all(|r| !zero.contains(&r.element)));
    // The default threshold (THRSHZ 0.0001) leaves case30 alone.
    let (same, _) = run(
        open,
        ContingencySettings {
            skip_zero_impedance: true,
            ..settings(Method::Screen)
        },
    );
    assert_eq!(same, all);
    // N-2 pairs only the rest; a listed zero-impedance line is left out too.
    let listed = vec![zero[0].clone(), "line:1:2:1".into(), "line:2:4:1".into()];
    let (pairs, _) = run(
        open,
        skip(ContingencySettings {
            elements: Some(listed),
            pairs: true,
            ..settings(Method::Screen)
        }),
    );
    assert_eq!(pairs, 1);
}

/// A lists file runs its named contingencies and the outages its sets
/// generate; what the case lacks or can't be applied is noted and skipped.
#[test]
fn a_lists_file_runs_its_contingencies() {
    let lists = "stannum-lists 0.1\n\
        subsystem one name=\"Area 1\" areas=[1]\n\
        contingency \"Line 1-2\" category=P1 events=[[open, line:2:1:1]]\n\
        contingency \"Two lines\" category=P2 events=[[open, line:1:2:1], [open, line:1:3:1]]\n\
        contingency \"Bus 26\" events=[[trip_bus, bus:26]]\n\
        contingency Missing events=[[open, line:98:99:1]]\n\
        contingency Closing events=[[close, line:1:2:1]]\n\
        contingency_set area_one kind=n_1_branches subsystem=one\n";
    let path = std::env::temp_dir().join(format!("stannum-lists-{}.stnl", std::process::id()));
    std::fs::write(&path, lists).unwrap();
    let mut ws = solved("case30.m");
    let open = ws.current_mut().unwrap();
    let summary = open
        .run_contingencies(ContingencySettings {
            lists: Some(path.to_string_lossy().into_owned()),
            ..settings(Method::Ac)
        })
        .unwrap();
    std::fs::remove_file(&path).ok();
    let rows = open.contingency_rows(0, 1000).unwrap();
    // Area 1's branches: those with an end on an area 1 bus.
    let text = std::fs::read_to_string(case("case30.m")).unwrap();
    let c = rs_power::parse_matpower(&text).unwrap();
    let area1: std::collections::HashSet<u32> = c
        .buses
        .iter()
        .filter(|b| b.area == 1)
        .map(|b| b.id)
        .collect();
    let in_area = c
        .branches
        .iter()
        .filter(|b| area1.contains(&b.from) || area1.contains(&b.to))
        .count();
    assert_eq!(summary.total, 3 + in_area, "{:?}", summary.notes);
    let named = |d: &str| {
        rows.iter()
            .find(|r| r.description.as_deref() == Some(d))
            .cloned()
    };
    let one = named("Line 1-2").expect("bus order doesn't matter");
    assert_eq!(
        (one.element.as_str(), one.category.as_deref()),
        ("line:1:2:1", Some("P1"))
    );
    assert_eq!(named("Two lines").unwrap().kind, "pair");
    let bus = named("Bus 26").unwrap();
    assert_eq!((bus.kind, bus.cut_off_buses), ("bus", 1));
    assert!(
        rows.iter()
            .filter(|r| r.category.as_deref() == Some("area_one"))
            .count()
            == in_area
    );
    assert_eq!(summary.notes.len(), 2, "{:?}", summary.notes);
    assert!(
        summary
            .notes
            .iter()
            .any(|n| n.contains("line:98:99:1 isn't in this case"))
    );
    assert!(
        summary
            .notes
            .iter()
            .any(|n| n.contains("close events aren't applied"))
    );
    assert!(summary.categories.iter().any(|c| c.category == "area_one"));
    // Each event counts once, however many kinds of problem it has.
    for c in &summary.categories {
        assert!(c.with_problems <= c.total, "{c:?}");
        assert!(c.with_problems >= c.with_overloads.max(c.with_voltage_problems));
    }
}

/// Monitors from a lists file choose what is checked: only the monitored
/// branch can overload, only the monitored bus has voltage problems (at the
/// monitor's limits), and an interface's flow is summed in the direction
/// its ids are written.
#[test]
fn a_lists_file_monitors_what_it_names() {
    let mut ws = solved("case30.m");
    let open = ws.current_mut().unwrap();
    let all = open.run_contingencies(settings(Method::Ac)).unwrap();
    let watched = all.worst.clone().unwrap().worst.unwrap();
    assert!(all.with_overloads > 0);
    let path = std::env::temp_dir().join(format!("stannum-monitors-{}.stnl", std::process::id()));
    let run = |open: &mut stannum_core::workspace::OpenCase, limit: &str| {
        // The watched branch is written with its buses swapped.
        let lists = format!(
            "stannum-lists 0.1\n\
             monitor one element=line:{}:{}:1 limit_pct=50\n\
             monitor v8 kind=voltage element=bus:8 vmin_pu=0.99\n\
             monitor bad element=line:98:99:1\n\
             interface gate elements=[[line:2:1:1, 1], [line:1:3:1, 1]]{limit}\n\
             monitor gate kind=interface interface=gate\n",
            watched.to, watched.from
        );
        std::fs::write(&path, lists).unwrap();
        let summary = open
            .run_contingencies(ContingencySettings {
                monitors: Some(path.to_string_lossy().into_owned()),
                ..settings(Method::Ac)
            })
            .unwrap();
        let rows = open.contingency_rows(0, 1000).unwrap().0;
        (summary, rows)
    };
    let (summary, rows) = run(open, "");
    assert!(summary.monitored);
    assert_eq!(summary.total, all.total);
    assert_eq!(summary.notes.len(), 1, "{:?}", summary.notes);
    assert!(summary.notes[0].contains("line:98:99:1 isn't a branch"));
    for r in rows.iter().filter(|r| r.converged == Some(true)) {
        if let Some(w) = &r.worst {
            assert_eq!(w.element, watched.element, "only the monitored branch");
        }
        assert!(r.overloads <= 1);
        if let Some(v) = &r.worst_voltage {
            assert_eq!((v.bus, v.vmin), (8, 0.99));
        }
        assert_eq!(r.interface_violations, 0, "no limit, no violation");
    }
    // At 50% of its rating, the watched branch is over in more outages.
    assert!(rows.iter().filter(|r| r.overloads > 0).count() >= 1);
    // The interface counts line 1-2 against its from end (written 2-1).
    let rank = rows
        .iter()
        .find(|r| r.element == "line:1:2:1")
        .unwrap()
        .rank;
    let detail = open.contingency_detail(rank).unwrap();
    let base = |id: &str| {
        detail
            .branches
            .iter()
            .find(|b| b.element == id)
            .unwrap()
            .base_mw
    };
    let gate = &detail.interfaces[0];
    assert_eq!(gate.name, "gate");
    assert!((gate.base_mw - (base("line:1:3:1") - base("line:1:2:1"))).abs() < 1e-6);
    // With line 1-2 out, all of it goes through 1-3.
    assert!(
        (gate.mw
            - detail
                .branches
                .iter()
                .find(|b| b.element == "line:1:3:1")
                .unwrap()
                .mw)
            .abs()
            < 1e-6
    );
    // A limit just above the base flow: the outages that push it further
    // violate it.
    let limit = gate.base_mw.abs() * 1.05;
    let (summary, rows) = run(open, &format!(" limit_mw={limit}"));
    std::fs::remove_file(&path).ok();
    assert!(summary.with_interface_problems > 0);
    let r = rows.iter().find(|r| r.interface_violations > 0).unwrap();
    let w = r.worst_interface.as_ref().unwrap();
    assert_eq!(w.name, "gate");
    assert!(w.pct.unwrap() > 100.0 && w.base_pct.unwrap() < 100.0);
}

/// How much worse an existing overload must get is a setting, and a run can
/// report existing problems too; the violations CSV lists every one.
#[test]
fn worse_by_and_existing_problems_are_settings() {
    let mut ws = solved("case30.m");
    let open = ws.current_mut().unwrap();
    let mut run = |worse_by_pct: f64, report_existing: bool| {
        open.run_contingencies(ContingencySettings {
            worse_by_pct,
            report_existing,
            ..settings(Method::Ac)
        })
        .unwrap();
        open.contingency_rows(0, 1000)
            .unwrap()
            .into_iter()
            .map(|r| (r.element.clone(), r.overloads))
            .collect::<std::collections::HashMap<_, _>>()
    };
    let default = run(1.0, false);
    let strict = run(1000.0, false);
    let every = run(1.0, true);
    let total = |m: &std::collections::HashMap<String, usize>| m.values().sum::<usize>();
    assert!(total(&default) > 0, "case30 has overloads on rate A");
    for (element, n) in &default {
        assert!(strict[element] <= *n, "{element}");
        assert!(every[element] >= *n, "{element}");
    }
    assert!(
        total(&every) > total(&strict),
        "existing overloads come back"
    );

    // The CSV (of the last run, reporting everything) has a line per
    // violation, each with every column.
    let summary = open.contingency_summary().unwrap();
    let rows = open.contingency_rows(0, 1000).unwrap();
    let path = std::env::temp_dir().join(format!("stannum-violations-{}.csv", std::process::id()));
    let written = open.export_contingency_violations(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let mut lines = text.lines();
    let header = lines.next().unwrap();
    assert_eq!(header, stannum_core::contingency::VIOLATIONS_CSV_HEADER);
    let columns = header.split(',').count();
    let body: Vec<&str> = lines.collect();
    assert_eq!(body.len(), written.rows);
    let expected: usize = rows
        .iter()
        .map(|r| {
            if r.converged == Some(false) {
                1
            } else {
                r.overloads + r.voltage_violations + r.interface_violations
            }
        })
        .sum();
    assert_eq!(written.rows, expected);
    assert!(
        written.rows > summary.with_overloads,
        "more than one per outage"
    );
    for line in &body {
        assert_eq!(line.split(',').count(), columns, "{line}");
    }
    assert!(body.iter().any(|l| l.split(',').nth(5) == Some("branch")));
}
