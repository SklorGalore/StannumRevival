//! The compact topology snapshot (`topology_wire`) and the full one it
//! stands for.
//!
//! `src/lib/analysis/__fixtures__/topology-wire.json` holds both forms for a
//! few small cases, and `topologyWire.test.ts` checks that the app rebuilds
//! the full form from the compact one. This test keeps that file current:
//! run it with `STANNUM_WRITE_FIXTURES=1` to rewrite it after a change.

use serde_json::{Value, json};
use stannum_core::Workspace;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The full snapshot as the app rebuilds it: source fields reduced to line
/// and section, names and circuits trimmed.
fn expected(mut full: Value) -> Value {
    let case = &mut full["case"];
    for table in ["buses", "equipment", "loads", "generators"] {
        for row in case[table].as_array_mut().unwrap() {
            let source = &row["source"];
            row["source"] = json!({ "line": source["line"], "section": source["section"] });
            for key in ["name", "circuit"] {
                if let Some(s) = row[key].as_str() {
                    row[key] = json!(s.trim());
                }
            }
        }
    }
    full
}

fn both(path: &Path, merge: bool) -> Value {
    let mut ws = Workspace::default();
    ws.open(path).unwrap();
    let case = ws.current_mut().unwrap();
    let view = if merge {
        Some(case.set_merge_zero_impedance_wire(true).unwrap())
    } else {
        None
    };
    let full = serde_json::to_value(case.topology_snapshot().unwrap()).unwrap();
    let wire = case.topology_wire().unwrap();
    if let Some(view) = view {
        assert_eq!(
            view["graph"], wire["graph"],
            "the merge answer is the new graph"
        );
        assert_eq!(view["analysis"], wire["analysis"]);
    }
    json!({ "wire": wire, "expected": expected(full) })
}

#[test]
fn fixtures_match_the_wire_form() {
    let fixtures = root().join("crates/stannum-core/tests/fixtures");
    // five_bus with its 69 kV line made zero-impedance, so merging has
    // something to merge.
    let dir = std::env::temp_dir().join(format!("stannum-topology-wire-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let zero = dir.join("five_bus_zero.raw");
    let text = std::fs::read_to_string(fixtures.join("five_bus.raw")).unwrap();
    let line = "     4,     5,'1 ',2.00000E-02,5.00000E-02,";
    assert!(text.contains(line));
    std::fs::write(
        &zero,
        text.replace(line, "     4,     5,'1 ',0.00000E+00,0.00000E+00,"),
    )
    .unwrap();
    // five_bus with a three-winding transformer to a new 13.8 kV bus, so the
    // graph has a junction node.
    let three = dir.join("five_bus_three_winding.raw");
    let bus = "     5,'Spur_69     ',  69.0000,1,   2,   1,   1,0.99000,  -5.0000,1.10000,0.90000,1.10000,0.90000";
    let transformer = "0 / END OF TRANSFORMER DATA";
    assert!(text.contains(bus) && text.contains(transformer));
    std::fs::write(
        &three,
        text.replace(
            bus,
            &format!("{bus}\n     6,'Tertiary    ',  13.8000,1,   1,   1,   1,1.00000,  -4.0000,1.10000,0.90000,1.10000,0.90000"),
        )
        .replace(
            transformer,
            &format!(
                "     2,     4,    6,'T ',1,1,1,0.00000E-1,0.00000E-1,2,'            ', 1,   1,1.0000,   0,1.0000,   0,1.0000,   0,1.0000,'            '\n\
                 3.82000E-3,1.12620E-1, 100.00,3.82000E-3,1.12620E-1, 100.00,3.82000E-3,1.12620E-1, 100.00,1.00000,   0.000\n\
                 1.000000,138.000,   0.000, 146.60, 146.60, 146.60,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00, 0,     0,   0,1.500000,0.510000,1.500000,0.510000,159, 0, 0.00000, 0.00000,  0.000\n\
                 1.000000, 69.000,   0.000, 146.60, 146.60, 146.60,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00, 0,     0,   0,1.500000,0.510000,1.500000,0.510000,159, 0, 0.00000, 0.00000,  0.000\n\
                 1.000000, 13.800,   0.000, 146.60, 146.60, 146.60,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00,   0.00, 0,     0,   0,1.500000,0.510000,1.500000,0.510000,159, 0, 0.00000, 0.00000,  0.000\n\
                 {transformer}"
            ),
        ),
    )
    .unwrap();

    let cases = json!([
        { "name": "five_bus", "case": both(&fixtures.join("five_bus.raw"), false) },
        { "name": "five_bus zero-impedance", "case": both(&zero, false) },
        { "name": "five_bus zero-impedance merged", "case": both(&zero, true) },
        { "name": "five_bus three-winding", "case": both(&three, false) },
        {
            "name": "derived topology, open three-winding transformer",
            "case": both(&root().join("crates/rs-power/tests/fixtures/raw/transformers.raw"), false),
        },
        { "name": "case9", "case": both(&fixtures.join("case9.m"), false) },
    ]);
    std::fs::remove_dir_all(&dir).ok();
    let merged = &cases[2]["case"]["wire"]["graph"];
    assert_eq!(merged["merge_zero_impedance"], json!(true));
    assert!(
        !merged["nodes"]["members"].as_array().unwrap().is_empty(),
        "a merged node"
    );
    let three = &cases[3]["case"]["wire"]["graph"]["nodes"]["equipment"];
    assert!(
        three.as_array().unwrap().iter().any(|e| !e.is_null()),
        "a junction"
    );

    let text = serde_json::to_string_pretty(&cases).unwrap() + "\n";
    let file = root().join("src/lib/analysis/__fixtures__/topology-wire.json");
    if std::env::var_os("STANNUM_WRITE_FIXTURES").is_some() {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, text).unwrap();
    } else {
        let kept = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(
            kept == text,
            "{} is out of date: run this test with STANNUM_WRITE_FIXTURES=1",
            file.display()
        );
    }
}

#[test]
fn source_fields_come_one_element_at_a_time() {
    let mut ws = Workspace::default();
    ws.open(&root().join("crates/stannum-core/tests/fixtures/five_bus.raw"))
        .unwrap();
    let case = ws.current().unwrap();
    let bus = case.topology_source("bus:2").unwrap().unwrap();
    assert_eq!(bus.section, "BUS");
    assert!(bus.line > 0);
    assert!(!bus.raw_lines.is_empty());
    let line = case.topology_source("line:1:2:1").unwrap().unwrap();
    assert!(line.fields.len() > 3, "{:?}", line.fields);
    assert!(case.topology_source("bus:99").unwrap().is_none());
    assert!(case.topology_source("nothing").unwrap().is_none());
}
