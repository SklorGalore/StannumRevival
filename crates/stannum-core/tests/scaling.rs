//! Scaling load and generation through the command set.
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

fn open(name: &str) -> Session {
    let s = Session::new();
    call(&s, "open_case", json!({ "path": fixture(name) }));
    s
}

/// A column of a record table, by field name, as numbers.
fn column(s: &Session, section: &str, field: &str) -> Vec<f64> {
    let t = call(s, "record_table", json!({ "section": section }));
    let at = t["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == field)
        .unwrap();
    // Rows come as columns: `cells` holds one array per row.
    t["rows"]["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[at].as_f64().unwrap())
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn area_load_scales_by_percent_at_constant_power_factor() {
    let s = open("five_bus.raw");
    let settings = json!({
        "buses": { "areas": [1] },
        "load": { "mw": { "kind": "percent", "value": 10.0 }, "constant_power_factor": true },
    });
    // The preview changes nothing.
    let preview = call(&s, "scale_preview", json!({ "settings": settings }));
    assert_eq!(preview["buses"], 3);
    let load = &preview["load"];
    assert!(
        near(load["mw_before"].as_f64().unwrap(), 130.0),
        "{preview}"
    );
    assert!(near(load["mw_after"].as_f64().unwrap(), 143.0), "{preview}");
    assert!(
        near(load["mvar_after"].as_f64().unwrap(), 46.2),
        "{preview}"
    );
    assert_eq!(column(&s, "LOAD", "PL"), vec![90.0, 40.0, 20.0]);

    let done = call(&s, "scale_load_generation", json!({ "settings": settings }));
    assert_eq!(done["report"]["changed"], 2);
    let pl = column(&s, "LOAD", "PL");
    let ql = column(&s, "LOAD", "QL");
    assert!(
        near(pl[0], 99.0) && near(pl[1], 44.0) && near(pl[2], 20.0),
        "{pl:?}"
    );
    assert!(
        near(ql[0], 33.0) && near(ql[1], 13.2) && near(ql[2], 6.0),
        "{ql:?}"
    );
    // The case still solves with the new load.
    call(&s, "solve_power_flow", json!({}));
}

#[test]
fn mw_and_mvar_scale_separately_to_targets() {
    let s = open("five_bus.raw");
    call(
        &s,
        "scale_load_generation",
        json!({ "settings": {
            "buses": { "kv_max": 100.0 },
            "load": {
                "mw": { "kind": "target", "value": 90.0 },
                "mvar": { "kind": "change", "value": -9.0 },
            },
        } }),
    );
    // Buses 4 and 5 are the 69 kV ones: 60 MW goes to 90, 18 Mvar to 9.
    let pl = column(&s, "LOAD", "PL");
    let ql = column(&s, "LOAD", "QL");
    assert!(near(pl[1], 60.0) && near(pl[2], 30.0), "{pl:?}");
    assert!(near(ql[1], 6.0) && near(ql[2], 3.0), "{ql:?}");
    assert!(near(pl[0], 90.0));
}

#[test]
fn generation_stops_at_pmax_unless_told_not_to() {
    let s = open("five_bus.raw");
    let generation = |respect: bool| {
        json!({ "settings": {
            "buses": { "areas": [2] },
            "generation": { "mw": { "kind": "target", "value": 400.0 }, "respect_limits": respect },
        } })
    };
    let kept = call(&s, "scale_preview", generation(true));
    assert!(
        near(kept["generation"]["mw_after"].as_f64().unwrap(), 300.0),
        "{kept}"
    );
    assert!(
        near(kept["shortfall_mw"].as_f64().unwrap(), 100.0),
        "{kept}"
    );
    assert_eq!(kept["at_limit"], 1);

    call(&s, "scale_load_generation", generation(false));
    let pg = column(&s, "GENERATOR", "PG");
    assert!(near(pg[0], 0.0) && near(pg[1], 400.0), "{pg:?}");
}

#[test]
fn units_at_zero_share_by_pmax() {
    let s = open("five_bus.raw");
    let report = call(
        &s,
        "scale_preview",
        json!({ "settings": { "buses": { "areas": [1] }, "generation": { "mw": { "kind": "change", "value": 50.0 } } } }),
    );
    // The only area 1 unit is at 0 MW, so scaling by output can't move it.
    assert!(
        near(report["shortfall_mw"].as_f64().unwrap(), 50.0),
        "{report}"
    );
    assert!(!report["notes"].as_array().unwrap().is_empty());
    let report = call(
        &s,
        "scale_preview",
        json!({ "settings": { "buses": { "areas": [1] }, "generation": {
            "mw": { "kind": "change", "value": 50.0 }, "participation": "pmax",
        } } }),
    );
    assert!(
        near(report["generation"]["mw_after"].as_f64().unwrap(), 50.0),
        "{report}"
    );
}

#[test]
fn matpower_loads_scale_on_the_bus_table() {
    let s = open("case9.m");
    call(
        &s,
        "scale_load_generation",
        json!({ "settings": {
            "load": { "mw": { "kind": "percent", "value": -10.0 } },
            "generation": { "mw": { "kind": "percent", "value": -10.0 } },
        } }),
    );
    let pd = column(&s, "bus", "PD");
    assert!(
        near(pd[4], 81.0) && near(pd[6], 90.0) && near(pd[8], 112.5),
        "{pd:?}"
    );
    let qd = column(&s, "bus", "QD");
    assert!(near(qd[4], 30.0), "Mvar stays: {qd:?}");
    let pg = column(&s, "gen", "PG");
    assert!(near(pg[1], 146.7), "{pg:?}");
}

#[test]
fn non_conforming_loads_stay_unless_asked() {
    let s = open("five_bus.raw");
    // Make bus 4's load non-conforming (SCALE = 0).
    let t = call(&s, "record_table", json!({ "section": "LOAD" }));
    let scale = t["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == "SCALE")
        .unwrap();
    call(
        &s,
        "apply_record_edits",
        json!({ "edits": [{ "section": "LOAD", "row": 1, "column": scale, "value": "0" }] }),
    );
    let settings = |include: bool| {
        json!({ "settings": {
            "buses": { "areas": [1] },
            "load": { "mw": { "kind": "percent", "value": 10.0 }, "include_non_conforming": include },
        } })
    };
    let kept = call(&s, "scale_preview", settings(false));
    // Only bus 2's 90 MW moves, by 10%; bus 4's 40 MW stays.
    assert!(
        near(kept["load"]["mw_before"].as_f64().unwrap(), 130.0),
        "{kept}"
    );
    assert!(
        near(kept["load"]["mw_after"].as_f64().unwrap(), 139.0),
        "{kept}"
    );
    assert_eq!(kept["changed"], 1);
    assert!(
        kept["notes"].to_string().contains("non-conforming"),
        "{kept}"
    );
    // A target still counts every load: 150 MW takes bus 2 to 110.
    let target = call(
        &s,
        "scale_preview",
        json!({ "settings": { "buses": { "areas": [1] }, "load": { "mw": { "kind": "target", "value": 150.0 } } } }),
    );
    assert!(
        near(target["load"]["mw_after"].as_f64().unwrap(), 150.0),
        "{target}"
    );
    call(&s, "scale_load_generation", settings(true));
    let pl = column(&s, "LOAD", "PL");
    assert!(near(pl[0], 99.0) && near(pl[1], 44.0), "{pl:?}");
}

#[test]
fn filters_narrow_each_other() {
    let s = open("five_bus.raw");
    // Area 1 and 69 kV: buses 4 only (bus 5 is area 2).
    let report = call(
        &s,
        "scale_preview",
        json!({ "settings": {
            "buses": { "areas": [1], "kv_max": 69.0 },
            "load": { "mw": { "kind": "change", "value": 4.0 } },
        } }),
    );
    assert_eq!(report["buses"], 1, "{report}");
    assert!(
        near(report["load"]["mw_after"].as_f64().unwrap(), 44.0),
        "{report}"
    );
}

#[test]
fn nothing_chosen_is_an_error() {
    let s = open("five_bus.raw");
    let e = s
        .invoke(
            "scale_preview",
            json!({ "settings": { "buses": { "areas": [9] }, "load": { "mw": { "kind": "change", "value": 1.0 } } } }),
        )
        .unwrap_err();
    assert!(e.message.contains("No bus"), "{}", e.message);
    assert!(
        s.invoke("scale_preview", json!({ "settings": {} }))
            .is_err()
    );
}
