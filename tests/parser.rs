use nodal::parse_raw;

fn raw(sections: &[&str]) -> Vec<u8> {
    let mut s = "0,100,35,0,0,60\nTitle\n\n".to_owned();
    for section in sections {
        s.push_str(section);
        if !section.is_empty() && !section.ends_with('\n') {
            s.push('\n');
        }
        s.push_str("0\n");
    }
    // All 21 v35 sections, including empty trailing sections.
    for _ in sections.len()..21 {
        s.push_str("0\n");
    }
    s.push_str("Q\n");
    s.into_bytes()
}

#[test]
fn parses_both_supplied_cases_with_v35_status_positions() {
    for (path, buses, lines, transformers, loads, generators, encoding) in [
        (
            "MemphisCase2026_Mar7.RAW",
            993,
            975,
            407,
            530,
            188,
            "Windows-1252",
        ),
        (
            "Texas2k_series25_case1_summerpeak.RAW",
            2751,
            3993,
            1351,
            1410,
            1099,
            "UTF-8",
        ),
    ] {
        let case = parse_raw(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(case.buses.len(), buses);
        assert_eq!(
            case.equipment
                .iter()
                .filter(|e| e.kind == nodal::model::EquipmentKind::Line)
                .count(),
            lines
        );
        assert_eq!(
            case.equipment
                .iter()
                .filter(|e| e.kind == nodal::model::EquipmentKind::Transformer)
                .count(),
            transformers
        );
        assert_eq!(case.loads.len(), loads);
        assert_eq!(case.generators.len(), generators);
        assert_eq!(case.encoding, encoding);
        assert!(
            case.equipment
                .iter()
                .all(|e| e.status == 0 || e.status == 1)
        );
    }
}

#[test]
fn parses_without_annotations_and_preserves_quoted_delimiters_and_titles() {
    let bytes = raw(&[
        "",
        "1,'One, / ''quoted'' ',138,3\n2,\"Two / bus\",69,1",
        "",
        "",
        "",
        "1,2,'A',.01,1E-2",
    ]);
    let case = parse_raw(&bytes).unwrap();
    assert_eq!(case.titles, ["Title", ""]);
    assert_eq!(case.buses[0].name, "One, / 'quoted' ");
    assert_eq!(case.buses[1].name, "Two / bus");
    assert_eq!(case.equipment[0].status, 1);
    assert_eq!(case.equipment[0].circuit, "A");
}

#[test]
fn rejects_bad_references_duplicates_and_malformed_numbers_with_source() {
    for (bus, branch, fragment) in [
        ("1,'One',138,1", "1,9,'1',0,.1", "bus 9"),
        ("1,'One',138,1\n1,'Again',138,1", "", "Duplicate bus"),
        ("1,'One',invalid,1", "", "BASKV"),
        ("1,'One',NaN,1", "", "BASKV"),
        (
            "1,'One',138,1\n2,'Two',138,1",
            "1,2,'1',0,.1\n2,1,'1 ',0,.1",
            "Duplicate equipment",
        ),
    ] {
        let error = parse_raw(&raw(&["", bus, "", "", "", branch])).unwrap_err();
        assert!(error.message.contains(fragment), "{error}");
        assert!(error.line > 0);
    }
}

#[test]
fn rejects_unsupported_revision_and_truncated_transformer() {
    let err = parse_raw(b"0,100,33\n\n\nQ").unwrap_err();
    assert!(err.message.contains("35"));
    let err = parse_raw(&raw(&[
        "",
        "1,'One',138,1\n2,'Two',69,1",
        "",
        "",
        "",
        "",
        "",
        "1,2,0,'1'\n.01,.1,100",
    ]))
    .unwrap_err();
    assert_eq!(err.section, "TRANSFORMER");
    assert!(err.message.contains("Truncated"));
}

#[test]
fn reports_nonempty_unsupported_connectivity_sections() {
    let case = parse_raw(&raw(&[
        "",
        "1,'One',138,1",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "'DC',1,0",
    ]))
    .unwrap();
    assert!(
        case.diagnostics
            .iter()
            .any(|d| d.section == "TWO-TERMINAL DC" && d.message.contains("excluded"))
    );
}

#[test]
fn switching_devices_use_current_status_not_normal_status() {
    let mut closed = vec!["1".to_owned(), "2".into(), "A".into(), "0".into()];
    closed.extend((0..12).map(|_| "0".to_owned()));
    closed.extend([
        "1".into(),
        "0".into(),
        "1".into(),
        "1".into(),
        "'Closed'".into(),
    ]);
    let mut open = closed.clone();
    open[2] = "B".into();
    open[16] = "0".into();
    open[17] = "1".into();
    let switches = format!("{}\n{}", closed.join(","), open.join(","));
    let case = parse_raw(&raw(&[
        "",
        "1,'One',138,1\n2,'Two',138,1",
        "",
        "",
        "",
        "",
        &switches,
    ]))
    .unwrap();
    let analysis = nodal::analyze(&case);
    assert_eq!(analysis.active_equipment, 1);
    assert_eq!(analysis.bridges.len(), 1);
    assert_eq!(analysis.bridges[0].equipment_id, "switch:1:2:A");
}

#[test]
fn parses_three_winding_status_codes_and_consumes_exact_record_length() {
    let buses = "1,'One',138,1\n2,'Two',69,1\n3,'Three',13.8,1";
    for (status, active) in [
        (1, vec![true, true, true]),
        (2, vec![true, false, true]),
        (3, vec![true, true, false]),
        (4, vec![false, true, true]),
    ] {
        let transformer = format!(
            "1,2,3,'A',1,1,1,0,0,2,'Three',{status}\n0,.1,100,0,.1,100,0,.1,100\n1,138\n1,69\n1,13.8"
        );
        let case = parse_raw(&raw(&["", buses, "", "", "", "", "", &transformer])).unwrap();
        assert_eq!(case.equipment.len(), 1);
        assert_eq!(case.equipment[0].active_terminals, active);
        assert_eq!(case.equipment[0].source.raw_lines.len(), 5);
        assert_eq!(nodal::analyze(&case).synthetic_junctions, 1);
    }
}

#[test]
fn comments_crlf_and_exact_physical_title_lines_do_not_shift_sections() {
    let bytes = raw(&[
        "GENERAL, THRSHZ=.0002",
        "1,'One',138,1 / bus comment\n@! optional header\n2,'Two',69,1",
        "",
        "",
        "",
        "1,2,'1',0,.1 / branch comment",
    ]);
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace("Title\n\n", "@! title text\n\n")
        .replace('\n', "\r\n");
    let case = parse_raw(text.as_bytes()).unwrap();
    assert_eq!(case.titles, ["@! title text", ""]);
    assert_eq!(case.buses.len(), 2);
    assert_eq!(case.equipment.len(), 1);
    assert!(nodal::shortest_path(&case, 1, 2).unwrap().is_some());
}

#[test]
fn numeric_circuit_header_cannot_replace_a_missing_transformer_winding_row() {
    let buses = "1,'One',138,1\n2,'Two',69,1\n3,'Three',138,1\n4,'Four',69,1";
    let transformer = "1,2,0,'1'\n0,.1,100\n1,138\n3,4,0,'1'";
    let error = parse_raw(&raw(&["", buses, "", "", "", "", "", transformer])).unwrap_err();
    assert_eq!(error.section, "TRANSFORMER");
    assert!(error.message.contains("Truncated") || error.message.contains("continuation"));
}

#[test]
fn requires_terminators_for_every_v35_section_before_q() {
    let bytes = b"0,100,35,0,0,60\nTitle\n\n0\n1,'One',138,1\n0\n0\n0\n0\n0\n0\n0\nQ\n";
    let error = parse_raw(bytes).unwrap_err();
    assert_eq!(error.section, "AREA");
    assert!(error.message.contains("section"));
}
