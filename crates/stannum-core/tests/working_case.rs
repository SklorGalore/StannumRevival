//! Case diff of the working case: the study's edits written into the RAW
//! records they change, compared with the file.
use stannum_core::Workspace;
use stannum_core::study::Edit;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn field<'a>(
    detail: &'a stannum_core::diff::RecordDetail,
    name: &str,
) -> &'a stannum_core::diff::FieldRow {
    detail
        .fields
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no field {name}"))
}

#[test]
fn study_edits_show_as_record_changes() {
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus.raw")).unwrap();
    // Nothing edited: the working case is the file.
    assert_eq!(ws.compare_working().unwrap().differing, 0);

    ws.current_mut()
        .unwrap()
        .set_edits(vec![
            Edit::Status {
                element: "line:4:5:1".into(),
                online: false,
            },
            Edit::Generation {
                element: "generator:3:1".into(),
                mw: 95.5,
            },
            Edit::Load {
                bus: 2,
                mw: 99.0,
                mvar: 30.0,
            },
            Edit::Status {
                element: "transformer:2:4:1".into(),
                online: false,
            },
        ])
        .unwrap();
    // The working text, read back, is the edited case.
    let working = ws.current_mut().unwrap().working_bytes().unwrap();
    let reread = stannum_core::workspace::OpenCase::from_bytes(&fixture("five_bus.raw"), &working);
    let power = reread.power.unwrap();
    let bus2 = power.case.buses.iter().find(|b| b.id == 2).unwrap();
    assert!((bus2.load().0 - 99.0).abs() < 1e-9);
    let line = power
        .elements
        .branches
        .iter()
        .position(|e| e.element == "line:4:5:1")
        .unwrap();
    assert!(!power.case.branches[line].online);

    let summary = ws.compare_working().unwrap();
    assert!(summary.comparison.name.ends_with("(working)"));
    assert_eq!(summary.differing, 4, "{summary:?}");
    let changed = |section: &str| {
        summary
            .sections
            .iter()
            .find(|s| s.name == section)
            .map_or(0, |s| s.counts.changed)
    };
    assert_eq!(changed("BRANCH"), 1);
    assert_eq!(changed("GENERATOR"), 1);
    assert_eq!(changed("LOAD"), 1);
    assert_eq!(changed("TRANSFORMER"), 1);

    let comparison = ws.comparison().unwrap();
    let row = |section: &str, element: &str| {
        comparison
            .section_records(section)
            .into_iter()
            .find(|r| r.element.as_deref() == Some(element))
            .unwrap_or_else(|| panic!("no {element}"))
            .index
    };
    let branch = comparison
        .record_detail("BRANCH", row("BRANCH", "line:4:5:1"))
        .unwrap();
    assert_eq!(field(&branch, "STAT").before.as_deref(), Some("1"));
    assert_eq!(field(&branch, "STAT").after.as_deref(), Some("0"));
    let gen_detail = comparison
        .record_detail("GENERATOR", row("GENERATOR", "generator:3:1"))
        .unwrap();
    assert_eq!(field(&gen_detail, "PG").after.as_deref(), Some("95.5"));
    let load = comparison
        .record_detail("LOAD", row("LOAD", "load:2:1"))
        .unwrap();
    assert_eq!(field(&load, "PL").after.as_deref(), Some("99"));
    // Mvar was unchanged, so QL is left alone.
    assert_eq!(
        field(&load, "QL").status,
        stannum_core::diff::Status::Unchanged
    );
}

#[test]
fn matpower_cases_are_refused_plainly() {
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let err = ws.compare_working().unwrap_err();
    assert!(err.message.contains("MATPOWER"), "{err}");
    assert_eq!(err.kind, stannum_core::ErrorKind::Unsupported);
}
