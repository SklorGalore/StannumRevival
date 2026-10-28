//! Run files end to end: read from disk, run in order, stop or keep going.

use serde_json::{Value, json};
use stannum_core::ErrorKind;
use stannum_core::commands::{RunOptions, Session};
use stannum_core::run_file;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/run_files")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stannum-run-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(session: &Session, path: &Path, args: Value) -> Value {
    let mut args = args;
    args["path"] = json!(path);
    serde_json::from_str(&session.invoke("run_file", args).unwrap()).unwrap()
}

#[test]
fn the_example_runs_every_line() {
    // Copy it beside a case, so its relative paths resolve in scratch space.
    let dir = scratch("example");
    let runs = dir.join("runs");
    std::fs::create_dir_all(&runs).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs-power/tests/fixtures/case9.m"),
        dir.join("case9.m"),
    )
    .unwrap();
    let file = runs.join("case9_study.stnrun");
    std::fs::copy(fixture("case9_study.stnrun"), &file).unwrap();

    let s = Session::new();
    let report = run(&s, &file, json!({}));
    assert_eq!(report["failed"], 0, "{report:#}");
    assert_eq!(report["steps"], 6);
    let lines: Vec<_> = report["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            (
                o["line"].as_u64().unwrap(),
                o["command"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (3, "open_case".into()),
            (4, "solve_power_flow".into()),
            (6, "set_study_edits".into()),
            (9, "solve_power_flow".into()),
            (10, "run_contingencies".into()),
            (11, "export_matpower".into()),
        ]
    );
    // Written beside the run file, where its relative path points.
    let written = std::fs::read_to_string(runs.join("case9_study.m")).unwrap();
    assert!(written.contains("mpc.gen"));
    // The workspace is left as the run file left it.
    let study: Value = serde_json::from_str(
        &s.invoke("result_report", json!({ "name": "study" }))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(study["converged"], true);
}

#[test]
fn a_failing_line_stops_the_run_unless_told_to_keep_going() {
    let dir = scratch("failing");
    let file = dir.join("bad.stnrun");
    std::fs::write(
        &file,
        format!(
            "open_case path={}\nelement_detail element=bus:999\nsolve_power_flow\n",
            serde_json::to_string(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs-power/tests/fixtures/case9.m")
            )
            .unwrap()
        ),
    )
    .unwrap();
    let s = Session::new();
    let stopped = run(&s, &file, json!({}));
    assert_eq!(stopped["failed"], 1);
    assert_eq!(stopped["outcomes"].as_array().unwrap().len(), 2);
    assert_eq!(stopped["outcomes"][1]["line"], 2);
    assert_eq!(stopped["outcomes"][1]["error"]["kind"], "not_found");

    let kept = run(&s, &file, json!({ "keepGoing": true }));
    assert_eq!(kept["failed"], 1);
    assert_eq!(kept["outcomes"].as_array().unwrap().len(), 3);
    assert!(kept["outcomes"][2]["error"].is_null());
}

#[test]
fn a_mistake_anywhere_stops_the_file_before_it_starts() {
    let dir = scratch("mistake");
    let file = dir.join("typo.stnrun");
    std::fs::write(&file, "close_case\nsolve_power_flow setings={}\n").unwrap();
    let s = Session::new();
    let e = s.invoke("run_file", json!({ "path": file })).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Failed);
    assert!(
        e.message
            .ends_with("Line 2: solve_power_flow has no argument setings. It takes settings."),
        "{e}"
    );
    let e = s
        .invoke("run_file", json!({ "path": dir.join("missing.stnrun") }))
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::File);
}

#[test]
fn steps_report_as_they_finish_and_cancel_stops_the_rest() {
    let s = Session::new();
    let steps = run_file::parse("list_commands\nlist_commands\nlist_commands\n").unwrap();
    let mut seen = vec![];
    let report = s
        .run_steps(Path::new("inline"), &steps, RunOptions::default(), |o| {
            seen.push(o.line);
            if o.line == 1 {
                s.invoke("cancel_run_file", Value::Null).unwrap();
            }
        })
        .unwrap();
    assert_eq!(seen, [1]);
    assert!(report.cancelled);
    assert!(!report.ok());
    // The next run starts afresh.
    let report = s
        .run_steps(Path::new("inline"), &steps, RunOptions::default(), |_| {})
        .unwrap();
    assert!(report.ok());
}
