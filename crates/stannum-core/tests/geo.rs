//! Bus coordinates from a CSV side file, through the command list.
use serde_json::{Value, json};
use stannum_core::commands::Session;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .display()
        .to_string()
}

fn invoke(s: &Session, command: &str, args: Value) -> Value {
    serde_json::from_str(&s.invoke(command, args).unwrap()).unwrap()
}

#[test]
fn attaches_places_and_estimates_buses() {
    let s = Session::new();
    s.invoke("open_case", json!({ "path": fixture("five_bus.raw") }))
        .unwrap();
    assert_eq!(
        s.invoke("bus_coordinates", json!({})).unwrap_err(),
        "Attach a bus coordinates file first."
    );

    let summary = invoke(
        &s,
        "load_coordinates",
        json!({ "path": fixture("five_bus.coordinates.csv") }),
    );
    assert_eq!(summary["rows"], 4);
    assert_eq!(summary["from_file"], 3);
    assert_eq!(summary["estimated"], 2);
    assert_eq!(summary["missing"], 0);
    assert_eq!(summary["unknown_buses"], 1);
    assert_eq!(summary["problem_count"], 1);
    assert_eq!(summary["problems"][0]["line"], 6);

    let case = invoke(&s, "case_summary", json!({}));
    assert_eq!(case["coordinates"], summary);

    let c = invoke(&s, "bus_coordinates", json!({}));
    assert_eq!(c["bus"], json!([1, 2, 3, 4, 5]));
    assert_eq!(
        c["source"],
        json!(["file", "file", "file", "estimated", "estimated"])
    );
    assert_eq!(c["substation"][1], "Waco");
    // Bus 4 hangs off bus 2 only, and bus 5 off bus 4.
    assert_eq!(c["latitude"][3], c["latitude"][1]);
    assert_eq!(c["longitude"][4], c["longitude"][1]);

    let bus4 = invoke(&s, "element_detail", json!({ "element": "bus:4" }));
    let location = bus4["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["title"] == "Location")
        .expect("a Location group");
    assert!(location.to_string().contains("\"estimated\""));

    s.invoke("clear_coordinates", json!({})).unwrap();
    assert_eq!(
        invoke(&s, "case_summary", json!({}))["coordinates"],
        Value::Null
    );
}

#[test]
fn a_bus_with_nothing_placed_in_its_island_has_no_place() {
    let s = Session::new();
    s.invoke("open_case", json!({ "path": fixture("case9.m") }))
        .unwrap();
    let dir = std::env::temp_dir().join(format!("stannum-geo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("one.csv");
    std::fs::write(&path, "bus,lat,lon\n1,40,-100\n").unwrap();
    let summary = invoke(&s, "load_coordinates", json!({ "path": path }));
    // case9 is one island, so every bus gets an estimate from bus 1.
    assert_eq!(summary["from_file"], 1);
    assert_eq!(summary["estimated"], 8);
    assert_eq!(summary["missing"], 0);

    std::fs::write(&path, "number,latitude\n1,40\n").unwrap();
    let err = s
        .invoke("load_coordinates", json!({ "path": path }))
        .unwrap_err();
    assert!(err.message.contains("bus, latitude and longitude"), "{err}");
    // A bad file leaves the attached one in place.
    assert_eq!(
        invoke(&s, "case_summary", json!({}))["coordinates"]["from_file"],
        1
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_raw_file_places_its_own_substations() {
    let s = Session::new();
    s.invoke(
        "open_case",
        json!({ "path": fixture("five_bus_substations.raw") }),
    )
    .unwrap();
    // No file attached: the case's own substations place buses 1, 2 and 4;
    // East is at 0, 0 (no place) and bus 5 is in no substation.
    let summary = invoke(&s, "case_summary", json!({}))["coordinates"].clone();
    assert_eq!(summary["path"], Value::Null);
    assert_eq!(summary["name"], "five_bus_substations.raw");
    assert_eq!(summary["from_case"], 3);
    assert_eq!(summary["case_substations"], 2);
    assert_eq!(summary["from_file"], 0);
    assert_eq!(summary["estimated"], 2);
    let c = invoke(&s, "bus_coordinates", json!({}));
    assert_eq!(
        c["source"],
        json!(["case", "case", "estimated", "case", "estimated"])
    );
    assert_eq!(
        c["substation"],
        json!(["North", "Central", null, "Central", null])
    );
    assert_eq!(c["latitude"][1], 31.5493);
    let bus2 = invoke(&s, "element_detail", json!({ "element": "bus:2" }));
    assert!(bus2.to_string().contains("from case substation"));

    // An attached file wins where it places a bus; the case fills the rest.
    let summary = invoke(
        &s,
        "load_coordinates",
        json!({ "path": fixture("five_bus.coordinates.csv") }),
    );
    assert_eq!(summary["from_file"], 3);
    assert_eq!(summary["from_case"], 1);
    assert_eq!(summary["estimated"], 1);
    let c = invoke(&s, "bus_coordinates", json!({}));
    assert_eq!(
        c["source"],
        json!(["file", "file", "file", "case", "estimated"])
    );
    assert_eq!(c["substation"][1], "Waco");
    assert_eq!(c["substation"][3], "Central");

    // Detaching the file goes back to the case's own places.
    s.invoke("clear_coordinates", json!({})).unwrap();
    let summary = invoke(&s, "case_summary", json!({}))["coordinates"].clone();
    assert_eq!(summary["path"], Value::Null);
    assert_eq!(summary["from_case"], 3);
}
