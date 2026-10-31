//! The command schemas: the checked-in copies are current, and every
//! command's real answers (and errors) fit its schema.
//!
//! After changing a command or a type it returns, write the copies again:
//! `pwsh scripts/update-schemas.ps1` (or set STANNUM_UPDATE_SCHEMAS=1 and run
//! this test).

use serde_json::{Value, json};
use stannum_core::commands::{COMMANDS, Session, schemas};
use stannum_core::schema::{typescript, validate};
use stannum_core::{Error, ErrorKind};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

const SCHEMA_FILE: &str = "crates/stannum-core/schema/commands.schema.json";
const TYPESCRIPT_FILE: &str = "src/lib/analysis/commands.gen.ts";

fn check_file(relative: &str, current: &str) {
    let path = root().join(relative);
    if std::env::var_os("STANNUM_UPDATE_SCHEMAS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, current).unwrap();
        return;
    }
    let saved = std::fs::read_to_string(&path).unwrap_or_default();
    // Git may check text files out with CRLF on Windows.
    assert!(
        saved.replace("\r\n", "\n") == current,
        "{relative} is out of date: run `pwsh scripts/update-schemas.ps1`"
    );
}

#[test]
fn checked_in_schemas_are_current() {
    let doc = schemas();
    check_file(
        SCHEMA_FILE,
        &(serde_json::to_string_pretty(&doc).unwrap() + "\n"),
    );
    check_file(TYPESCRIPT_FILE, &typescript(&doc));
}

#[test]
fn every_command_has_a_schema() {
    let doc = schemas();
    let listed: BTreeSet<_> = COMMANDS.iter().map(|c| c.name).collect();
    let schemas: BTreeSet<_> = doc["commands"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(listed, schemas);
    // Every reference points at a definition.
    fn refs(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, f) in m {
                    match (k.as_str(), f) {
                        ("$ref", Value::String(r)) => out.push(r.clone()),
                        _ => refs(f, out),
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|i| refs(i, out)),
            _ => {}
        }
    }
    let mut all = vec![];
    refs(&doc, &mut all);
    for r in all {
        assert!(
            doc.pointer(r.trim_start_matches('#')).is_some(),
            "{r} isn't defined"
        );
    }
}

/// Runs commands and checks each answer against the command's schema.
struct Checked {
    session: Session,
    doc: Value,
    seen: BTreeSet<String>,
}

impl Checked {
    fn new() -> Self {
        Self {
            session: Session::new(),
            doc: schemas(),
            seen: BTreeSet::new(),
        }
    }

    fn args_fit(&self, command: &str, args: &Value) {
        let schema = &self.doc["commands"][command]["args"];
        if let Err(e) = validate(&self.doc, schema, args) {
            panic!("{command} arguments {args} don't fit the schema: {e}");
        }
    }

    fn ok(&mut self, command: &str, args: Value) -> Value {
        self.args_fit(command, &args);
        let text = self
            .session
            .invoke(command, args)
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        let value: Value = serde_json::from_str(&text).unwrap();
        let schema = &self.doc["commands"][command]["result"];
        if let Err(e) = validate(&self.doc, schema, &value) {
            panic!("{command}'s answer doesn't fit its schema: {e}");
        }
        self.seen.insert(command.to_string());
        value
    }

    fn err(&mut self, command: &str, args: Value) -> Error {
        let e = self.session.invoke(command, args).unwrap_err();
        let value = serde_json::to_value(&e).unwrap();
        validate(&self.doc, &self.doc["error"], &value).unwrap();
        e
    }
}

#[test]
fn answers_fit_their_schemas() {
    let mut c = Checked::new();
    let solve = json!({ "model": "ac", "start": "flat", "tolerance_mva": 0.01, "max_iterations": 20, "enforce_q_limits": false });

    c.ok("list_commands", json!({}));
    c.ok("element_kinds", json!({}));
    let served = c.ok("command_schemas", json!({}));
    assert_eq!(served, schemas());
    assert!(c.ok("case_summary", json!({})).is_null());
    c.ok("contingency_progress", json!({}));
    c.ok("transfer_progress", json!({}));
    c.ok("cancel_transfer_limits", json!({}));
    c.ok("log_entries", json!({ "since": 0 }));
    c.ok("diagnostics", json!({}));

    // A RAW case: every module works on it.
    let raw = fixture("five_bus.raw");
    let summary = c.ok("open_case", json!({ "path": raw }));
    let case = summary["id"].clone();
    c.ok("case_summary", json!({ "case": case }));
    c.ok("file_fingerprint", json!({ "path": raw }));
    c.ok(
        "study_relative_path",
        json!({ "study": fixture("study.stannum"), "casePath": raw }),
    );
    c.ok(
        "study_locate_case",
        json!({ "study": fixture("study.stannum"), "absolute": raw, "relative": "five_bus.raw" }),
    );
    let report = c.ok(
        "solve_power_flow",
        json!({ "case": case, "settings": solve }),
    );
    assert_eq!(report["converged"], true);
    c.ok("result_report", json!({ "name": "base" }));
    let page = c.ok(
        "result_buses",
        json!({
            "name": "base",
            "query": { "limit": 2, "sort": { "field": "vm", "descending": true }, "find": "bus:3" },
            "busRules": [{ "field": "area", "operator": "present" }],
        }),
    );
    assert_eq!(page["total"], 5);
    assert_eq!(page["rows"]["count"], 2);
    let vm = page["rows"]["vm"].as_array().unwrap();
    assert!(vm[0].as_f64() >= vm[1].as_f64(), "sorted highest first");
    c.ok(
        "result_generators",
        json!({ "name": "base", "query": { "search": "1" } }),
    );
    c.ok("result_branches", json!({ "name": "base" }));
    c.ok(
        "result_bus_values",
        json!({ "name": "base", "field": "kind" }),
    );
    c.ok("diagram_network", json!({}));
    let network = c.ok("diagram_network", json!({ "case": case }));
    let bus = network["buses"][0]["id"].as_u64().unwrap();
    let branch = network["branches"][0]["element"]
        .as_str()
        .unwrap()
        .to_string();
    let generator = network["generators"][0]["element"]
        .as_str()
        .unwrap()
        .to_string();
    for element in [format!("bus:{bus}"), branch.clone(), generator.clone()] {
        c.ok("element_detail", json!({ "element": element }));
    }
    c.ok("calculate_sensitivities", json!({ "settings": solve }));
    c.ok(
        "sensitivity_responses",
        json!({ "column": 0, "quantity": "magnitude", "engineering": false }),
    );
    c.ok(
        "load_coordinates",
        json!({ "path": fixture("five_bus.coordinates.csv") }),
    );
    c.ok("bus_coordinates", json!({}));
    c.ok("element_detail", json!({ "element": format!("bus:{bus}") }));
    c.ok("clear_coordinates", json!({}));
    let book = c.ok("case_records", json!({}));
    let section = book["sections"][0]["id"].as_str().unwrap().to_string();
    c.ok("record_table", json!({ "section": section }));
    let page = c.ok(
        "record_page",
        json!({ "section": section, "query": { "limit": 3 } }),
    );
    assert_eq!(page["page"]["rows"]["count"], 3);
    c.ok("record_fields", json!({ "section": section }));
    c.ok("record_values", json!({ "section": section, "field": "0" }));
    let topology = c.ok("topology_snapshot", json!({}));
    let to = topology["analysis"]["articulation_buses"]
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(json!(bus));
    c.ok("topology_shortest_path", json!({ "from": bus, "to": to }));
    let source = c.ok(
        "topology_source",
        json!({ "element": format!("bus:{bus}") }),
    );
    assert!(!source.is_null());
    let equipment = topology["case"]["equipment"]["id"][0]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        !c.ok("topology_source", json!({ "element": equipment }))
            .is_null()
    );
    assert!(
        c.ok("topology_source", json!({ "element": "nothing" }))
            .is_null()
    );
    c.ok("topology_set_merge", json!({ "enabled": true }));
    c.ok("topology_set_merge", json!({ "enabled": false }));
    let run = c.ok(
        "run_contingencies",
        json!({ "settings": { "method": "ac", "solve": solve } }),
    );
    assert!(run["total"].as_u64().unwrap_or(1) > 0);
    c.ok("contingency_summary", json!({}));
    c.ok("contingency_rows", json!({ "offset": 0, "limit": 10 }));
    c.ok(
        "contingency_page",
        json!({ "query": { "sort": { "field": "worst.loading_pct", "descending": true } } }),
    );
    c.ok("contingency_detail", json!({ "rank": 1 }));
    let csv = std::env::temp_dir().join(format!("stannum-violations-{}.csv", std::process::id()));
    c.ok("export_contingency_violations", json!({ "path": csv }));
    std::fs::remove_file(&csv).ok();
    c.ok("result_report", json!({ "name": "contingency 1" }));
    let counts = c.ok(
        "planning_event_counts",
        json!({ "settings": { "categories": ["P0", "P1"], "kv_min": 0.0 } }),
    );
    assert!(counts.as_array().is_some_and(|c| c.len() == 6), "{counts}");
    let planned = c.ok(
        "run_contingencies",
        json!({ "settings": { "method": "ac", "solve": solve, "planning": { "categories": ["P0", "P1.2"] } } }),
    );
    assert_eq!(planned["categories"][0]["category"], "P0");
    c.ok(
        "trip_and_compare",
        json!({ "element": branch, "limitPct": 100.0 }),
    );
    let transfer = c.ok(
        "run_transfer_limits",
        json!({ "settings": {
            "source": { "areas": [2] },
            "sink": { "areas": [1] },
            "contingencies": {},
        } }),
    );
    assert!(transfer["summary"]["fcitc_mw"].is_number(), "{transfer}");
    c.ok(
        "set_study_edits",
        json!({ "edits": [{ "kind": "generation", "element": generator, "mw": 10.0 }] }),
    );
    c.ok("solve_power_flow", json!({ "settings": solve }));
    c.ok("result_report", json!({ "name": "study" }));
    assert_eq!(c.ok("study_edits", json!({})).as_array().unwrap().len(), 1);
    c.ok("set_study_edits", json!({ "edits": [] }));
    c.ok("solve_power_flow", json!({ "settings": solve }));
    c.ok("diff_compare_working", json!({}));
    c.ok(
        "diff_compare",
        json!({ "baseline": raw, "comparison": fixture("five_bus_modified.raw") }),
    );
    let sections = c.ok("diff_section_records", json!({ "section": "BUS" }));
    if !sections.as_array().unwrap().is_empty() {
        c.ok(
            "diff_record_detail",
            json!({ "section": "BUS", "index": 0 }),
        );
    }
    c.ok(
        "diff_record_detail",
        json!({ "section": "BUS", "index": 9999 }),
    );
    let dir = std::env::temp_dir().join(format!("stannum-schemas-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    c.ok(
        "export_matpower",
        json!({ "path": dir.join("five_bus.m"), "solved": true }),
    );
    let exported = c.ok(
        "export_raw",
        json!({ "path": dir.join("five_bus_v33.raw"), "revision": 33 }),
    );
    assert_eq!(exported["source"], "records");
    let stn = c.ok("export_stn", json!({ "path": dir.join("five_bus.stn") }));
    assert!(stn["records"].as_u64().unwrap() > 0);
    c.ok("apply_record_edits", json!({ "edits": [] }));
    c.ok(
        "save_case_as",
        json!({ "path": dir.join("five_bus_copy.raw") }),
    );
    c.ok("cancel_solve", json!({}));
    c.ok("cancel_contingencies", json!({}));
    c.ok("cancel_run_file", json!({}));

    // A run file, recorded and run again.
    let run = dir.join("study.stnrun");
    c.ok(
        "save_run_file",
        json!({ "path": run, "steps": [
            { "command": "open_case", "args": { "path": raw } },
            { "command": "solve_power_flow", "args": { "settings": solve } },
            { "command": "export_matpower", "args": { "path": dir.join("again.m"), "solved": true } },
        ] }),
    );
    let report = c.ok("run_file", json!({ "path": run, "results": true }));
    assert_eq!(report["failed"], 0);
    assert_eq!(report["outcomes"].as_array().unwrap().len(), 3);
    assert_eq!(report["outcomes"][1]["result"]["converged"], true);
    assert!(dir.join("again.m").exists());
    c.ok("close_case", json!({}));

    // A MATPOWER case with a DC grid file, for the VSC grid report.
    c.ok(
        "open_case",
        json!({ "path": fixture("vsc_grids/two_area.m") }),
    );
    c.ok(
        "load_dc_grids",
        json!({ "path": fixture("vsc_grids/two_area.dcgrid.json") }),
    );
    let report = c.ok("solve_power_flow", json!({ "settings": solve }));
    assert!(!report["vsc_grids"].is_null());
    c.ok("clear_dc_grids", json!({}));
    c.ok("topology_snapshot", json!({}));

    // A built-in sample, written into the scratch folder.
    let samples = c.ok("sample_cases", json!({}));
    let id = samples[1]["id"].clone();
    let summary = c.ok("open_sample", json!({ "id": id, "dir": dir }));
    assert!(!summary["coordinates"].is_null());
    let _ = std::fs::remove_dir_all(&dir);

    let missing: Vec<_> = COMMANDS
        .iter()
        .map(|c| c.name)
        .filter(|n| !c.seen.contains(*n))
        .collect();
    assert!(missing.is_empty(), "not exercised: {missing:?}");
}

#[test]
fn errors_say_what_kind_they_are() {
    let mut c = Checked::new();
    let solve = json!({ "model": "ac", "start": "flat", "tolerance_mva": 0.01, "max_iterations": 20, "enforce_q_limits": false });
    let kind = |e: Error| e.kind;

    assert_eq!(
        kind(c.err("no_such_command", json!({}))),
        ErrorKind::UnknownCommand
    );
    assert_eq!(
        kind(c.err("open_case", json!({}))),
        ErrorKind::InvalidArguments
    );
    assert_eq!(kind(c.err("diagram_network", json!({}))), ErrorKind::NoCase);
    assert_eq!(
        kind(c.err("open_case", json!({ "path": fixture("no_such_case.raw") }))),
        ErrorKind::File
    );
    let summary = c.ok("open_case", json!({ "path": fixture("five_bus.raw") }));
    let id = summary["id"].as_u64().unwrap();
    assert_eq!(
        kind(c.err("diagram_network", json!({ "case": id + 1 }))),
        ErrorKind::CaseNotOpen
    );
    assert_eq!(
        kind(c.err("result_report", json!({ "name": "base" }))),
        ErrorKind::NotReady
    );
    assert_eq!(
        kind(c.err("contingency_rows", json!({ "offset": 0, "limit": 5 }))),
        ErrorKind::NotReady
    );
    assert_eq!(
        kind(c.err(
            "sensitivity_responses",
            json!({ "column": 0, "quantity": "magnitude", "engineering": false })
        )),
        ErrorKind::NotReady
    );
    assert_eq!(
        kind(c.err("bus_coordinates", json!({}))),
        ErrorKind::NotReady
    );
    assert_eq!(
        kind(c.err("element_detail", json!({ "element": "bus:99999" }))),
        ErrorKind::NotFound
    );
    assert_eq!(
        kind(c.err("element_detail", json!({ "element": "nonsense" }))),
        ErrorKind::InvalidArguments
    );
    assert_eq!(
        kind(c.err("record_table", json!({ "section": "NO SUCH SECTION" }))),
        ErrorKind::NotFound
    );
    assert_eq!(
        kind(c.err("result_report", json!({ "name": "???" }))),
        ErrorKind::InvalidArguments
    );
    c.ok("solve_power_flow", json!({ "settings": solve }));
    assert_eq!(
        kind(c.err("result_report", json!({ "name": "study" }))),
        ErrorKind::NotFound
    );
    assert_eq!(
        kind(c.err(
            "sensitivity_responses",
            json!({ "column": 0, "quantity": "nonsense", "engineering": false })
        )),
        ErrorKind::NotReady
    );
    c.ok("calculate_sensitivities", json!({ "settings": solve }));
    assert_eq!(
        kind(c.err(
            "sensitivity_responses",
            json!({ "column": 0, "quantity": "nonsense", "engineering": false })
        )),
        ErrorKind::InvalidArguments
    );
    c.ok(
        "run_contingencies",
        json!({ "settings": { "method": "screen", "solve": solve } }),
    );
    assert_eq!(
        kind(c.err("contingency_detail", json!({ "rank": 9999 }))),
        ErrorKind::NotFound
    );
    assert_eq!(
        kind(c.err("diff_section_records", json!({ "section": "BUS" }))),
        ErrorKind::NotReady
    );

    // MATPOWER cases can't be compared with case diff.
    c.ok("open_case", json!({ "path": fixture("case9.m") }));
    assert_eq!(
        kind(c.err("diff_compare_working", json!({}))),
        ErrorKind::Unsupported
    );
}
