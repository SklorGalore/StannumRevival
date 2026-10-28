//! RAW 33 and 35 export, checked by reading what it wrote.
//!
//! Every export is read back by case-diff's parser (strictly, as Case diff
//! does) and compared record by record with the case it came from, and by
//! rs-power, whose solution must match the original's.

use stannum_core::Workspace;
use stannum_core::diff::{self, DiffCase, Status, parse_bytes};
use stannum_core::power::SolveSettings;
use stannum_core::raw_export::RawExport;
use stannum_core::record_edit::RecordEdit;
use stannum_core::study::Edit;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn rs_power_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs-power/tests/fixtures")
        .join(name)
}

fn out_dir(test: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("stannum-raw-export-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Open `path` and export it as `revision` into `dir`.
fn export(path: &Path, revision: u32, dir: &Path) -> (PathBuf, RawExport) {
    let mut ws = Workspace::default();
    ws.open(path).unwrap();
    export_open(&mut ws, path, revision, dir)
}

fn export_open(ws: &mut Workspace, path: &Path, revision: u32, dir: &Path) -> (PathBuf, RawExport) {
    let stem = path.file_stem().unwrap().to_string_lossy();
    let out = dir.join(format!("{stem}_v{revision}.raw"));
    let report = ws
        .current_mut()
        .unwrap()
        .export_raw(&out, revision)
        .unwrap();
    assert_eq!(report.revision, revision);
    (out, report)
}

fn read(path: &Path) -> DiffCase {
    let bytes = std::fs::read(path).unwrap();
    parse_bytes(&path.display().to_string(), &bytes)
        .unwrap_or_else(|e| panic!("case-diff can't read the export: {e}"))
}

/// Every difference between two cases, as "SECTION key: FIELD" or
/// "SECTION key added/removed".
fn changes(before: DiffCase, after: DiffCase) -> BTreeSet<String> {
    let c = diff::compare_cases(before, after);
    let mut out = BTreeSet::new();
    for (section, records) in &c.sections {
        let rows = c.section_records(section);
        for (i, r) in records.iter().enumerate() {
            match r.status {
                Status::Unchanged => {}
                Status::Added | Status::Removed => {
                    let key = rows[i].key.clone();
                    out.insert(format!("{section} {key} {:?}", r.status));
                }
                Status::Changed => {
                    let key = rows[i].key.clone();
                    for f in &r.fields {
                        if f.status != Status::Unchanged {
                            out.insert(format!("{section} {key}: {}", f.name));
                        }
                    }
                }
            }
        }
    }
    out
}

/// The lines the reader keeps without decoding them, by section.
fn kept_lines(case: &DiffCase) -> Vec<(String, Vec<String>)> {
    case.diagnostics
        .iter()
        .filter(|d| !d.raw_lines.is_empty())
        .map(|d| (d.section.clone(), d.raw_lines.clone()))
        .collect()
}

/// Bus voltages after a solve with a tight tolerance.
fn solve(path: &Path) -> Vec<(u32, f64, f64)> {
    let mut ws = Workspace::default();
    ws.open(path).unwrap();
    let settings = SolveSettings {
        tolerance_mva: 1e-6,
        ..Default::default()
    };
    let report = ws.current_mut().unwrap().solve(settings).unwrap();
    assert!(report.converged, "{} didn't solve", path.display());
    let mut buses: Vec<(u32, f64, f64)> = report.buses.iter().map(|b| (b.id, b.vm, b.va)).collect();
    buses.sort_by_key(|b| b.0);
    buses
}

fn assert_same_solution(a: &Path, b: &Path) {
    let (x, y) = (solve(a), solve(b));
    // Star buses of three-winding transformers are numbered by the reader.
    assert_eq!(x.len(), y.len(), "{} and {}", a.display(), b.display());
    for (p, q) in x.iter().zip(&y) {
        assert_eq!(p.0, q.0);
        assert!(
            (p.1 - q.1).abs() < 1e-6 && (p.2 - q.2).abs() < 1e-4,
            "bus {}: {:?} in {} but {:?} in {}",
            p.0,
            (p.1, p.2),
            a.display(),
            (q.1, q.2),
            b.display()
        );
    }
}

const RAW_35: &[&str] = &[
    "five_bus.raw",
    "five_bus_devices.raw",
    "five_bus_dc.raw",
    "five_bus_zip.raw",
    "five_bus_modified.raw",
    "raw_export/five_bus_everything.raw",
];

#[test]
fn the_same_revision_writes_every_record_unchanged() {
    let dir = out_dir("same");
    for name in RAW_35 {
        let path = fixture(name);
        let (out, report) = export(&path, 35, &dir);
        assert_eq!(report.source, "records");
        assert!(report.notes.is_empty(), "{name}: {:?}", report.notes);
        let (before, after) = (read(&path), read(&out));
        assert_eq!(kept_lines(&before), kept_lines(&after), "{name}");
        assert_eq!(changes(before, after), BTreeSet::new(), "{name}");
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn revision_33_and_back_loses_only_what_33_has_no_place_for() {
    let dir = out_dir("down-up");
    for name in RAW_35 {
        let path = fixture(name);
        let (v33, report) = export(&path, 33, &dir);
        assert!(
            report.notes[0].contains("Converted from RAW revision 35"),
            "{name}"
        );
        let case33 = read(&v33);
        assert_eq!(case33.revision, 33);
        let up_dir = dir.join("up");
        std::fs::create_dir_all(&up_dir).unwrap();
        let (v35, _) = export(&v33, 35, &up_dir);
        let lost = changes(read(&path), read(&v35));
        let allowed = |c: &str| {
            // System-wide data and the 35-only fields of the records below.
            c.starts_with("CASE DETAILS")
                || c.ends_with(": BASLOD")
                || c.ends_with(": NAME") && c.starts_with("BRANCH")
                || c.starts_with("BRANCH")
                    && c.rsplit_once(": RATE")
                        .is_some_and(|(_, n)| n.parse::<u32>().is_ok_and(|n| n >= 4))
                || c.starts_with("LOAD 4 / '1'")
                || c.starts_with("LOAD") && (c.ends_with(": DGENM") || c.ends_with(": LOADTYPE"))
                || c.starts_with("SWITCHED SHUNT 5 / '2'")
                || c.starts_with("SYSTEM SWITCHING DEVICE 3 / 5 / '1': NSTAT")
                || c.starts_with("SYSTEM SWITCHING DEVICE") && c.ends_with(": NAME")
        };
        let unexpected: Vec<&String> = lost.iter().filter(|c| !allowed(c)).collect();
        assert!(unexpected.is_empty(), "{name}: {unexpected:#?}");
        // Revision 33 holds everything a 33 file had: the way up and down
        // again loses nothing.
        let down_dir = dir.join("down");
        std::fs::create_dir_all(&down_dir).unwrap();
        let (again, _) = export(&v35, 33, &down_dir);
        let (a, b) = (read(&v33), read(&again));
        assert_eq!(kept_lines(&a), kept_lines(&b), "{name}");
        assert_eq!(changes(a, b), BTreeSet::new(), "{name}");
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn converted_records_keep_their_meaning() {
    let dir = out_dir("meaning");
    let path = fixture("raw_export/five_bus_everything.raw");
    let (v33, report) = export(&path, 33, &dir);
    let notes = report.notes.join("\n");
    for expected in [
        "System-wide data dropped",
        "Branch names dropped",
        "Branch ratings 4 to 12 dropped",
        "Load distributed generation netted",
        "System switching devices written as branches",
        "Extra switched shunts on a bus written as fixed shunts",
        "Impedance correction tables rewritten for revision 33",
        "Imaginary parts of impedance correction factors dropped",
    ] {
        assert!(notes.contains(expected), "no note {expected:?} in\n{notes}");
    }
    let case = read(&v33);
    let record = |section: &str, key: &str| {
        let (_, records) = case.sections.iter().find(|(s, _)| s == section).unwrap();
        records
            .iter()
            .find(|r| {
                r.key
                    .iter()
                    .map(|v| v.display())
                    .collect::<Vec<_>>()
                    .join(" / ")
                    == key
            })
            .unwrap_or_else(|| panic!("no {section} {key}"))
            .clone()
    };
    let field = |r: &diff::parser::Record, name: &str| r.field(name).unwrap().display();
    // Rating A is rating 1.
    let line = record("BRANCH", "1 / 2 / '1'");
    assert_eq!(field(&line, "RATEA"), "150.00");
    // 40 MW of load less 5 MW of distributed generation.
    let load = record("LOAD", "4 / '1'");
    assert_eq!(field(&load, "PL"), "35.0");
    assert_eq!(field(&load, "QL"), "11.0");
    // The open breaker is an open branch marked @.
    let breaker = record("BRANCH", "3 / 5 / '@1'");
    assert_eq!(field(&breaker, "STAT"), "0");
    assert_eq!(field(&breaker, "X"), "0.000100000");
    // One switched shunt per bus: the first, with its block.
    let shunt = record("SWITCHED SHUNT", "5");
    assert_eq!(field(&shunt, "SWREM"), "5");
    assert_eq!(field(&shunt, "N1"), "2");
    let text = std::fs::read_to_string(&v33).unwrap();
    assert!(
        text.contains("\n5,'2',1,0.0,-5.00\n"),
        "the second shunt as a fixed shunt"
    );
    assert!(
        text.contains("\n1,-30.000,1.10000,0.000,1.00000,30.000,1.10000,0.0,0.0,"),
        "{text}"
    );

    // And back up: the breaker is a switching device again, the shunt's
    // block is in service, and the table is in triples ended by zeros.
    let (v35, _) = export(&v33, 35, &dir);
    let text = std::fs::read_to_string(&v35).unwrap();
    assert!(text.contains("\n3,5,'1 ',0.000100000,100.00,"), "{text}");
    assert!(
        text.contains("\n5,'1 ',1,0,1,1.05000,0.95000,5,0,100.0,'            ',10.00,1,2,10.00\n")
    );
    assert!(
        text.contains("\n1,-30.000,1.10000,0.0,0.000,1.00000,0.0,30.000,1.10000,0.0,0.0,0.0,0.0\n")
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn rs_power_solves_every_export_the_same() {
    let dir = out_dir("solve");
    // Not five_bus_everything: rs-power leaves out RAW 35 distributed
    // generation, which revision 33 nets from the load.
    for name in [
        "five_bus.raw",
        "five_bus_devices.raw",
        "five_bus_dc.raw",
        "five_bus_zip.raw",
    ] {
        let path = fixture(name);
        for revision in [33, 35] {
            let (out, _) = export(&path, revision, &dir);
            assert_same_solution(&path, &out);
        }
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn substation_data_is_kept_in_35_and_dropped_in_33() {
    let dir = out_dir("substations");
    let path = fixture("five_bus_substations.raw");
    let (v35, report) = export(&path, 35, &dir);
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    let original = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\r\n", "\n");
    let written = std::fs::read_to_string(&v35).unwrap();
    let tail = |t: &str| t[t.find("BEGIN SUBSTATION DATA").unwrap()..].to_string();
    assert_eq!(tail(&written), tail(&original));
    let substations = |p: &Path| {
        let text = std::fs::read_to_string(p).unwrap();
        stannum_core::power::load(&text, false)
            .unwrap()
            .substations
            .len()
    };
    assert_eq!(substations(&v35), substations(&path));
    let (v33, report) = export(&path, 33, &dir);
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("Substation (node-breaker) data dropped"))
    );
    assert!(
        !std::fs::read_to_string(&v33)
            .unwrap()
            .contains("SUBSTATION")
    );
    read(&v33);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn study_and_records_edits_are_in_the_export() {
    let dir = out_dir("edits");
    let path = fixture("five_bus.raw");
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    let case = ws.current_mut().unwrap();
    // A Records edit: bus 5's name.
    let records = case.records().unwrap().clone();
    let bus = records.table("BUS").unwrap();
    let column = bus.columns.iter().position(|c| c.name == "NAME").unwrap();
    case.apply_record_edits(&[RecordEdit {
        section: "BUS".into(),
        row: 4,
        column,
        value: "Renamed".into(),
    }])
    .unwrap();
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
        ])
        .unwrap();
    for revision in [33, 35] {
        let (out, _) = export_open(&mut ws, &path, revision, &dir);
        let case = read(&out);
        let section = |s: &str| &case.sections.iter().find(|(n, _)| n == s).unwrap().1;
        let line = section("BRANCH")
            .iter()
            .find(|r| r.field("I").unwrap().display() == "4")
            .unwrap();
        assert_eq!(
            line.field("STAT").unwrap().display(),
            "0",
            "revision {revision}"
        );
        let generator = section("GENERATOR")
            .iter()
            .find(|r| r.field("I").unwrap().display() == "3")
            .unwrap();
        assert_eq!(generator.field("PG").unwrap().display(), "95.5");
        let bus = section("BUS")
            .iter()
            .find(|r| r.field("I").unwrap().display() == "5")
            .unwrap();
        assert!(bus.field("NAME").unwrap().display().contains("Renamed"));
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn matpower_cases_are_written_from_the_model() {
    let dir = out_dir("matpower");
    for name in [
        "case9.m",
        "case14.m",
        "case30.m",
        "case57.m",
        "case118.m",
        "case300.m",
    ] {
        let path = rs_power_fixture(name);
        for revision in [33, 35] {
            let (out, report) = export(&path, revision, &dir);
            assert_eq!(report.source, "model");
            assert!(report.notes[0].contains("MATPOWER"), "{:?}", report.notes);
            let case = read(&out);
            assert_eq!(case.revision, i64::from(revision));
            assert_same_solution(&path, &out);
        }
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn other_raw_revisions_are_written_from_the_model() {
    let dir = out_dir("older");
    for rev in 23..=32 {
        let path = rs_power_fixture(&format!("raw/revision_{rev}.raw"));
        for revision in [33, 35] {
            let (out, report) = export(&path, revision, &dir);
            assert_eq!(report.source, "model");
            read(&out);
            assert_same_solution(&path, &out);
        }
    }
    // A 33 file case-diff's parser can't read, with three-winding transformers.
    let path = rs_power_fixture("raw/transformers.raw");
    let (out, report) = export(&path, 35, &dir);
    assert!(
        report.notes[0].contains("couldn't be read one by one"),
        "{:?}",
        report.notes
    );
    read(&out);
    assert_same_solution(&path, &out);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn only_revisions_33_and_35_are_written() {
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus.raw")).unwrap();
    let err = ws
        .current_mut()
        .unwrap()
        .export_raw(&std::env::temp_dir().join("never.raw"), 34)
        .unwrap_err();
    assert!(err.to_string().contains("33 and 35"));
}

/// The synthetic grids in crates/nodal: every record of a real-size file.
#[test]
fn large_synthetic_cases_round_trip() {
    let dir = out_dir("large");
    for name in [
        "Texas2k_series25_case1_summerpeak.RAW",
        "MemphisCase2026_Mar7.RAW",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../nodal")
            .join(name);
        if !path.exists() {
            continue;
        }
        let (v35, report) = export(&path, 35, &dir);
        assert!(report.notes.is_empty(), "{name}: {:?}", report.notes);
        let (before, after) = (read(&path), read(&v35));
        assert_eq!(kept_lines(&before), kept_lines(&after), "{name}");
        assert_eq!(changes(before, after), BTreeSet::new(), "{name}");
        let (v33, report) = export(&path, 33, &dir);
        println!("{name} as 33: {:#?}", report.notes);
        read(&v33);
        let up = dir.join("up");
        std::fs::create_dir_all(&up).unwrap();
        let (back, _) = export(&v33, 35, &up);
        let lost: Vec<String> = changes(read(&path), read(&back))
            .into_iter()
            .filter(|c| {
                !(c.starts_with("CASE DETAILS")
                    || c.starts_with("BRANCH") && c.ends_with(": NAME")
                    || c.ends_with(": BASLOD")
                    || c.contains(": RATE") && !c.contains(": RATE1") && !c.contains(": RATE2") && !c.contains(": RATE3")
                    || c.contains(": RATE1") && c.len() > 2 && c.ends_with(char::is_numeric) && !c.ends_with(": RATE1")
                    || c.starts_with("LOAD") && (c.ends_with(": LOADTYPE") || c.ends_with(": DGENM"))
                    // Revision 33 holds one switched shunt per bus; the others
                    // became fixed shunts (see the notes).
                    || c.starts_with("SWITCHED SHUNT") && c.ends_with("Removed"))
            })
            .collect();
        assert!(
            lost.is_empty(),
            "{name}: {} unexpected, such as {:#?}",
            lost.len(),
            &lost[..lost.len().min(20)]
        );
    }
    std::fs::remove_dir_all(dir).ok();
}

/// The PSS/E sample case (revision 30, so written from the model) with a
/// shunt watching a FACTS device (MODSW 6), beside its shunt watching a VSC
/// converter (MODSW 4), phase shifters, area targets and both kinds of DC
/// line. Multi-terminal DC is left out so that the solutions can match.
fn psse_sample_case() -> String {
    let text = std::fs::read_to_string(rs_power_fixture("raw/t_psse_case3.raw"))
        .unwrap()
        .replace(
            " 93002,1,0.99628,0.99628,     0,  100.00,'            ',",
            " 93002,6,0.80000,0.20000,   153,  100.00,'1           ',",
        );
    let start = text.find("BEGIN MULTI-TERMINAL DC DATA").unwrap();
    let start = text[start..].find('\n').unwrap() + start + 1;
    let end = text.find(" 0 / END OF MULTI-TERMINAL DC DATA").unwrap();
    format!("{}{}", &text[..start], &text[end..])
}

/// A PowerCase's controls and DC lines, without source line numbers.
fn controls(power: &stannum_core::power::PowerCase) -> String {
    let mut v = power.voltage_controls.clone();
    v.switched_shunts.iter_mut().for_each(|s| s.line = 0);
    v.taps.iter_mut().for_each(|t| t.line = 0);
    v.facts.iter_mut().for_each(|d| d.line = 0);
    let mut f = power.flow_controls.clone();
    f.phase_shifters.iter_mut().for_each(|p| p.line = 0);
    f.reactive_taps.iter_mut().for_each(|t| t.line = 0);
    f.dc_line_taps.iter_mut().for_each(|t| t.line = 0);
    format!(
        "{v:#?}\n{f:#?}\n{:#?}\n{:#?}\n{:#?}",
        power.areas, power.lcc_lines, power.vsc_lines
    )
}

#[test]
fn model_exports_keep_dc_lines_facts_and_every_control() {
    let dir = out_dir("controls");
    let original = dir.join("psse_sample.raw");
    std::fs::write(&original, psse_sample_case()).unwrap();
    let power = stannum_core::power::load(&psse_sample_case(), false).unwrap();
    let v = &power.voltage_controls;
    // The case has everything this test is about.
    assert!(
        v.switched_shunts
            .iter()
            .any(|s| matches!(s.mode, rs_power::ShuntMode::ConverterReactive { .. }))
    );
    assert!(
        v.switched_shunts
            .iter()
            .any(|s| matches!(s.mode, rs_power::ShuntMode::FactsReactive { .. }))
    );
    assert_eq!(v.facts.len(), 2);
    assert!(!power.flow_controls.phase_shifters.is_empty());
    assert!(!power.lcc_lines.is_empty() && !power.vsc_lines.is_empty());
    assert!(power.areas.iter().any(|a| a.desired_mw != 0.0));

    for revision in [33, 35] {
        let (out, report) = export(&original, revision, &dir);
        assert_eq!(report.source, "model");
        assert!(
            !report
                .notes
                .iter()
                .any(|n| n.contains("DC line") || n.contains("MODSW")),
            "{:?}",
            report.notes
        );
        read(&out);
        let text = encoding_rs::WINDOWS_1252
            .decode(&std::fs::read(&out).unwrap())
            .0
            .into_owned();
        let back = stannum_core::power::load(&text, false).unwrap();
        assert_eq!(controls(&power), controls(&back), "revision {revision}");

        let mut ws = Workspace::default();
        ws.open(&original).unwrap();
        let settings = SolveSettings {
            tolerance_mva: 1e-6,
            area_interchange: stannum_core::power::InterchangeChoice::TieLines,
            ..Default::default()
        };
        let a = ws.current_mut().unwrap().solve(settings.clone()).unwrap();
        ws.open(&out).unwrap();
        let b = ws.current_mut().unwrap().solve(settings).unwrap();
        assert!(a.converged && b.converged);
        let mut x: Vec<_> = a.buses.iter().map(|b| (b.id, b.vm, b.va)).collect();
        let mut y: Vec<_> = b.buses.iter().map(|b| (b.id, b.vm, b.va)).collect();
        x.sort_by_key(|b| b.0);
        y.sort_by_key(|b| b.0);
        assert_eq!(x.len(), y.len());
        for (p, q) in x.iter().zip(&y) {
            assert!(
                (p.1 - q.1).abs() < 1e-6 && (p.2 - q.2).abs() < 1e-4,
                "revision {revision} bus {}: {:?} then {:?}",
                p.0,
                (p.1, p.2),
                (q.1, q.2)
            );
        }
    }
    std::fs::remove_dir_all(dir).ok();
}

/// SyntheticUSA names all nine of its DC lines 'DC Line 1'. A repeated key
/// is numbered in file order and matched by that number, so the case can
/// still be compared.
#[test]
fn case_diff_compares_records_that_repeat_a_key() {
    let text = std::fs::read_to_string(fixture("five_bus_dc.raw")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("'East link"))
        .unwrap();
    let record = lines[at..at + 3].join("\n");
    let with_copy = |copy: &str| {
        let mut out = lines[..at + 3].join("\n");
        out.push('\n');
        out.push_str(copy);
        out.push('\n');
        out.push_str(&lines[at + 3..].join("\n"));
        out.push('\n');
        out
    };
    let before = with_copy(&record);
    let after = with_copy(&record.replacen("40.00", "45.00", 1));
    let before = parse_bytes("before", before.as_bytes()).unwrap();
    assert!(
        before
            .diagnostics
            .iter()
            .any(|d| d.message == diff::parser::REPEATED_KEY)
    );
    let after = parse_bytes("after", after.as_bytes()).unwrap();
    let found = changes(before, after);
    assert_eq!(found.len(), 1, "{found:?}");
    let only = found.iter().next().unwrap();
    assert!(only.contains("'#2'") || only.contains("#2"), "{only}");
}
