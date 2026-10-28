//! The sample cases built into Stannum open, solve and place every bus.
use serde_json::{Value, json};
use stannum_core::commands::Session;
use std::path::PathBuf;

/// A fresh folder for one test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "stannum-samples-test-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn invoke(s: &Session, command: &str, args: Value) -> Value {
    serde_json::from_str(&s.invoke(command, args).unwrap()).unwrap()
}

#[test]
fn every_sample_opens_and_solves() {
    let dir = folder("all");
    let s = Session::new();
    let samples = invoke(&s, "sample_cases", json!({}));
    let samples = samples.as_array().unwrap();
    assert_eq!(samples.len(), 4);
    for sample in samples {
        let id = sample["id"].as_str().unwrap();
        let summary = invoke(&s, "open_sample", json!({ "id": id, "dir": dir }));
        let path = dir.join(sample["file_name"].as_str().unwrap());
        assert_eq!(summary["path"], path.display().to_string(), "{id}");
        assert!(path.exists(), "{id}");
        assert_eq!(summary["format"], sample["format"], "{id}");
        assert_eq!(summary["power"]["available"], true, "{id}");
        let buses = summary["power"]["info"]["counts"]["buses"]
            .as_u64()
            .unwrap();
        assert_eq!(buses, sample["buses"].as_u64().unwrap(), "{id}");

        if sample["has_coordinates"] == true {
            let c = &summary["coordinates"];
            assert!(c["from_file"].as_u64().unwrap() >= 5, "{id}: {c}");
            assert_eq!(c["missing"], 0, "{id}: {c}");
            assert_eq!(c["unknown_buses"], 0, "{id}: {c}");
            assert_eq!(c["problem_count"], 0, "{id}: {c}");
        } else {
            assert!(summary["coordinates"].is_null(), "{id}");
        }

        let report = invoke(&s, "solve_power_flow", json!({}));
        assert_eq!(report["converged"], true, "{id}: {}", report["status"]);
    }
}

#[test]
fn opening_a_sample_again_restores_its_file() {
    let dir = folder("restore");
    let s = Session::new();
    invoke(&s, "open_sample", json!({ "id": "case9", "dir": dir }));
    let path = dir.join("case9.m");
    let pristine = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, "broken").unwrap();
    invoke(&s, "open_sample", json!({ "id": "case9", "dir": dir }));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), pristine);
}

#[test]
fn an_unknown_sample_is_refused() {
    let s = Session::new();
    let e = s
        .invoke("open_sample", json!({ "id": "case99" }))
        .unwrap_err();
    assert!(e.message.contains("no sample called case99"), "{e}");
    assert!(e.message.contains("five_bus"), "{e}");
}
