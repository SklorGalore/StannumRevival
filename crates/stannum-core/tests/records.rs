//! The Records module's tables: every record type with every field.
use stannum_core::Workspace;
use stannum_core::record_edit::RecordEdit;
use stannum_core::records::{ColumnKind, RecordSource};
use std::path::PathBuf;

fn open(path: PathBuf) -> Workspace {
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    ws
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn raw_records_keep_every_field_and_link_to_elements() {
    let mut ws = open(fixture("five_bus.raw"));
    let case = ws.current_mut().unwrap();
    let book = case.record_book().unwrap();
    assert_eq!(book.source, RecordSource::Raw);
    let ids: Vec<&str> = book.sections.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.first(), Some(&"BUS"), "{ids:?}");
    assert_eq!(ids.last(), Some(&"CASE DETAILS"), "{ids:?}");
    assert!(ids.contains(&"LOAD") && ids.contains(&"GENERATOR") && ids.contains(&"BRANCH"));

    let buses = case.record_table("BUS").unwrap();
    let names: Vec<&str> = buses.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(&names[..4], ["I", "NAME", "BASKV", "IDE"]);
    assert_eq!(buses.columns[0].kind, ColumnKind::Integer);
    assert_eq!(buses.columns[1].kind, ColumnKind::Text);
    assert_eq!(buses.columns[2].kind, ColumnKind::Number);
    let first = &buses.rows[0];
    assert_eq!(first.cells.len(), buses.columns.len());
    assert!(first.line > 0);
    let number = first.cells[0].as_u64().unwrap();
    assert_eq!(
        first.element.as_deref(),
        Some(format!("bus:{number}").as_str())
    );
    // Names are trimmed of the RAW padding.
    let name = first.cells[1].as_str().unwrap();
    assert_eq!(name, name.trim());

    let loads = case.record_table("LOAD").unwrap();
    assert!(
        loads
            .rows
            .iter()
            .all(|r| r.element.as_deref().is_some_and(|e| e.starts_with("load:")))
    );
    let branches = case.record_table("BRANCH").unwrap();
    assert!(
        branches
            .rows
            .iter()
            .all(|r| r.element.as_deref().is_some_and(|e| e.starts_with("line:")))
    );
    // Every linked element is one the inspector knows.
    for row in loads.rows.iter().chain(&branches.rows).chain(&buses.rows) {
        case.element_detail(row.element.as_deref().unwrap())
            .unwrap();
    }
    assert!(case.record_table("NOPE").is_err());
}

#[test]
fn matpower_records_come_from_the_model() {
    let mut ws = open(fixture("case9.m"));
    let case = ws.current_mut().unwrap();
    let book = case.record_book().unwrap();
    assert_eq!(book.source, RecordSource::Model);
    assert!(book.note.is_none());
    let ids: Vec<&str> = book.sections.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(&ids[..3], ["bus", "gen", "branch"]);
    let buses = case.record_table("bus").unwrap();
    assert_eq!(buses.rows.len(), 9);
    assert_eq!(buses.columns[0].name, "BUS_I");
    assert_eq!(buses.columns[0].kind, ColumnKind::Integer);
    let gens = case.record_table("gen").unwrap();
    assert_eq!(gens.rows.len(), 3);
    assert_eq!(gens.columns[7].name, "GEN_STATUS");
    for row in gens
        .rows
        .iter()
        .chain(&case.record_table("branch").unwrap().rows)
    {
        case.element_detail(row.element.as_deref().unwrap())
            .unwrap();
    }
}

/// A large case from tests/large (or STANNUM_LARGE_CASES), when present.
fn large(name: &str) -> Option<PathBuf> {
    let dir = std::env::var("STANNUM_LARGE_CASES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/large"));
    let path = dir.join(name);
    path.exists().then_some(path)
}

#[test]
#[ignore = "needs tests/large/SyntheticUSA.RAW"]
fn synthetic_usa_lists_every_record() {
    let Some(path) = large("SyntheticUSA.RAW") else {
        return;
    };
    let mut ws = open(path);
    let case = ws.current_mut().unwrap();
    let start = std::time::Instant::now();
    let book = case.record_book().unwrap();
    eprintln!("records read in {:?}", start.elapsed());
    for s in &book.sections {
        eprintln!("{:<26} {:>7} rows, {} skipped", s.label, s.count, s.skipped);
    }
    assert_eq!(book.source, RecordSource::Raw);
    let count = |id: &str| book.sections.iter().find(|s| s.id == id).unwrap().count;
    // The model's 82,000 buses include a star bus per three-winding transformer.
    assert_eq!(count("BUS"), 79_600);
    // Read leniently: fixed shunts (layout not verified for revision 33) and
    // three-winding transformers are listed too.
    assert_eq!(count("FIXED SHUNT"), 916);
    assert_eq!(count("TRANSFORMER"), 13_496 + 2_400);
    let transformers = case.record_table("TRANSFORMER").unwrap();
    let three: Vec<_> = transformers
        .rows
        .iter()
        .filter(|r| {
            r.element
                .as_deref()
                .is_some_and(|e| e.split(':').count() == 5)
        })
        .collect();
    assert_eq!(three.len(), 2_400);
    for row in three.iter().take(50) {
        case.element_detail(row.element.as_deref().unwrap())
            .unwrap();
    }
    assert!(
        transformers
            .note
            .as_deref()
            .unwrap()
            .contains("three-winding")
    );
}

fn col(table: &stannum_core::records::RecordTable, name: &str) -> usize {
    table.columns.iter().position(|c| c.name == name).unwrap()
}

fn edit(section: &str, row: usize, column: usize, value: &str) -> RecordEdit {
    RecordEdit {
        section: section.into(),
        row,
        column,
        value: value.into(),
    }
}

#[test]
fn raw_edits_rewrite_only_the_edited_fields_and_reach_power_flow() {
    let mut ws = open(fixture("five_bus.raw"));
    let case = ws.current_mut().unwrap();
    let loads = case.record_table("LOAD").unwrap();
    let buses = case.record_table("BUS").unwrap();
    let (pl, name, id) = (col(&loads, "PL"), col(&buses, "NAME"), col(&buses, "I"));
    assert!(!buses.columns[id].editable, "key fields stay as read");
    assert!(buses.columns[name].editable);
    let before = String::from_utf8(case.bytes.to_vec()).unwrap();

    let summary = case
        .apply_record_edits(&[
            edit("LOAD", 0, pl, "95.5"),
            edit("BUS", 4, name, "Spur_South"),
        ])
        .unwrap();
    assert_eq!(summary.record_edits, 2);
    let after = String::from_utf8(case.bytes.to_vec()).unwrap();
    let changed: Vec<(&str, &str)> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 2, "{changed:?}");
    // Width and the rest of the line are kept.
    assert_eq!(
        changed[0].1,
        "     5,'Spur_South  ',  69.0000,1,   2,   1,   1,0.99000,  -5.0000,1.10000,0.90000,1.10000,0.90000"
    );
    assert!(
        changed[1]
            .1
            .starts_with("    2,'1 ',1,   1,   1,      95.5,    30.000,"),
        "{}",
        changed[1].1
    );
    // The tables and the power flow model both see the new values.
    assert_eq!(
        case.record_table("LOAD").unwrap().rows[0].cells[pl],
        serde_json::json!(95.5)
    );
    let pd = case
        .power
        .as_ref()
        .unwrap()
        .case
        .buses
        .iter()
        .find(|b| b.id == 2)
        .unwrap()
        .pd;
    assert!((pd - 95.5).abs() < 1e-9, "{pd}");

    // Bad values and key fields are refused, and nothing changes.
    assert!(
        case.apply_record_edits(&[edit("LOAD", 0, pl, "lots")])
            .is_err()
    );
    assert!(case.apply_record_edits(&[edit("BUS", 0, id, "7")]).is_err());
    assert_eq!(String::from_utf8(case.bytes.to_vec()).unwrap(), after);

    // Save as writes the edited case, which reads back the same.
    let dir = std::env::temp_dir().join(format!("stannum-records-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let saved = dir.join("five_bus_edited.raw");
    let summary = case.save_as(&saved).unwrap();
    assert_eq!(summary.record_edits, 0);
    assert_eq!(summary.name, "five_bus_edited.raw");
    assert_eq!(std::fs::read_to_string(&saved).unwrap(), after);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn matpower_edits_change_the_model() {
    let mut ws = open(fixture("case9.m"));
    let case = ws.current_mut().unwrap();
    let gens = case.record_table("gen").unwrap();
    let (pg, status, bus) = (
        col(&gens, "PG"),
        col(&gens, "GEN_STATUS"),
        col(&gens, "GEN_BUS"),
    );
    assert!(!gens.columns[bus].editable);
    case.apply_record_edits(&[edit("gen", 1, pg, "150"), edit("gen", 2, status, "0")])
        .unwrap();
    let model = &case.power.as_ref().unwrap().case;
    assert_eq!(model.generators[1].pg, 150.0);
    assert!(!model.generators[2].online);
    assert_eq!(
        case.record_table("gen").unwrap().rows[1].cells[pg].as_f64(),
        Some(150.0)
    );
    assert!(
        case.apply_record_edits(&[edit("gen", 0, bus, "4")])
            .is_err()
    );
}

#[test]
fn a_record_edit_keeps_the_last_solve_as_the_starting_point() {
    use stannum_core::power::{SolveSettings, StartChoice};
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rs-power/tests/fixtures/case300.m");
    let mut ws = open(path);
    let case = ws.current_mut().unwrap();
    let flat = SolveSettings {
        start: StartChoice::Flat,
        tolerance_mva: 0.01,
        ..SolveSettings::default()
    };
    let last = SolveSettings {
        start: StartChoice::Last,
        ..flat.clone()
    };
    assert!(case.solve(flat.clone()).unwrap().converged);
    let buses = case.record_table("bus").unwrap();
    let pd = col(&buses, "PD");
    let row = 4;
    let old: f64 = buses.rows[row].cells[pd].as_f64().unwrap();
    case.apply_record_edits(&[edit("bus", row, pd, &format!("{}", old + 5.0))])
        .unwrap();
    // The edited case is read again, but its topology waits until asked for.
    assert!(!case.topology_built());
    let warm = case.solve(last).unwrap();
    let cold = case.solve(flat).unwrap();
    assert!(warm.converged && cold.converged);
    assert!(
        warm.iterations < cold.iterations,
        "from the last solve: {} iterations, from flat: {}",
        warm.iterations,
        cold.iterations
    );
}
