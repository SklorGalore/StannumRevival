//! Stannum case files (.stn): RAW and MATPOWER cases converted to them and
//! back must solve the same, and converting again must change nothing.

use stannum_core::Workspace;
use stannum_core::power::SolveSettings;
use stannum_core::stn;
use std::path::{Path, PathBuf};

fn core_fixture(name: &str) -> PathBuf {
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
    let dir = std::env::temp_dir().join(format!("stannum-stn-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Bus voltages after a tight solve.
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
    assert_eq!(x.len(), y.len(), "{} and {}", a.display(), b.display());
    for (p, q) in x.iter().zip(&y) {
        assert_eq!(p.0, q.0);
        assert!(
            (p.1 - q.1).abs() < 1e-6 && (p.2 - q.2).abs() < 1e-4,
            "bus {}: {:?} in {} but {:?} in {}",
            p.0,
            p,
            a.display(),
            q,
            b.display()
        );
    }
}

/// RAW → .stn → RAW: the .stn text reads back to itself, the RAW written
/// from it solves like the original, and converting that RAW again gives
/// the same .stn text.
fn round_trip(path: &Path, dir: &Path) -> String {
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let bytes = std::fs::read(path).unwrap();
    let (doc, report) = stn::from_raw(&name, &bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    let text = stn::write(&doc).unwrap();
    let again =
        stn::read(&text).unwrap_or_else(|e| panic!("{name} .stn doesn't read back:\n{e}\n{text}"));
    assert_eq!(
        stn::write(&again).unwrap(),
        text,
        "{name}: canonical text changed on reading"
    );
    assert!(report.records > 0);

    let raw = stn::to_raw(&again, &name).unwrap();
    let back = dir.join(format!(
        "{}-from-stn.raw",
        path.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::write(&back, &raw).unwrap();
    assert_same_solution(path, &back);

    let (doc2, _) = stn::from_raw(&name, raw.as_bytes()).unwrap();
    let text2 = stn::write(&doc2).unwrap();
    if text2 != text {
        let diff: Vec<String> = text
            .lines()
            .zip(text2.lines())
            .filter(|(a, b)| a != b)
            .take(6)
            .map(|(a, b)| format!("- {a}\n+ {b}"))
            .collect();
        panic!("{name}: .stn → RAW → .stn changed:\n{}", diff.join("\n"));
    }
    text
}

#[test]
fn raw_cases_round_trip() {
    let dir = out_dir("raw");
    for name in [
        "five_bus.raw",
        "five_bus_devices.raw",
        "five_bus_dc.raw",
        "five_bus_zip.raw",
        "five_bus_switches.raw",
        "five_bus_substations.raw",
        "five_bus_phase_shifter.raw",
        "five_bus_reactive_tap.raw",
        "raw_export/five_bus_everything.raw",
    ] {
        round_trip(&core_fixture(name), &dir);
    }
    round_trip(
        &rs_power_fixture("dc_line_taps/five_bus_dc_transformer.raw"),
        &dir,
    );
}

/// MATPOWER and older RAW revisions go through the power flow model, as
/// RAW export does: the .stn file must solve like the RAW export (which
/// matches the original except for what the model doesn't hold, such as
/// multi-terminal DC in t_psse_case3).
#[test]
fn other_cases_convert_through_the_model() {
    let dir = out_dir("model");
    for (path, buses) in [
        (core_fixture("case9.m"), 9),
        (rs_power_fixture("raw/t_psse_case2.raw"), 0),
        (rs_power_fixture("raw/t_psse_case3.raw"), 0),
        // Revision 33 files the record reader refuses (too few bus fields).
        (rs_power_fixture("raw/transformers.raw"), 0),
        (rs_power_fixture("raw/revision_33.raw"), 0),
    ] {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let mut ws = Workspace::default();
        ws.open(&path).unwrap();
        let power = ws.current().unwrap().power.clone().unwrap();
        let (doc, report) = stn::from_model(&name, &power, "From the model.".into());
        assert_eq!(report.notes[0], "From the model.");
        let text = stn::write(&doc).unwrap();
        assert!(text.starts_with("stannum-case 0.1\n"));
        if buses > 0 {
            assert_eq!(doc.records("bus").len(), buses, "{name}");
        }
        let raw = stn::to_raw(&stn::read(&text).unwrap(), &name).unwrap();
        let back = dir.join(format!("{name}-from-stn.raw"));
        std::fs::write(&back, raw).unwrap();
        let exported = dir.join(format!("{name}-exported.raw"));
        ws.current_mut().unwrap().export_raw(&exported, 35).unwrap();
        assert_same_solution(&exported, &back);
    }
}

#[test]
fn units_and_names_replace_codes() {
    let text = round_trip(&core_fixture("five_bus_devices.raw"), &out_dir("units"));
    // Buses by number, codes as names.
    assert!(
        text.contains("\n# bus\nbus 1 number=1 name=North_138 kv=138 type=ref vm_pu=1.04\n"),
        "{text}"
    );
    // Constant-admittance load is MVAr consumed; zero stays plain zero.
    assert!(!text.contains("yq_mvar=-0"), "{text}");
}

/// A transformer in kV ratios (CW 2), on its own 300 MVA base (CZ 2), with
/// no-load loss and exciting current (CM 2); and a three-winding one with
/// load loss in watts (CZ 3). The format holds per-unit values; RAW written
/// back gives the file's own numbers, and the case solves the same.
#[test]
fn transformer_codes_convert_and_come_back() {
    let dir = out_dir("codes");
    let text = std::fs::read_to_string(core_fixture("raw_export/five_bus_everything.raw")).unwrap();
    let text = text
        .replace(
            "     2,     4,    0,'1 ',1,1,1,0.00000E-1,0.00000E-1,2,",
            "     2,     4,    0,'1 ',2,2,2,50000.0,0.01,2,",
        )
        .replace("3.82000E-3,1.12620E-1, 100.00\n", "1.14600E-2,3.37860E-1, 300.00\n")
        .replace(
            "1.000000,138.000,   0.000, 146.60, 146.60, 146.60,",
            "140.0,138.000,   0.000, 146.60, 146.60, 146.60,",
        )
        .replace("1.500000,0.510000,1.500000,0.510000,159", "207.0,70.38,1.500000,0.510000,159")
        .replace("1.000000, 69.000\n", "69.0, 69.000\n")
        .replace(
            "     2,     4,     6,'2 ',1,1,1,",
            "     2,     4,     6,'2 ',1,3,1,",
        )
        .replace(
            "5.00000E-3,1.50000E-1, 100.00,4.00000E-3,1.00000E-1, 100.00,4.50000E-3,1.20000E-1, 100.00,",
            "250000.0,0.15, 100.00,200000.0,0.1, 100.00,225000.0,0.12, 100.00,",
        );
    let original = dir.join("codes.raw");
    std::fs::write(&original, &text).unwrap();
    let (doc, report) = stn::from_raw("codes.raw", text.as_bytes()).unwrap();
    assert!(
        report.notes.iter().any(|n| n.contains("2 transformers")),
        "{:?}",
        report.notes
    );
    let two = &doc.records("transformer")[0];
    // 140 kV on a 138 kV winding; 1.146 % on 300 MVA stays per unit on 300 MVA.
    assert_eq!(two.float("w1_tap_pu"), Some(140.0 / 138.0));
    assert_eq!(two.float("mva_base"), Some(300.0));
    assert_eq!(two.float("r_pu"), Some(0.01146));
    assert_eq!(two.float("adjust_max"), Some(207.0 / 138.0));
    assert!(two.get("x.psse.orig").is_some());
    let stn_text = stn::write(&doc).unwrap();
    let raw = stn::to_raw(&stn::read(&stn_text).unwrap(), "codes.raw").unwrap();
    // The file's own numbers come back.
    assert!(raw.contains("2,4,0,'1',2,2,2,50000.0,0.01,2"), "{raw}");
    assert!(raw.contains("0.01146,0.33786,300.0"), "{raw}");
    let back = dir.join("codes-from-stn.raw");
    std::fs::write(&back, &raw).unwrap();
    assert_same_solution(&original, &back);

    // Once changed, the transformer is written in the format's own units.
    let mut edited = stn::read(&stn_text).unwrap();
    let t = &mut edited
        .tables
        .iter_mut()
        .find(|t| t.name == "transformer")
        .unwrap()
        .records[0];
    t.set("x_pu", stannum_format::Value::Float(0.4));
    let raw = stn::to_raw(&edited, "codes.raw").unwrap();
    assert!(raw.contains("2,4,0,'1',3,2,1,"), "{raw}");
    assert!(raw.contains("0.01146,0.4,300.0"), "{raw}");
}

/// A .stn file opens like any case: it solves like the RAW it came from,
/// takes Records edits, compares with its working case, saves as .stn or
/// .raw, and a MATPOWER case exports to one.
#[test]
fn stn_files_open_edit_and_save() {
    use stannum_core::record_edit::RecordEdit;
    use stannum_core::workspace::CaseFormat;
    let dir = out_dir("app");
    let raw = core_fixture("five_bus_devices.raw");
    let (doc, _) = stn::from_raw("five_bus_devices.raw", &std::fs::read(&raw).unwrap()).unwrap();
    let path = dir.join("five_bus_devices.stn");
    std::fs::write(&path, stn::write(&doc).unwrap()).unwrap();
    assert_same_solution(&raw, &path);

    let mut ws = Workspace::default();
    let summary = ws.open(&path).unwrap();
    assert_eq!(summary.format, CaseFormat::Stn);
    assert!(summary.diffable);
    let case = ws.current_mut().unwrap();
    let records = case.records().unwrap().clone();
    assert!(records.editable);
    let bus = records.table("BUS").unwrap();
    let column = bus.columns.iter().position(|c| c.name == "NAME").unwrap();
    case.apply_record_edits(&[RecordEdit {
        section: "BUS".into(),
        row: 4,
        column,
        value: "Renamed".into(),
    }])
    .unwrap();
    assert_eq!(ws.current().unwrap().format, CaseFormat::Stn);
    let diff = ws.compare_working().unwrap();
    assert_eq!(diff.differing, 1);

    // Saved as .stn it stays a Stannum case and keeps the edit.
    let saved = dir.join("five_bus_devices-edited.stn");
    let summary = ws.current_mut().unwrap().save_as(&saved).unwrap();
    assert_eq!(summary.format, CaseFormat::Stn);
    let text = std::fs::read_to_string(&saved).unwrap();
    assert!(text.starts_with("stannum-case 0.1\n"));
    assert!(text.contains("name=Renamed"), "{text}");
    // Saved as .raw it becomes RAW.
    let as_raw = dir.join("five_bus_devices-edited.raw");
    let summary = ws.current_mut().unwrap().save_as(&as_raw).unwrap();
    assert_eq!(summary.format, CaseFormat::Raw);
    assert_same_solution(&saved, &as_raw);

    // Warnings point at the .stn file's own lines, or at none.
    let text = std::fs::read_to_string(core_fixture("raw_export/five_bus_everything.raw")).unwrap();
    let (doc, _) = stn::from_raw("e.raw", text.as_bytes()).unwrap();
    let everything = dir.join("everything.stn");
    let stn_text = stn::write(&doc).unwrap();
    std::fs::write(&everything, &stn_text).unwrap();
    let mut ws = Workspace::default();
    ws.open(&everything).unwrap();
    let warnings = &ws.current().unwrap().power.as_ref().unwrap().warnings;
    let generator = warnings
        .iter()
        .find(|w| w.element.as_deref() == Some("generator:1:1"))
        .unwrap();
    assert!(
        stn_text
            .lines()
            .nth(generator.line - 1)
            .unwrap()
            .starts_with("generator 1:1 ")
    );
    assert!(
        warnings
            .iter()
            .filter(|w| w.element.is_none())
            .all(|w| w.line == 0)
    );

    // A .stn file the reader refuses doesn't open.
    let bad = dir.join("bad.stn");
    std::fs::write(&bad, "stannum-case 0.1\nbus 1 number=one\n").unwrap();
    let err = Workspace::default().open(&bad).unwrap_err();
    assert!(err.message.contains("number"), "{}", err.message);

    // MATPOWER exports from its model; the export solves the same.
    let mut ws = Workspace::default();
    ws.open(&core_fixture("case9.m")).unwrap();
    let out = dir.join("case9.stn");
    let done = ws.current_mut().unwrap().export_stn(&out).unwrap();
    assert!(done.notes[0].contains("MATPOWER"));
    assert_same_solution(&core_fixture("case9.m"), &out);
    assert!(
        ws.current_mut()
            .unwrap()
            .save_as(&dir.join("x.stn"))
            .is_err()
    );
}

/// The RAW and MATPOWER cases in the format's conformance suite convert to
/// their `.stn` files there exactly. `STANNUM_UPDATE_CONFORMANCE=1` writes
/// them again, for a reviewed change to the conversion.
#[test]
fn conformance_conversions() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stannum-format/conformance/raw");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("raw" | "m")))
        .collect();
    cases.sort();
    assert!(cases.len() >= 3);
    let out = out_dir("conformance");
    for path in cases {
        let golden = path.with_extension("stn");
        let written = out.join(golden.file_name().unwrap());
        let mut ws = Workspace::default();
        ws.open(&path).unwrap();
        ws.current_mut().unwrap().export_stn(&written).unwrap();
        let text = std::fs::read_to_string(&written).unwrap();
        if std::env::var_os("STANNUM_UPDATE_CONFORMANCE").is_some() {
            std::fs::write(&golden, &text).unwrap();
        } else {
            let expected = std::fs::read_to_string(&golden)
                .unwrap()
                .replace("\r\n", "\n");
            assert_eq!(text, expected, "{}", golden.display());
        }
        assert_same_solution(&path, &written);
    }
}

/// A .stn file's own ids, comments and extensions survive opening it,
/// editing it in Records and saving or exporting it again, though the app
/// works on its RAW form.
#[test]
fn stn_ids_comments_and_extensions_survive_editing() {
    use stannum_core::record_edit::RecordEdit;
    let dir = out_dir("carry");
    let raw = core_fixture("five_bus_devices.raw");
    let (doc, _) = stn::from_raw("five_bus_devices.raw", &std::fs::read(&raw).unwrap()).unwrap();
    let text = stn::write(&doc)
        .unwrap()
        .replace("stannum-case 0.1\n", "stannum-case 0.1\n# Spur study.\n")
        .replace(
            "bus 5 number=5 ",
            "# Renumbered from 15.\nbus spur number=5 x.note=\"keep me\" ",
        )
        .replace(" bus=5 ", " bus=spur ")
        .replace(" to=5 ", " to=spur ")
        .replace("reg_bus=5", "reg_bus=spur");
    assert!(text.contains("bus spur number=5"), "{text}");
    let path = dir.join("spur.stn");
    std::fs::write(&path, &text).unwrap();
    assert_same_solution(&raw, &path);

    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    let case = ws.current_mut().unwrap();
    let records = case.records().unwrap().clone();
    let load = records.table("LOAD").unwrap();
    let column = load.columns.iter().position(|c| c.name == "PL").unwrap();
    case.apply_record_edits(&[RecordEdit {
        section: "LOAD".into(),
        row: 2,
        column,
        value: "25".into(),
    }])
    .unwrap();
    let saved = dir.join("spur-edited.stn");
    ws.current_mut().unwrap().save_as(&saved).unwrap();
    let out = std::fs::read_to_string(&saved).unwrap();
    assert!(
        out.starts_with("stannum-case 0.1\n# Spur study.\n"),
        "{out}"
    );
    assert!(
        out.contains("# Renumbered from 15.\nbus spur number=5 "),
        "{out}"
    );
    assert!(out.contains("x.note=\"keep me\""), "{out}");
    assert!(out.contains("load 5:1 bus=spur area=1 p_mw=25 "), "{out}");
    assert!(out.contains("line 4:5:1 from=4 to=spur "), "{out}");
    // Exporting gives the same file.
    let exported = dir.join("spur-exported.stn");
    ws.current_mut().unwrap().export_stn(&exported).unwrap();
    assert_eq!(std::fs::read_to_string(&exported).unwrap(), out);
}
