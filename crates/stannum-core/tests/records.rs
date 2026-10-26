//! The Records module's tables: every record type with every field.
use stannum_core::Workspace;
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
