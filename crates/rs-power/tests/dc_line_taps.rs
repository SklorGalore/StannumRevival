//! Transformers that a two-terminal DC line converter moves in place of its
//! own tap (PSS/E transformer COD 4).
use rs_power::{
    DcModel, LccEnd, LccMode, PowerFlowOptions, PowerFlowResult, RawImport, RawOptions,
    SolveStatus, parse_raw_with_options, solve,
};

fn fixture(name: &str) -> String {
    let dir = if name == FIVE_BUS {
        "dc_line_taps"
    } else {
        "raw"
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir)
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn import(text: &str) -> RawImport {
    parse_raw_with_options(
        text,
        &RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap()
}

fn run(imported: &RawImport, adjust_taps: bool) -> PowerFlowResult {
    let options = PowerFlowOptions {
        dc_model: DcModel::Detailed {
            lines: imported.lcc_lines.clone(),
            vsc_lines: Vec::new(),
            mtdc: Vec::new(),
            adjust_taps,
        },
        flow_controls: imported.flow_controls.clone(),
        ..Default::default()
    };
    let result = solve(&imported.case, &options).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    result
}

/// The five-bus DC fixture with the rectifier moved behind a 1-6
/// transformer with COD 4: at its converter tap of 0.95 its firing angle is
/// above its 10 to 20 degree range.
const FIVE_BUS: &str = "five_bus_dc_transformer.raw";
/// The COD field of that transformer's winding record.
const COD_4: &str = "0.00, 4,     0,   0,1.100000";

#[test]
fn raw_import_links_the_transformer_to_its_converter() {
    let imported = import(&fixture(FIVE_BUS));
    let [tap] = &imported.flow_controls.dc_line_taps[..] else {
        panic!("{:?}", imported.flow_controls.dc_line_taps);
    };
    let branch = &imported.case.branches[tap.branch];
    assert_eq!((branch.from, branch.to), (1, 6));
    assert_eq!(
        (tap.dc_line, tap.end, tap.ratio),
        (0, LccEnd::Rectifier, 1.0)
    );
    assert!(tap.enabled);
    assert_eq!(tap.line, 34);
    for w in &imported.warnings {
        assert!(
            !w.message.contains("IF/IT/ID") && w.line != 34,
            "{}: {}",
            w.line,
            w.message
        );
    }
}

#[test]
fn transformer_moves_in_place_of_the_converter_tap() {
    let imported = import(&fixture(FIVE_BUS));
    let tap = &imported.flow_controls.dc_line_taps[0];
    let rect = &imported.lcc_lines[0].rectifier;
    let held = run(&imported, false);
    assert_eq!(held.dc_line_tap_ratios, [1.0]);
    assert!(
        held.lcc[0].alpha > rect.angle_max + 3.0,
        "{}",
        held.lcc[0].alpha
    );

    let moved = run(&imported, true);
    let s = &moved.lcc[0];
    let ratio = moved.dc_line_tap_ratios[0];
    assert_eq!(s.mode, LccMode::RectifierControl);
    // The converter tap holds and the transformer brings α into range.
    assert_eq!(s.rectifier_tap, rect.tap);
    assert!(ratio > 1.0 && ratio <= rect.tap_max, "{ratio}");
    assert_eq!(moved.case.branches[tap.branch].tap, ratio);
    // A whole number of converter tap steps from TMN.
    let steps = (ratio - rect.tap_min) / rect.tap_step;
    assert!((steps - steps.round()).abs() < 1e-9, "{steps}");
    assert!(
        s.alpha >= rect.angle_min && s.alpha <= rect.angle_max,
        "{}",
        s.alpha
    );
}

#[test]
fn suppressed_control_holds_the_transformer_and_the_converter_tap() {
    let text = fixture(FIVE_BUS).replace(COD_4, "0.00,-4,     0,   0,1.100000");
    let imported = import(&text);
    assert!(!imported.flow_controls.dc_line_taps[0].enabled);
    let result = run(&imported, true);
    assert_eq!(result.dc_line_tap_ratios, [1.0]);
    assert_eq!(
        result.lcc[0].rectifier_tap,
        imported.lcc_lines[0].rectifier.tap
    );
}

#[test]
fn converter_naming_a_transformer_without_cod_4_moves_its_own_tap() {
    let text = fixture(FIVE_BUS).replace(COD_4, "0.00, 0,     0,   0,1.100000");
    let imported = import(&text);
    assert!(imported.flow_controls.dc_line_taps.is_empty());
    assert!(
        imported.warnings.iter().any(|w| w.line == 40
            && w.message
                .contains("not a two-winding transformer with COD 4")),
        "{:?}",
        imported.warnings
    );
    let result = run(&imported, true);
    let s = &result.lcc[0];
    assert_ne!(s.rectifier_tap, imported.lcc_lines[0].rectifier.tap);
    assert!(s.alpha <= imported.lcc_lines[0].rectifier.angle_max + 1e-9);
}

#[test]
fn unnamed_cod_4_transformer_is_held_with_a_warning() {
    // The rectifier no longer names the transformer.
    let text =
        fixture(FIVE_BUS).replace("0,     0,     1,     6,'1 '", "0,     0,     0,     0,'1 '");
    let imported = import(&text);
    assert!(imported.flow_controls.dc_line_taps.is_empty());
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.line == 34 && w.message.contains("not named by a two-terminal DC line")),
        "{:?}",
        imported.warnings
    );
}

/// MATPOWER's PSS/E test case 3: both inverters name a transformer with
/// COD 4 (T4 and T5), whose taps start at the inverters' TMXI of 1.1.
#[test]
fn psse_case3_inverters_hold_their_taps_and_move_transformers() {
    let imported = import(&fixture("t_psse_case3.raw"));
    let taps = &imported.flow_controls.dc_line_taps;
    assert_eq!(taps.len(), 2);
    for (tap, line) in taps.iter().zip(&imported.lcc_lines) {
        assert_eq!((tap.dc_line, tap.end), (line.row, LccEnd::Inverter));
        let b = &imported.case.branches[tap.branch];
        assert_eq!((b.from, b.to), (152, line.inverter.bus));
    }
    let result = run(&imported, true);
    for (s, line) in result.lcc.iter().zip(&imported.lcc_lines) {
        assert_eq!(s.mode, LccMode::RectifierControl);
        assert_eq!(s.inverter_tap, line.inverter.tap);
        assert!((s.rectifier_kv * s.current_ka - line.setpoint).abs() < 1e-6);
    }
    // Both inverters want a lower DC voltage, which the transformers would
    // give above TMXI: they stay at the limit.
    assert_eq!(result.dc_line_tap_ratios, [1.1, 1.1]);
}
