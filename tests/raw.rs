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
        parse_raw(&good.replace("0,100,33", "0,100,34")),
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
            strict: false,
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
