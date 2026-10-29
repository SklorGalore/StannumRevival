//! NERC TPL-001 planning events: what each category generates from a case,
//! and that the run takes out shunts and DC lines as a solve without them
//! would.
use stannum_core::Workspace;
use stannum_core::contingency::{ContingencyRow, ContingencySettings, Method};
use stannum_core::planning::PlanningSettings;
use stannum_core::power::SolveSettings;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Tight enough that two solves of the same network agree to 1e-4 p.u.
fn solve_settings() -> SolveSettings {
    SolveSettings {
        tolerance_mva: 0.001,
        ..SolveSettings::default()
    }
}

fn every_voltage() -> PlanningSettings {
    PlanningSettings {
        kv_min: 0.0,
        ..PlanningSettings::default()
    }
}

fn planning(p: PlanningSettings, method: Method) -> ContingencySettings {
    ContingencySettings {
        method,
        ac_count: 0,
        solve: solve_settings(),
        planning: Some(p),
        ..ContingencySettings::default()
    }
}

fn open_text(text: &str) -> Workspace {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("stannum-planning-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("case.raw");
    std::fs::write(&path, text).unwrap();
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    let report = ws.current_mut().unwrap().solve(solve_settings()).unwrap();
    assert!(report.converged);
    ws
}

fn case_text() -> String {
    std::fs::read_to_string(fixture("five_bus_planning.raw")).unwrap()
}

fn run(ws: &mut Workspace, settings: ContingencySettings) -> Vec<ContingencyRow> {
    let case = ws.current_mut().unwrap();
    case.run_contingencies(settings).unwrap();
    case.contingency_rows(0, 1000)
        .unwrap()
        .into_iter()
        .collect()
}

fn counts(ws: &Workspace, p: &PlanningSettings) -> Vec<(String, usize)> {
    ws.current()
        .unwrap()
        .planning_event_counts(p)
        .unwrap()
        .into_iter()
        .map(|c| (c.category, c.count))
        .collect()
}

fn bus_vm(ws: &mut Workspace, name: &str) -> Vec<(u32, f64)> {
    let report = ws.current_mut().unwrap().result(name).unwrap();
    report.buses.iter().map(|b| (b.id, b.vm)).collect()
}

fn assert_same_voltages(a: &[(u32, f64)], b: &[(u32, f64)], what: &str) {
    assert_eq!(a.len(), b.len());
    for ((id, va), (_, vb)) in a.iter().zip(b) {
        assert!(
            (va - vb).abs() < 1e-4,
            "{what}: bus {id} at {va:.5} in the run vs {vb:.5} solved without it"
        );
    }
}

#[test]
fn each_category_generates_its_events() {
    let ws = open_text(&case_text());
    let got = counts(&ws, &every_voltage());
    let want: Vec<(String, usize)> = [
        ("P0", 1),
        ("P1.1", 2),
        // Lines 1-2, 2-3 and 4-5, and the multi-section line 1-3 as one.
        ("P1.2", 4),
        ("P1.3", 1),
        // The fixed shunt at bus 4 and the switched shunt at bus 5.
        ("P1.4", 2),
        ("P1.5", 1),
        // Each section of the multi-section line.
        ("P2.1", 2),
        ("P7.2", 1),
    ]
    .iter()
    .map(|(c, n)| (c.to_string(), *n))
    .collect();
    assert_eq!(got, want);

    // At 100 kV and above, the 69 kV spur and both shunts on it drop out.
    let high = counts(&ws, &PlanningSettings::default());
    let p12 = high.iter().find(|(c, _)| c == "P1.2").unwrap().1;
    let p14 = high.iter().find(|(c, _)| c == "P1.4").unwrap().1;
    assert_eq!((p12, p14), (3, 0));

    // Groups and single categories; unknown ones are refused.
    let p1 = PlanningSettings {
        categories: vec!["p1".into(), "P7.2".into()],
        ..every_voltage()
    };
    let names: Vec<String> = counts(&ws, &p1).into_iter().map(|(c, _)| c).collect();
    assert_eq!(names, ["P1.1", "P1.2", "P1.3", "P1.4", "P1.5", "P7.2"]);
    let p4 = PlanningSettings {
        categories: vec!["P4".into()],
        ..every_voltage()
    };
    let error = ws
        .current()
        .unwrap()
        .planning_event_counts(&p4)
        .unwrap_err();
    assert!(error.message.contains("doesn't generate P4"), "{error}");

    // Area 2 holds buses 3 and 5 only.
    let east = PlanningSettings {
        areas: vec![2],
        ..every_voltage()
    };
    let gens = counts(&ws, &east)
        .into_iter()
        .find(|(c, _)| c == "P1.1")
        .unwrap()
        .1;
    assert_eq!(gens, 1);
}

#[test]
fn a_run_labels_rows_by_category() {
    let mut ws = open_text(&case_text());
    let rows = run(&mut ws, planning(every_voltage(), Method::Ac));
    assert_eq!(rows.len(), 14);
    assert!(
        rows.iter()
            .all(|r| r.category.is_some() && r.checked == "ac")
    );
    let summary = ws.current().unwrap().contingency_summary().unwrap();
    assert_eq!(summary.categories.len(), 8);
    assert_eq!(
        summary.categories.iter().map(|c| c.total).sum::<usize>(),
        14
    );

    // The multi-section line goes out whole: both sections, and its dummy
    // bus isn't left behind on its own.
    let whole = rows.iter().find(|r| r.element == "line:1:3:&1").unwrap();
    assert_eq!(whole.category.as_deref(), Some("P1.2"));
    assert_eq!(whole.cut_off_buses, 1, "the dummy bus 6 has no other line");
    assert_eq!(whole.lost_load_mw, 0.0);
    let detail = ws
        .current_mut()
        .unwrap()
        .contingency_detail(whole.rank)
        .unwrap();
    assert_eq!(detail.branches.iter().filter(|b| b.tripped).count(), 2);

    // One section opened leaves the dummy bus fed from the other end.
    let section = rows
        .iter()
        .find(|r| r.category.as_deref() == Some("P2.1"))
        .unwrap();
    assert_eq!(section.cut_off_buses, 0);
    assert!(section.description.as_deref().unwrap().contains("Section"));
}

#[test]
fn p0_reports_problems_already_in_the_base_case() {
    let mut ws = open_text(&case_text());
    // At 10% of rate A every loaded branch is "overloaded" in the base case.
    let rows = run(
        &mut ws,
        ContingencySettings {
            limit_pct: 10.0,
            ..planning(every_voltage(), Method::Screen)
        },
    );
    let base = rows.iter().find(|r| r.kind == "base").unwrap();
    assert_eq!(base.category.as_deref(), Some("P0"));
    assert_eq!(base.element, "");
    assert_eq!(base.checked, "ac", "P0 is always solved in AC");
    assert!(base.overloads >= 3, "{} base overloads", base.overloads);
    // Other events count only what they make new or worse.
    let others: usize = rows
        .iter()
        .filter(|r| r.kind != "base")
        .map(|r| r.overloads)
        .max()
        .unwrap();
    assert!(others < base.overloads);
}

#[test]
fn shunt_outages_match_a_case_without_the_shunt() {
    let text = case_text();
    let mut ws = open_text(&text);
    let shunts = || PlanningSettings {
        categories: vec!["P1.4".into()],
        ..every_voltage()
    };
    // The screen can't see shunts: the largest ones get the AC budget.
    let rows = run(
        &mut ws,
        ContingencySettings {
            ac_count: 1,
            ..planning(shunts(), Method::Screen)
        },
    );
    let checked = |rows: &[ContingencyRow], element: &str| {
        rows.iter().find(|r| r.element == element).unwrap().checked
    };
    // 15 MVAr fixed against 10 MVAr switched.
    assert_eq!(checked(&rows, "shunt:4:1"), "ac");
    assert_eq!(checked(&rows, "switched-shunt:5:1"), "linear");

    let rows = run(&mut ws, planning(shunts(), Method::Ac));
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|r| r.checked == "ac" && r.converged == Some(true))
    );

    for (element, record) in [
        ("shunt:4:1", "    4,'1 ',1,     0.000,    15.000\n"),
        (
            "switched-shunt:5:1",
            "    5,'1 ',1,0,1,1.05000,0.95000,    5,   0,100.0,'            ',    10.00, 1, 2,  10.00\n",
        ),
    ] {
        let rank = rows.iter().find(|r| r.element == element).unwrap().rank;
        let run_vm = bus_vm(&mut ws, &format!("contingency {rank}"));
        assert!(text.contains(record), "{element} record");
        let mut without = open_text(&text.replace(record, ""));
        let solved_vm = bus_vm(&mut without, "latest");
        assert_same_voltages(&run_vm, &solved_vm, element);
    }
}

#[test]
fn dc_line_events_match_a_case_with_the_line_changed() {
    let text = case_text();
    let mut ws = open_text(&text);
    let rows = run(
        &mut ws,
        planning(
            PlanningSettings {
                categories: vec!["P1.5".into(), "P7.2".into()],
                ..every_voltage()
            },
            Method::Ac,
        ),
    );
    assert_eq!(rows.len(), 2);
    let record = "'East link   ',1,    5.0000,     40.00,";
    assert!(text.contains(record));
    for (category, changed) in [
        // One pole: half the 40 MW schedule.
        ("P1.5", "'East link   ',1,    5.0000,     20.00,"),
        // Both poles: blocked.
        ("P7.2", "'East link   ',0,    5.0000,     40.00,"),
    ] {
        let row = rows
            .iter()
            .find(|r| r.category.as_deref() == Some(category))
            .unwrap();
        assert_eq!(row.element, "dcline:1:3:East link");
        assert_eq!(row.converged, Some(true));
        let run_vm = bus_vm(&mut ws, &format!("contingency {}", row.rank));
        let mut other = open_text(&text.replace(record, changed));
        let solved_vm = bus_vm(&mut other, "latest");
        assert_same_voltages(&run_vm, &solved_vm, category);
    }
}

#[test]
fn the_linear_screen_moves_dc_line_power() {
    let mut ws = open_text(&case_text());
    let rows = run(
        &mut ws,
        planning(
            PlanningSettings {
                categories: vec!["P7.2".into()],
                ..every_voltage()
            },
            Method::Screen,
        ),
    );
    let row = &rows[0];
    assert_eq!(row.checked, "linear");
    let linear = ws
        .current_mut()
        .unwrap()
        .contingency_detail(row.rank)
        .unwrap();
    // The same outage in AC: the screen should get the MW shift about right.
    let rows = run(
        &mut ws,
        planning(
            PlanningSettings {
                categories: vec!["P7.2".into()],
                ..every_voltage()
            },
            Method::Ac,
        ),
    );
    let ac = ws
        .current_mut()
        .unwrap()
        .contingency_detail(rows[0].rank)
        .unwrap();
    let moved = |d: &stannum_core::contingency::ContingencyDetail, element: &str| {
        d.branches
            .iter()
            .find(|b| b.element == element)
            .map_or(0.0, |b| b.mw - b.base_mw)
    };
    // 40 MW no longer goes from bus 1 to bus 3 over the DC line, so the AC
    // path 1-2-3 picks some of it up.
    let ac_shift = moved(&ac, "line:1:2:1");
    let linear_shift = moved(&linear, "line:1:2:1");
    assert!(ac_shift > 5.0, "AC shift {ac_shift:.2} MW");
    assert!(
        (linear_shift - ac_shift).abs() < 0.2 * ac_shift,
        "linear {linear_shift:.2} MW vs AC {ac_shift:.2} MW"
    );
}

#[test]
fn matpower_bus_shunts_are_shunt_devices() {
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    ws.current_mut().unwrap().solve(solve_settings()).unwrap();
    let counts = counts(&ws, &every_voltage());
    // case9 has no bus shunts, lines only between buses, and no DC lines.
    let n = |c: &str| counts.iter().find(|(k, _)| k == c).unwrap().1;
    assert_eq!(n("P1.4"), 0);
    assert_eq!(n("P1.1"), 3);
    assert_eq!(n("P1.5") + n("P2.1") + n("P7.2"), 0);
}
