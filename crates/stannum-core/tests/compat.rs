//! Files saved by earlier releases still open: the run files and bus
//! coordinates in `tests/compat`, and the case a study there points to. The
//! study and diagram files themselves are read by the app, and checked by
//! `src/lib/compat/compat.test.ts`. See `tests/compat/README.md`.

use serde_json::{Value, json};
use stannum_core::commands::Session;
use stannum_core::{geo, run_file, study_file};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compat() -> PathBuf {
    repo().join("tests/compat")
}

fn case9() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/case9.m")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stannum-compat-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every file with this extension in tests/compat, as (release, path).
fn samples(extension: &str) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    for release in std::fs::read_dir(compat()).unwrap() {
        let release = release.unwrap().path();
        if !release.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&release).unwrap() {
            let file = file.unwrap().path();
            if file.extension().is_some_and(|e| e == extension) {
                let name = release.file_name().unwrap().to_string_lossy().into_owned();
                out.push((name, file));
            }
        }
    }
    out.sort();
    out
}

fn run(session: &Session, path: &Path) -> Value {
    serde_json::from_str(&session.invoke("run_file", json!({ "path": path })).unwrap()).unwrap()
}

#[test]
fn run_files_from_earlier_releases_still_read() {
    let files = samples(run_file::EXTENSION);
    assert!(files.len() >= 2, "{files:?}");
    for (release, file) in files {
        let steps = run_file::read(&file).unwrap_or_else(|e| panic!("{release} {file:?}: {e}"));
        assert!(!steps.is_empty(), "{release} {file:?}");
    }
}

#[test]
fn the_recorded_0_4_run_file_still_runs() {
    let dir = scratch("recorded");
    std::fs::copy(case9(), dir.join("case9.m")).unwrap();
    let file = dir.join("case9_outage.stnrun");
    std::fs::copy(compat().join("0.4.1/case9_outage.stnrun"), &file).unwrap();
    let s = Session::new();
    let report = run(&s, &file);
    assert_eq!(report["failed"], 0, "{report:#}");
    assert_eq!(report["steps"], 4);
    // The study edit it recorded is in place.
    let study: Value = serde_json::from_str(
        &s.invoke("result_report", json!({ "name": "study" }))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(study["converged"], true);
}

#[test]
fn the_0_4_example_run_file_still_runs() {
    // It opens ../case9.m and writes beside itself.
    let dir = scratch("example");
    let runs = dir.join("runs");
    std::fs::create_dir_all(&runs).unwrap();
    std::fs::copy(case9(), dir.join("case9.m")).unwrap();
    let file = runs.join("case9_study.stnrun");
    std::fs::copy(compat().join("0.4.1/case9_study.stnrun"), &file).unwrap();
    let report = run(&Session::new(), &file);
    assert_eq!(report["failed"], 0, "{report:#}");
    assert!(runs.join("case9_study.m").is_file());
}

#[test]
fn coordinates_files_from_earlier_releases_still_attach() {
    let files = samples("csv");
    assert!(!files.is_empty());
    for (release, file) in files {
        let read = geo::LoadedCoordinates::read(&file).unwrap_or_else(|e| panic!("{release}: {e}"));
        assert!(
            read.file.problems.is_empty(),
            "{release}: {:?}",
            read.file.problems
        );
        assert_eq!(read.file.places.len(), 9, "{release}");
    }
    let s = Session::new();
    s.invoke("open_case", json!({ "path": case9() })).unwrap();
    let summary: Value = serde_json::from_str(
        &s.invoke(
            "load_coordinates",
            json!({ "path": compat().join("0.4.1/case9-coordinates.csv") }),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(summary["from_file"], 9, "{summary:#}");
    assert_eq!(summary["estimated"], 0, "{summary:#}");
}

#[test]
fn studies_from_earlier_releases_find_their_case() {
    let files = samples("stannum");
    assert!(files.len() >= 2, "{files:?}");
    let expected = study_file::fingerprint(&case9()).unwrap();
    for (release, file) in files {
        let study: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(study["format"], "stannum-study", "{release}");
        assert_eq!(study["version"], 1, "{release}");
        let case = &study["case"];
        // The saved absolute path is from another machine; the relative one
        // leads to the case, which is unchanged.
        let found = study_file::locate_case(
            &file,
            case["path"].as_str().unwrap(),
            case["relative"].as_str(),
        )
        .unwrap_or_else(|| panic!("{release}: case not found"));
        assert_eq!(
            study_file::fingerprint(Path::new(&found)).unwrap().sha256,
            expected.sha256
        );
        assert_eq!(
            case["fingerprint"]["sha256"],
            expected.sha256.as_str(),
            "{release}"
        );
        if let Some(coords) = study["coordinates"].as_object() {
            let found = study_file::locate_case(
                &file,
                coords["path"].as_str().unwrap(),
                coords["relative"].as_str(),
            );
            assert!(found.is_some(), "{release}: coordinates not found");
        }
    }
}
