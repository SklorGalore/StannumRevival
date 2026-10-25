use rs_power::{
    Error, PowerFlowOptions, RawOptions, SolveStatus, parse_raw, parse_raw_with_options, solve,
};

fn raw(sections: &[&str]) -> String {
    let mut text = "0,100,33,0,0,60\nTest case\nSecond title\n".to_owned();
    for i in 0..19 {
        if let Some(s) = sections.get(i) {
            text.push_str(s);
        }
        text.push_str("0 / section end\n");
    }
    text.push_str("Q\n");
    text
}
const BUSES: &str =
    "10,'Slack, / O''Brien',230,3,1,1,1,1,0,1.1,.9\n20,'Load',115,1,1,1,1,1,0,1.1,.9\n";
const GEN: &str = "10,'1',50,0,100,-100,1,0,100,0,1,0,0,1,1,100,200,0\n";
const LINE: &str = "10,20,'1',.01,.1,.02,100,110,120,0,0,0,0,1\n";

#[test]
fn loads_shunts_labels_and_solve() {
    let text = raw(&[
        BUSES,
        "20,1,1,1,1,30,10,0,0,0,0\n20,2,1,1,1,20,5\n20,3,0,1,1,999,999\n",
        "20,1,1,1,2\n20,2,0,50,50\n",
        GEN,
        LINE,
    ]);
    let c = parse_raw(&text).unwrap();
    assert_eq!(c.buses[1].pd, 50.0);
    assert_eq!(c.buses[1].qd, 15.0);
    assert_eq!((c.buses[1].gs, c.buses[1].bs), (1.0, 2.0));
    assert_eq!(c.labels["bus_name"][0], "Slack, / O'Brien");
    assert_eq!(
        solve(&c, &PowerFlowOptions::default()).unwrap().status,
        SolveStatus::Converged
    );
    assert_eq!(parse_raw(&text.replace('\n', "\r\n")).unwrap(), c);
}

#[test]
fn section_order_does_not_depend_on_comment_headers() {
    let original = raw(&[BUSES, "20,1,1,1,1,30,10\n", "", GEN, LINE]);
    let annotated = original
        .replace(
            "Test case\nSecond title\n",
            "Test case\nSecond title\n@! END OF OWNER DATA, BEGIN LOAD DATA\n",
        )
        .replace(
            "0 / section end",
            "0 @! deliberately incorrect section header",
        )
        .replace("'Load'", "'Load @! retained'");

    let expected = parse_raw(&original).unwrap();
    let actual = parse_raw(&annotated).unwrap();
    assert_eq!(actual.buses, expected.buses);
    assert_eq!(actual.generators, expected.generators);
    assert_eq!(actual.branches, expected.branches);
    assert_eq!(actual.labels["bus_name"][1], "Load @! retained");
}

#[test]
fn transformer_base_conversion_matches_equivalent_branch() {
    let transformer =
        "10,20,0,'1',2,2,1,0,0,2,'T',1\n.005,.05,50\n230,230,5,100,110,120,0\n115,115\n";
    let c = parse_raw(&raw(&[
        BUSES,
        "20,1,1,1,1,50,15\n",
        "",
        GEN,
        "",
        transformer,
    ]))
    .unwrap();
    let b = &c.branches[0];
    assert_eq!((b.r, b.x, b.tap, b.shift), (0.01, 0.1, 1.0, 5.0));
    let mut expected = parse_raw(&raw(&[BUSES, "20,1,1,1,1,50,15\n", "", GEN, LINE])).unwrap();
    expected.branches[0].b = 0.0;
    expected.branches[0].tap = 1.0;
    expected.branches[0].shift = 5.0;
    let actual = solve(&c, &PowerFlowOptions::default()).unwrap();
    let expected = solve(&expected, &PowerFlowOptions::default()).unwrap();
    assert_eq!(actual.case.buses, expected.case.buses);
}

#[test]
fn rejects_unsupported_and_malformed_data() {
    let good = raw(&[BUSES, "", "", GEN, LINE]);
    assert!(matches!(
        parse_raw(&good.replace("0,100,33", "0,100,36")),
        Err(Error::Unsupported(_))
    ));
    let zip = raw(&[BUSES, "20,1,1,1,1,50,15,1,0,0,0\n", "", GEN, LINE]);
    assert!(matches!(
        parse_raw_with_options(
            &zip,
            &RawOptions {
                strict: true,
                ..Default::default()
            }
        ),
        Err(Error::Unsupported(_))
    ));
    for bad in [
        good.replace(".01", "NaN"),
        good.replace("'Load'", "'Load"),
        raw(&[BUSES, "", "", "", "", "10,20,0,1\n"]),
        raw(&[BUSES, "", "", "", "", "10,20,30,1\n"]),
    ] {
        assert!(parse_raw(&bad).is_err());
    }
    assert_eq!(
        parse_raw(&good.replace("Q\n", "")).unwrap(),
        parse_raw(&good).unwrap()
    );
    let unknown = parse_raw_with_options(
        &good.replace("10,20,'1'", "10,99,'1'"),
        &RawOptions::default(),
    )
    .unwrap();
    assert!(
        unknown
            .warnings
            .iter()
            .any(|w| w.message.contains("unknown bus"))
    );
    assert!(solve(&unknown.case, &PowerFlowOptions::default()).is_err());
    let mut sections = vec![""; 18];
    sections[0] = BUSES;
    sections[7] = "'DC link',1\n";
    assert!(parse_raw(&raw(&sections)).is_err());
    sections[7] = "";
    sections[16] = "20,0,0,1,1.1,.9,0,100,'',5\n";
    assert_eq!(parse_raw(&raw(&sections)).unwrap().buses[1].bs, 5.0);
}

#[test]
fn revision_35_system_data_and_shifted_fields() {
    let text = "@!IC,SBASE,REV,XFRRAT,NXFRAT,BASFRQ\n0,100,35,0,0,60\nTitle\nTitle\nGENERAL,THRSHZ=.0002\n0 / END OF SYSTEM-WIDE DATA, BEGIN BUS DATA\n\
10,'Slack',230,3,1,1,1,1.02,0,1.1,.9,1.1,.9\n20,'Load',115,1,1,1,1,1,0,1.1,.9,1.1,.9\n\
0 / END OF BUS DATA, BEGIN LOAD DATA\n20,'1',1,1,1,30,10\n0 / END OF LOAD DATA, BEGIN FIXED SHUNT DATA\n\
0 / END OF FIXED SHUNT DATA, BEGIN GENERATOR DATA\n10,'1',50,0,100,-100,1.02,0,0,100,0,1,0,0,1,1,100,200,0\n\
0 / END OF GENERATOR DATA, BEGIN BRANCH DATA\n10,20,'1',.01,.1,.02,'Line',100,110,120,0,0,0,0,0,0,0,0,0,.01,.02,.03,.04,1\n\
0 / END OF BRANCH DATA, BEGIN SYSTEM SWITCHING DEVICE DATA\n0 / END OF SYSTEM SWITCHING DEVICE DATA, BEGIN TRANSFORMER DATA\n\
0 / END OF TRANSFORMER DATA\nQ\n";
    let imported = parse_raw_with_options(text, &RawOptions::default()).unwrap();
    assert_eq!(imported.revision, 35);
    assert_eq!(imported.case.generators[0].mbase, 100.0);
    assert!(imported.case.generators[0].online);
    assert_eq!(imported.case.branches[0].rate_a, 100.0);
    assert!(imported.case.branches[0].online);
    assert_eq!(
        (imported.case.buses[0].gs, imported.case.buses[0].bs),
        (1.0, 2.0)
    );
}

#[test]
fn memphis_revision_35_real_world_case() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("_cases/MemphisCase2026_Mar7.RAW");
    let imported = rs_power::load_raw_with_options(path, &RawOptions::default()).unwrap();
    assert_eq!(imported.revision, 35);
    assert_eq!(imported.case.buses.len(), 993);
    assert_eq!(imported.case.generators.len(), 188);
    assert_eq!(imported.case.branches.len(), 1382);
    assert_eq!(imported.case.dcline.as_ref().map_or(0, Vec::len), 0);
    assert_eq!(imported.case.labels["bus_name"].len(), 993);
    assert_eq!(imported.warnings.len(), 435);
    for message in [
        "switched shunt frozen at BINIT",
        "remote generator regulation ignored",
        "wind-machine control ignored",
    ] {
        assert!(
            imported
                .warnings
                .iter()
                .any(|w| w.message.contains(message))
        );
    }

    // A large imported case must survive the literal interchange format too.
    let formatted = rs_power::format_matpower(&imported.case).unwrap();
    assert_eq!(rs_power::parse_matpower(&formatted).unwrap(), imported.case);

    for model in [rs_power::Model::Ac, rs_power::Model::Dc] {
        let result = solve(
            &imported.case,
            &PowerFlowOptions {
                model,
                tolerance: 1e-10,
                max_iterations: 30,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        assert!(
            result.residual <= 1e-10,
            "{model:?} residual {}",
            result.residual
        );
    }
}

#[test]
fn branch_end_shunts_and_offline_equipment() {
    let c = parse_raw(&raw(&[
        BUSES,
        "",
        "",
        GEN,
        "10,20,1,.01,.1,0,0,0,0,.01,.02,.03,.04,1\n10,20,2,.01,.1,0,0,0,0,1,1,1,1,0\n",
    ]))
    .unwrap();
    assert_eq!((c.buses[0].gs, c.buses[0].bs), (1.0, 2.0));
    assert_eq!((c.buses[1].gs, c.buses[1].bs), (3.0, 4.0));
    assert!(!c.branches[1].online);
}

#[test]
fn transformer_loss_code_and_nominal_voltage_conversion() {
    let transformer =
        "10,20,0,1,3,3,1,.001,-.002,2,'T',1\n500000,.05,50\n1,230,0,100,110,120,0\n1,115\n";
    let c = parse_raw(&raw(&[BUSES, "", "", GEN, "", transformer])).unwrap();
    assert!((c.branches[0].r - 0.02).abs() < 1e-12);
    assert!((c.branches[0].x - (0.05_f64.powi(2) - 0.01_f64.powi(2)).sqrt() * 2.0).abs() < 1e-12);
    assert_eq!(c.branches[0].tap, 1.0);
    assert_eq!((c.buses[0].gs, c.buses[0].bs), (0.0, 0.0));
}

#[test]
fn file_loading_and_isolated_line_shunts() {
    let text = raw(&[
        &BUSES.replace("115,1,1", "115,4,1"),
        "",
        "",
        GEN,
        "10,20,1,.01,.1,0,0,0,0,1,1,1,1,1\n",
    ]);
    let path = std::env::temp_dir().join(format!("rs-power-raw-{}.raw", std::process::id()));
    std::fs::write(&path, &text).unwrap();
    let loaded = rs_power::load_raw(&path);
    std::fs::remove_file(&path).unwrap();
    let c = loaded.unwrap();
    assert_eq!(c, parse_raw(&text).unwrap());
    assert_eq!((c.buses[0].gs, c.buses[0].bs), (100.0, 100.0));
    assert!(matches!(rs_power::load_raw(&path), Err(Error::Io(_))));
}

#[test]
fn revision_override_default_and_mixed_delimiters() {
    let old = include_str!("fixtures/raw/revision_23.raw");
    let unknown = old.replacen("0,100,23,0,0,60", "0 100", 1);
    let report = parse_raw_with_options(&unknown, &RawOptions::default()).unwrap();
    assert_eq!(report.revision, 23);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.message.contains("unknown revision"))
    );
    assert_eq!(report.case, parse_raw(old).unwrap());
    let modern = raw(&[BUSES, "", "", GEN, LINE]);
    let overridden = parse_raw_with_options(
        &modern.replacen("0,100,33", "0,100,99", 1),
        &RawOptions {
            revision: Some(33),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(overridden.case, parse_raw(&modern).unwrap());
    let comment = modern.replacen("0,100,33,0,0,60", "0 100 / PSS(tm)E-33 RAW", 1);
    assert_eq!(parse_raw(&comment).unwrap(), overridden.case);
    assert_eq!(
        parse_raw(&comment.replace('\n', "\r")).unwrap(),
        overridden.case
    );
    // Mixed separators, omitted numeric fields, empty strings and quoted spaces.
    let buses = "20 ' padded, / name ' 115 1 1 1 1 1 0 , ,\n10 'Slack' 230 3 1 1 1 1 0 1.05 .95\n";
    let case = parse_raw(&raw(&[buses, "20 '1' 1 1 1 10 2 , , , ,\n", "", GEN, LINE])).unwrap();
    assert_eq!(case.labels["bus_name"][1], " padded, / name ");
    assert_eq!(case.buses[0].vmax, 1.1); // MATPOWER defaults the entire table.
    assert_eq!(case.buses[1].pd, 10.0);
}

#[test]
fn warning_and_strict_modes_for_ignored_equipment() {
    let mut sections = vec![""; 19];
    sections[0] = BUSES;
    sections[3] = GEN;
    sections[4] = LINE;
    sections[8] = "'VSC data omitted by MATPOWER'\n";
    let text = raw(&sections);
    let report = parse_raw_with_options(&text, &RawOptions::default()).unwrap();
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.line > 3 && w.message.contains("voltage source converter"))
    );
    assert!(matches!(
        parse_raw_with_options(
            &text,
            &RawOptions {
                strict: true,
                ..Default::default()
            }
        ),
        Err(Error::Unsupported(_))
    ));
    sections[8] = "";
    sections[16] = "20,1,0,0,1.1,.9,0,100,'',5\n";
    let report = parse_raw_with_options(&raw(&sections), &RawOptions::default()).unwrap();
    assert_eq!(report.case.buses[1].bs, 5.0);
    assert_eq!(report.warnings.len(), 2);
}

#[test]
fn dc_lines_are_preserved_and_validated() {
    let c = parse_raw(include_str!("fixtures/raw/dc_modes.raw")).unwrap();
    let lines = c.dcline.as_ref().unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0][2], 0.0);
    assert_eq!(lines[1][3], 100.0);
    assert_eq!(lines[2][3], 50.0);
    let mut invalid = c;
    invalid.dcline = Some(vec![vec![0.0; 16]]);
    assert!(rs_power::format_matpower(&invalid).is_err());
    let text =
        rs_power::format_matpower(&parse_raw(&raw(&[BUSES, "", "", GEN, LINE])).unwrap()).unwrap();
    assert!(rs_power::parse_matpower(&(text + "mpc.dcline = [1 2];\n")).is_err());
}

#[test]
fn q_termination_and_truncation() {
    let good = raw(&[BUSES, "", "", GEN, LINE]);
    assert_eq!(
        parse_raw(&(good.clone() + "ignored sequence data '")).unwrap(),
        parse_raw(&good).unwrap()
    );
    let tail = format!("0,100,33\nTitle\nTitle\n{BUSES}0\n0\n0\n{GEN}0\n{LINE}");
    assert!(parse_raw(&tail).is_err());
    assert_eq!(
        parse_raw(&(tail + "Q\n")).unwrap(),
        parse_raw(&good).unwrap()
    );
    for value in ["NaN", "inf", "1e999", "-1", "1.5"] {
        assert!(parse_raw(&good.replacen("10,'Slack", &format!("{value},'Slack"), 1)).is_err());
    }
}

#[test]
fn branch_and_generator_sources_follow_converted_rows() {
    use rs_power::BranchKind;
    let buses = format!("{BUSES}30,'Tertiary',13.8,1,1,1,1,1,0,1.1,.9\n");
    let lines = "10,20,'1',.01,.1,.02,100,110,120,0,0,0,0,1\n10,20,' 2 ',.01,.1,.02,100,110,120,0,0,0,0,1\n";
    // A three-winding transformer written before a two-winding one: rs-power
    // converts two-winding transformers first, and the sources must follow.
    let transformers = concat!(
        "10,20,30,'T3',1,1,1,0,0,2,'X',1\n",
        ".001,.1,100,.001,.1,100,.001,.1,100,1,0\n",
        "1,230,0,100,100,100\n1,115,0,100,100,100\n1,13.8,0,100,100,100\n",
        "20,10,0,'',1,1,1,0,0,2,'Y',1\n.001,.1,100\n1,115,0,100,100,100\n1,230\n",
    );
    let import = parse_raw_with_options(
        &raw(&[&buses, "", "", GEN, lines, transformers]),
        &RawOptions::default(),
    )
    .unwrap();
    let s = &import.branch_sources;
    assert_eq!(s.len(), import.case.branches.len());
    assert_eq!(s.len(), 6);
    assert_eq!((s[0].kind, s[0].circuit.as_str()), (BranchKind::Line, "1"));
    assert_eq!(s[1].circuit, "2", "circuit ids are trimmed");
    assert_eq!(
        (s[2].kind, s[2].buses.clone()),
        (BranchKind::Transformer, vec![20, 10])
    );
    assert_eq!(
        (s[2].circuit.as_str(), s[2].winding),
        ("1", 0),
        "blank circuit defaults to 1"
    );
    for (i, leg) in s[3..].iter().enumerate() {
        assert_eq!((leg.circuit.as_str(), leg.winding), ("T3", i as u8 + 1));
        assert_eq!(leg.buses, vec![10, 20, 30]);
        assert_eq!(import.case.branches[3 + i].from, [10, 20, 30][i]);
    }
    assert_eq!(import.generator_sources.len(), 1);
    assert_eq!(import.generator_sources[0].id, "1");
}

#[test]
fn vsc_and_multi_terminal_dc_lines_import_on_request() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let text = std::fs::read_to_string(path).unwrap();
    let plain = parse_raw_with_options(&text, &RawOptions::default()).unwrap();
    assert_eq!(plain.case.dcline.as_ref().unwrap().len(), 2);
    let all = parse_raw_with_options(
        &text,
        &RawOptions {
            all_dc_lines: true,
            ..Default::default()
        },
    )
    .unwrap();
    use rs_power::DcLineKind::*;
    assert_eq!(
        all.dc_line_kinds,
        [
            TwoTerminal,
            TwoTerminal,
            VoltageSourceConverter,
            VoltageSourceConverter,
            MultiTerminal,
            MultiTerminal,
            MultiTerminal
        ]
    );
    let lines = all.case.dcline.as_ref().unwrap();
    assert_eq!(lines[..2], plain.case.dcline.as_ref().unwrap()[..]);
    // VDCLINE1: 3005 withdraws 209 MW (DCSET -209) and sends it to 3008,
    // which regulates 0.99 pu within -140..150 MVAr.
    let v1 = &lines[2];
    assert_eq!(all.dc_line_names[2], "VDCLINE1");
    assert_eq!((v1[0], v1[1], v1[2], v1[3]), (3005.0, 3008.0, 1.0, 209.0));
    assert_eq!((v1[8], v1[13], v1[14]), (0.99, -140.0, 150.0));
    // 3005 is in power factor control at 0.95.
    let q = 209.0 * 0.95f64.acos().tan();
    assert!((v1[11] - q).abs() < 1e-9 && (v1[12] - q).abs() < 1e-9);
    // MTDC system 1: rectifiers 402 and 401 (321 MW each) and inverter 213
    // (303.8 MW) each pair with 212, which holds the DC voltage.
    let mt: Vec<_> = lines[4..].iter().map(|l| (l[0], l[1], l[3])).collect();
    assert_eq!(
        mt,
        [
            (402.0, 212.0, 321.0),
            (401.0, 212.0, 321.0),
            (212.0, 213.0, 303.8)
        ]
    );
    assert!(all.dc_line_names[4..].iter().all(|n| n == "1"));
    assert!(!all.warnings.iter().any(|w| w.message.contains("skipped")
        && (w.message.contains("voltage source") || w.message.contains("multi-terminal"))));
    assert!(
        parse_raw_with_options(
            &text,
            &RawOptions {
                all_dc_lines: true,
                strict: true,
                ..Default::default()
            }
        )
        .is_err()
    );
    // The extra rows solve as fixed terminal injections.
    let solved = rs_power::solve(&all.case, &rs_power::PowerFlowOptions::default()).unwrap();
    assert_eq!(solved.status, rs_power::SolveStatus::Converged);
}

#[test]
fn revision_34_mixes_revision_33_and_35_layouts() {
    // Revision 34 (PowerWorld's default export) has the system-wide block,
    // switching-device section and twelve-rating branches of revision 35, but
    // revision 33 generator and switched shunt records.
    let text = "@!IC,SBASE,REV,XFRRAT,NXFRAT,BASFRQ\n0,100,34,0,0,60\nTitle\nTitle\nGENERAL,THRSHZ=.0002\n0 / END OF SYSTEM-WIDE DATA, BEGIN BUS DATA\n\
10,'Slack',230,3,1,1,1,1.02,0,1.1,.9,1.1,.9\n20,'Load',115,1,1,1,1,1,0,1.1,.9,1.1,.9\n\
0 / END OF BUS DATA, BEGIN LOAD DATA\n20,'1',1,1,1,30,10\n0 / END OF LOAD DATA, BEGIN FIXED SHUNT DATA\n\
0 / END OF FIXED SHUNT DATA, BEGIN GENERATOR DATA\n10,'1',50,0,100,-100,1.02,0,100,0,1,0,0,1,1,100,200,0\n\
0 / END OF GENERATOR DATA, BEGIN BRANCH DATA\n10,20,'1',.01,.1,.02,'Line',100,110,120,0,0,0,0,0,0,0,0,0,.01,.02,.03,.04,1\n\
0 / END OF BRANCH DATA, BEGIN SYSTEM SWITCHING DEVICE DATA\n0 / END OF SYSTEM SWITCHING DEVICE DATA, BEGIN TRANSFORMER DATA\n\
0 / END OF TRANSFORMER DATA, BEGIN AREA DATA\n0 / END OF AREA DATA, BEGIN TWO-TERMINAL DC DATA\n\
0 / END OF TWO-TERMINAL DC DATA, BEGIN VSC DC LINE DATA\n0 / END OF VSC DC LINE DATA, BEGIN IMPEDANCE CORRECTION DATA\n\
0 / END OF IMPEDANCE CORRECTION DATA, BEGIN MULTI-TERMINAL DC DATA\n0 / END OF MULTI-TERMINAL DC DATA, BEGIN MULTI-SECTION LINE DATA\n\
0 / END OF MULTI-SECTION LINE DATA, BEGIN ZONE DATA\n0 / END OF ZONE DATA, BEGIN INTER-AREA TRANSFER DATA\n\
0 / END OF INTER-AREA TRANSFER DATA, BEGIN OWNER DATA\n0 / END OF OWNER DATA, BEGIN FACTS DEVICE DATA\n\
0 / END OF FACTS DEVICE DATA, BEGIN SWITCHED SHUNT DATA\n20,0,0,1,1.1,.9,0,100,'',25,1,25\n\
0 / END OF SWITCHED SHUNT DATA, BEGIN GNE DEVICE DATA\n0 / END OF GNE DEVICE DATA, BEGIN INDUCTION MACHINE DATA\n\
0 / END OF INDUCTION MACHINE DATA, BEGIN SUBSTATION DATA\n1,'Sub',0,0,0.1\n0 / END OF SUBSTATION NODE DATA\n0 / END OF SUBSTATION TERMINAL DATA\n\
2,'Sub 2',0,0,0.1\n0 / END OF SUBSTATION NODE DATA\nQ\n";
    let imported = parse_raw_with_options(text, &RawOptions::default()).unwrap();
    assert_eq!(imported.revision, 34);
    let g = &imported.case.generators[0];
    assert_eq!((g.mbase, g.online, g.pmax), (100.0, true, 200.0));
    let b = &imported.case.branches[0];
    assert_eq!((b.rate_a, b.rate_c, b.online), (100.0, 120.0, true));
    // BINIT 25 from the switched shunt plus BJ 0.04 p.u. from the line end.
    assert_eq!(imported.case.buses[1].bs, 29.0);
    // Substation blocks after the known sections are reported once.
    let extra: Vec<_> = imported
        .warnings
        .iter()
        .filter(|w| w.message.contains("additional section"))
        .collect();
    assert_eq!(extra.len(), 1);
    assert_eq!(
        extra[0].message,
        "skipped 2 records of additional section data"
    );
}

#[test]
fn line_commutated_converters_absorb_reactive_power_on_request() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let text = std::fs::read_to_string(path).unwrap();
    let options = |lcc_absorbs_reactive| RawOptions {
        all_dc_lines: true,
        lcc_absorbs_reactive,
        ..Default::default()
    };
    let plain = parse_raw_with_options(&text, &options(false)).unwrap();
    let flipped = parse_raw_with_options(&text, &options(true)).unwrap();
    let (plain, flipped) = (plain.case.dcline.unwrap(), flipped.case.dcline.unwrap());
    for (i, kind) in [0, 1, 2, 3, 4, 5, 6]
        .into_iter()
        .zip(["lcc", "lcc", "vsc", "vsc", "lcc", "lcc", "lcc"])
    {
        let (p, f) = (&plain[i], &flipped[i]);
        if kind == "vsc" {
            assert_eq!(p, f, "VSC row {i} is unchanged");
        } else {
            for col in [11, 13] {
                assert_eq!((f[col], f[col + 1]), (-p[col + 1], -p[col]), "row {i}");
                assert!(f[col + 1] <= 0.0, "row {i} only absorbs");
            }
            assert_eq!(p[..11], f[..11]);
            assert_eq!(p[15..], f[15..]);
        }
    }
}

#[test]
fn psse_case3_dc_lines_solve_with_q_limits_when_converters_absorb() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/raw/t_psse_case3.raw");
    let text = std::fs::read_to_string(path).unwrap();
    let imported = parse_raw_with_options(
        &text,
        &RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    for enforce_q_limits in [false, true] {
        let result = rs_power::solve(
            &imported.case,
            &rs_power::PowerFlowOptions {
                enforce_q_limits,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.status, rs_power::SolveStatus::Converged);
        let lines = result.case.dcline.as_ref().unwrap();
        // Both two-terminal rectifiers absorb reactive power.
        assert!(lines[0][5] < 0.0 && lines[1][5] < 0.0);
        // VDCLINE1's sending end holds its 0.95 power factor.
        let q = 209.0 * 0.95f64.acos().tan();
        assert!((lines[2][5] - q).abs() < 1e-9);
        if enforce_q_limits {
            for line in lines {
                assert!(line[5] >= line[11] - 1e-4 && line[5] <= line[12] + 1e-4);
                assert!(line[6] >= line[13] - 1e-4 && line[6] <= line[14] + 1e-4);
            }
        }
    }
}
