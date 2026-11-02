//! Redispatch through the command set.
use serde_json::{Value, json};
use stannum_core::commands::Session;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn call(s: &Session, command: &str, args: Value) -> Value {
    let text = s
        .invoke(command, args)
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// The five-bus case with line 1-2 (78 MW, 47 Mvar) rated 80 MVA, solved.
fn solved() -> Session {
    let s = Session::new();
    call(&s, "open_case", json!({ "path": fixture("five_bus.raw") }));
    let t = call(&s, "record_table", json!({ "section": "BRANCH" }));
    let column = t["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == "RATE1")
        .unwrap();
    call(
        &s,
        "apply_record_edits",
        json!({ "edits": [{ "section": "BRANCH", "row": 0, "column": column, "value": "80" }] }),
    );
    call(&s, "solve_power_flow", json!({}));
    s
}

fn pg(s: &Session) -> Vec<f64> {
    let t = call(s, "record_table", json!({ "section": "GENERATOR" }));
    let at = t["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == "PG")
        .unwrap();
    t["rows"]["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[at].as_f64().unwrap())
        .collect()
}

#[test]
fn needs_an_ac_solve() {
    let s = Session::new();
    call(&s, "open_case", json!({ "path": fixture("five_bus.raw") }));
    let e = s
        .invoke("redispatch_preview", json!({ "settings": {} }))
        .unwrap_err();
    assert!(e.message.contains("Solve the case"), "{e}");
}

#[test]
fn relieves_a_branch_and_writes_the_outputs() {
    let s = solved();
    let settings = json!({});
    let preview = call(&s, "redispatch_preview", json!({ "settings": settings }));
    let summary = &preview["summary"];
    assert_eq!(summary["status"], "relieved", "{preview:#}");
    let target = &preview["branches"][0];
    assert_eq!(target["element"], "line:1:2:1");
    assert!(
        target["pct_before"].as_f64().unwrap() > 110.0,
        "{preview:#}"
    );
    assert!(
        target["pct_after"].as_f64().unwrap() <= 100.05,
        "{preview:#}"
    );
    // The preview changes nothing.
    let before = pg(&s);
    assert_eq!(before, vec![0.0, 80.0]);

    let done = call(&s, "redispatch", json!({ "settings": settings }));
    // The rating edit and both units' new outputs.
    assert_eq!(done["case"]["record_edits"], 3, "{done:#}");
    let after = pg(&s);
    let unit = done["report"]["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["element"] == "generator:3:1")
        .unwrap()
        .clone();
    assert!(
        (after[1] - unit["mw_after"].as_f64().unwrap()).abs() < 1e-3,
        "{after:?} {unit}"
    );
    assert!(after[1] > 80.0, "the east unit picks up: {after:?}");
    // A fresh solve agrees with the check.
    let report = call(&s, "solve_power_flow", json!({}));
    let _ = report;
    let again = call(&s, "redispatch_preview", json!({ "settings": settings }));
    assert_eq!(again["summary"]["status"], "nothing_to_do", "{again:#}");
}
