//! Long work runs without the workspace lock: a solve, the first read of
//! the records and record edits are taken out of the open case, run, and
//! put back only if the case is still the one they were taken from.
use stannum_core::contingency::{ContingencySettings, Method};
use stannum_core::power::SolveSettings;
use stannum_core::record_edit::RecordEdit;
use stannum_core::study::Edit;
use stannum_core::workspace::CASE_CHANGED;
use stannum_core::{OpenCase, Workspace};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn open(name: &str) -> Workspace {
    let mut ws = Workspace::default();
    ws.open(&fixture(name)).unwrap();
    ws
}

fn trip(element: &str) -> Edit {
    Edit::Status {
        element: element.into(),
        online: false,
    }
}

fn first_branch(case: &OpenCase) -> String {
    case.diagram_network().unwrap().branches[0].element.clone()
}

#[test]
fn a_solve_that_finishes_after_an_edit_is_set_aside() {
    let mut ws = open("five_bus.raw");
    let case = ws.current_mut().unwrap();
    let job = case.solve_job(SolveSettings::default(), None).unwrap();
    // The study changes while the solve runs.
    let branch = first_branch(case);
    case.set_edits(vec![trip(&branch)]).unwrap();
    let done = job.run().unwrap();
    let e = case.finish_solve(done).unwrap_err();
    assert!(e.message.contains("changed during the solve"), "{e}");
    assert_eq!(e.kind, stannum_core::ErrorKind::CaseChanged);
    assert!(case.solved.is_none(), "the old case's solve isn't kept");

    // Undisturbed, the same steps keep the solve.
    let done = case
        .solve_job(SolveSettings::default(), None)
        .unwrap()
        .run()
        .unwrap();
    assert!(case.finish_solve(done).unwrap().converged);
    assert!(case.solved.is_some());
}

#[test]
fn contingencies_that_finish_after_an_edit_are_set_aside() {
    let mut ws = open("five_bus.raw");
    let case = ws.current_mut().unwrap();
    case.solve(SolveSettings::default()).unwrap();
    let settings = ContingencySettings {
        elements: None,
        branches: true,
        generators: true,
        method: Method::Screen,
        ac_count: 0,
        limit_pct: 100.0,
        solve: SolveSettings::default(),
    };
    let version = case.version;
    let job = case.contingency_job(settings.clone()).unwrap();
    let branch = first_branch(case);
    case.set_edits(vec![trip(&branch)]).unwrap();
    let results = job.run().unwrap();
    let e = case.keep_contingencies(version, results).unwrap_err();
    assert!(e.message.contains("changed while contingencies ran"), "{e}");
    assert_eq!(e.kind, stannum_core::ErrorKind::CaseChanged);
    assert!(case.contingencies.is_none());

    case.solve(SolveSettings::default()).unwrap();
    let version = case.version;
    let results = case.contingency_job(settings).unwrap().run().unwrap();
    assert!(case.keep_contingencies(version, results).is_ok());
    assert!(case.contingencies.is_some());
}

#[test]
fn records_read_for_an_older_case_are_not_kept() {
    let mut ws = open("five_bus.raw");
    let case = ws.current_mut().unwrap();
    let job = case.records_job().expect("not read yet");
    let branch = first_branch(case);
    case.set_edits(vec![trip(&branch)]).unwrap();
    assert_eq!(case.keep_records(job.run()).unwrap_err(), CASE_CHANGED);
    assert!(case.records.is_none());

    case.keep_records(case.records_job().unwrap().run())
        .unwrap();
    assert!(case.records_job().is_none(), "read once");
}

#[test]
fn record_edits_written_into_an_older_case_are_not_kept() {
    let mut ws = open("five_bus.raw");
    let case = ws.current_mut().unwrap();
    let loads = case.record_table("LOAD").unwrap();
    let pl = loads.columns.iter().position(|c| c.name == "PL").unwrap();
    let edit = |value: &str| RecordEdit {
        section: "LOAD".into(),
        row: 0,
        column: pl,
        value: value.into(),
    };
    let job = case.record_edit_job(&[edit("95.5")]).unwrap();
    // Another edit lands first.
    case.apply_record_edits(&[edit("80")]).unwrap();
    let done = job.run().unwrap();
    assert_eq!(case.finish_record_edits(done).unwrap_err(), CASE_CHANGED);
    let text = String::from_utf8(case.bytes.to_vec()).unwrap();
    assert!(!text.contains("95.5"), "the late edit isn't written");
    assert_eq!(case.record_edits, 1);
}

#[test]
fn record_edits_keep_a_save_as_made_while_they_ran() {
    let dir = std::env::temp_dir().join(format!("stannum-lock-free-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let saved = dir.join("saved.raw");
    let mut ws = open("five_bus.raw");
    let case = ws.current_mut().unwrap();
    let loads = case.record_table("LOAD").unwrap();
    let pl = loads.columns.iter().position(|c| c.name == "PL").unwrap();
    let job = case
        .record_edit_job(&[RecordEdit {
            section: "LOAD".into(),
            row: 0,
            column: pl,
            value: "95.5".into(),
        }])
        .unwrap();
    case.save_as(&saved).unwrap();
    let summary = case.finish_record_edits(job.run().unwrap()).unwrap();
    assert_eq!(summary.name, "saved.raw");
    assert_eq!(summary.record_edits, 1);
    std::fs::remove_dir_all(&dir).ok();
}
