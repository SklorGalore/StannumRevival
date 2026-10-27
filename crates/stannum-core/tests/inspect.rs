//! The element inspector's case data, read from the file.
use stannum_core::Workspace;
use stannum_core::inspect::Value;
use std::path::PathBuf;

fn open(name: &str) -> Workspace {
    let mut ws = Workspace::default();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    ws.open(&path).unwrap();
    ws
}

fn number(fields: &[stannum_core::inspect::Field], label: &str, unit: &str) -> f64 {
    match fields
        .iter()
        .find(|f| f.label == label && f.unit == unit)
        .map(|f| &f.value)
    {
        Some(Value::Number(v)) => *v,
        other => panic!("{label} ({unit}): {other:?}"),
    }
}

#[test]
fn describes_each_kind_in_case9() {
    let ws = open("case9.m");
    let case = ws.case.as_ref().unwrap();

    let bus = case.element_detail("bus:5").unwrap();
    assert_eq!(bus.buses[0].number, 5);
    assert_eq!(number(&bus.data[0].fields, "Load", "MW"), 90.0);
    assert_eq!(number(&bus.limits, "Maximum voltage", "p.u."), 1.1);
    assert!(bus.connected.contains(&"line:4:5:1".to_string()));
    assert!(bus.connected.contains(&"line:5:6:1".to_string()));

    // case9 branch 4-5: r 0.017, x 0.092, b 0.158, rate A 250 on 345 kV.
    let line = case.element_detail("line:4:5:1").unwrap();
    assert_eq!(
        line.buses.iter().map(|b| b.number).collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert_eq!(number(&line.data[0].fields, "Reactance", "p.u."), 0.092);
    let ohms = number(&line.data[0].fields, "Reactance", "Ω");
    assert!((ohms - 0.092 * 345.0 * 345.0 / 100.0).abs() < 1e-9);
    assert_eq!(number(&line.limits, "Rate A", "MVA"), 250.0);

    let gen_ = case.element_detail("generator:2:1").unwrap();
    assert_eq!(number(&gen_.data[0].fields, "Output", "MW"), 163.0);
    assert_eq!(number(&gen_.limits, "Maximum output", "MW"), 300.0);

    let load = case.element_detail("load:5:1").unwrap();
    assert!(load.note.is_some(), "MATPOWER loads are bus totals");

    assert!(case.element_detail("line:4:5:9").is_err());
    assert!(case.element_detail("nonsense").is_err());
}

#[test]
fn raw_loads_and_transformers() {
    let ws = open("five_bus.raw");
    let case = ws.case.as_ref().unwrap();
    let power = case.power.as_ref().unwrap();
    for e in &power.elements.branches {
        let d = case.element_detail(&e.element).unwrap();
        assert!(!d.data.is_empty() && d.limits.len() >= 3, "{}", e.element);
        assert!(d.buses.len() >= 2, "{}", e.element);
    }
    for e in &power.elements.generators {
        case.element_detail(&e.element).unwrap();
    }
    // nodal reads revision 35, so a RAW load is its own record.
    let topo = case.topology().as_ref().unwrap();
    if let Some(l) = topo.case.loads.first() {
        let d = case.element_detail(&l.id).unwrap();
        assert!(d.note.is_none());
        assert_eq!(number(&d.data[0].fields, "Load", "MW"), l.p_mw);
    }
}

#[test]
fn shunts_facts_and_switched_shunts_have_their_own_ids() {
    use serde_json::{Value, json};
    let s = stannum_core::commands::Session::new();
    let call = |command: &str, args: Value| -> Value {
        serde_json::from_str(&s.invoke(command, args).unwrap()).unwrap()
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/five_bus_devices.raw");
    call("open_case", json!({ "path": path }));
    // The Records rows carry the ids.
    for (section, id) in [
        ("FIXED SHUNT", "shunt:4:1"),
        ("SWITCHED SHUNT", "switched-shunt:5:1"),
        ("FACTS DEVICE", "facts:STATCOM_3"),
    ] {
        let table = call("record_table", json!({ "section": section }));
        assert_eq!(table["rows"]["element"][0], id, "{section}");
    }
    // The inspector reads them from their records.
    let shunt = call("element_detail", json!({ "element": "switched-shunt:5:1" }));
    assert_eq!(shunt["kind"], "switched-shunt");
    assert_eq!(shunt["buses"][0]["number"], 5);
    assert!(shunt["online"].as_bool().unwrap());
    let facts = call("element_detail", json!({ "element": "facts:STATCOM_3" }));
    assert_eq!(
        facts["buses"].as_array().unwrap().len(),
        1,
        "J = 0 is no bus"
    );
    // A bus lists them once the records are read.
    let bus = call("element_detail", json!({ "element": "bus:5" }));
    assert!(
        bus["connected"]
            .as_array()
            .unwrap()
            .contains(&json!("switched-shunt:5:1"))
    );
    // Every kind's id is listed with where it comes from.
    let kinds = call("element_kinds", json!({}));
    let shunt_kind = kinds
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["kind"] == "switched-shunt")
        .unwrap();
    assert_eq!(shunt_kind["raw_sections"], json!(["SWITCHED SHUNT"]));
    assert_eq!(shunt_kind["inspector"], "record");
    let missing = s
        .invoke("element_detail", json!({ "element": "facts:NOPE" }))
        .unwrap_err();
    assert_eq!(missing.kind, stannum_core::ErrorKind::NotFound);
}
