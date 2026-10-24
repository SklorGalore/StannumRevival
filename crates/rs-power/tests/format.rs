use rs_power::{format_matpower, load_matpower, parse_matpower, save_matpower};

#[test]
fn bundled_cases_round_trip_exactly() {
    for name in ["case9", "case14", "case30", "case57", "case118", "case300"] {
        let case = load_matpower(format!("tests/fixtures/{name}.m")).unwrap();
        let text = format_matpower(&case).unwrap();
        assert_eq!(parse_matpower(&text).unwrap(), case, "{name}");
    }
}

#[test]
fn preserves_labels_extra_columns_infinities_and_empty_tables() {
    let mut case = load_matpower("tests/fixtures/case9.m").unwrap();
    case.generators[0].qmin = f64::NEG_INFINITY;
    case.generators[0].qmax = f64::INFINITY;
    case.generators[0].online = false;
    for bus in &mut case.buses {
        bus.extra = vec![1e-200, -0.0, f64::MAX];
    }
    case.labels.insert(
        "bus_name".into(),
        vec!["O'Brien % ... 東京".into(), String::new()],
    );
    case.labels.insert("gentype".into(), vec![]);
    case.gencost = Some(vec![]);
    case.branches.clear();
    assert_eq!(
        parse_matpower(&format_matpower(&case).unwrap()).unwrap(),
        case
    );
}

#[test]
fn rejects_unrepresentable_cases() {
    let original = load_matpower("tests/fixtures/case9.m").unwrap();
    let mut case = original.clone();
    case.buses[0].extra.push(1.0);
    assert!(format_matpower(&case).is_err());
    let mut case = original.clone();
    case.base_mva = f64::NAN;
    assert!(format_matpower(&case).is_err());
    let mut case = original.clone();
    case.labels
        .insert("bus_name".into(), vec!["line\nbreak".into()]);
    assert!(format_matpower(&case).is_err());
    let mut case = original.clone();
    case.labels.insert("unknown".into(), vec![]);
    assert!(format_matpower(&case).is_err());
    let mut case = original;
    case.gencost = Some(vec![vec![]]);
    assert!(format_matpower(&case).is_err());
}

#[test]
fn saved_function_can_be_loaded_and_invalid_input_does_not_overwrite() {
    let dir = std::env::temp_dir().join(format!("rs_power_format_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("exported_case.m");
    let mut case = load_matpower("tests/fixtures/case9.m").unwrap();
    save_matpower(&path, &case).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("function mpc = exported_case\n"));
    assert_eq!(load_matpower(&path).unwrap(), case);
    for name in ["bad-name.m", "1case.m", "end.m"] {
        assert!(save_matpower(dir.join(name), &case).is_err());
        assert!(!dir.join(name).exists());
    }
    case.base_mva = f64::NAN;
    assert!(save_matpower(&path, &case).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    std::fs::remove_dir_all(dir).unwrap();
}
