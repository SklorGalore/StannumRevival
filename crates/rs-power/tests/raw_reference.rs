use rs_power::{
    Case, Model, PowerFlowOptions, RawOptions, SolveStatus, format_matpower, parse_matpower,
    parse_raw_with_options, solve,
};
use serde_json::Value;
use stannum_verify::Check;

fn expected_case(data: &Value) -> Case {
    let mut text = format!("mpc.version = '2';\nmpc.baseMVA = {};\n", data["baseMVA"]);
    for name in ["bus", "gen", "branch", "dcline"] {
        if data[name].is_null() {
            continue;
        }
        text.push_str(&format!("mpc.{name} = [\n"));
        let table = data[name].as_array().unwrap();
        let rows = if table.first().is_some_and(Value::is_number) {
            vec![&data[name]]
        } else {
            table.iter().collect()
        };
        for row in rows {
            for number in row.as_array().unwrap() {
                text.push_str(&format!("{number} "));
            }
            text.push_str(";\n");
        }
        text.push_str("];\n");
    }
    parse_matpower(&text).unwrap()
}

fn compare(actual: &Case, expected: &Case, check: &mut Check) {
    assert_eq!(actual.buses.len(), expected.buses.len());
    assert_eq!(actual.generators.len(), expected.generators.len());
    assert_eq!(actual.branches.len(), expected.branches.len());
    let numbers = |case: &Case| {
        let mut case = case.clone();
        case.labels.clear();
        format_matpower(&case)
            .unwrap()
            .split_whitespace()
            .filter_map(|s| s.trim_end_matches(';').parse::<f64>().ok())
            .collect::<Vec<_>>()
    };
    let a = numbers(actual);
    let e = numbers(expected);
    assert_eq!(a.len(), e.len());
    for (i, (a, e)) in a.iter().zip(&e).enumerate() {
        check.diff((a - e) / (1.0 + e.abs()));
        assert!(
            (a - e).abs() <= 1e-10 * (1.0 + e.abs()),
            "numeric field {i}: actual {a}, expected {e}"
        );
    }
}

/// The row name on the Verification page.
fn label(stem: &str) -> String {
    match stem {
        "dc_modes" => "DC line control modes".into(),
        "transformers" => "Every transformer CW and CZ code".into(),
        "t_case9_save2psse" => "MATPOWER t_case9_save2psse".into(),
        "t_psse_case2" => "MATPOWER t_psse_case2".into(),
        "t_psse_case3" => "MATPOWER t_psse_case3".into(),
        _ => match stem.strip_prefix("revision_") {
            Some(n) => format!("Revision {n} layout"),
            None => stem.into(),
        },
    }
}

#[test]
fn matpower_81_raw_conversion_references() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raw");
    let mut count = 0;
    let mut solved = 0;
    for file in std::fs::read_dir(&root).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|s| s != "raw") {
            continue;
        }
        eprintln!("RAW reference: {}", path.display());
        let data: Value =
            serde_json::from_str(&std::fs::read_to_string(path.with_extension("json")).unwrap())
                .unwrap();
        assert_eq!(data["matpower_version"], "8.1");
        let imported = parse_raw_with_options(
            &std::fs::read_to_string(&path).unwrap(),
            &RawOptions::default(),
        )
        .unwrap();
        let row = label(path.file_stem().unwrap().to_str().unwrap());
        let mut fields = Check::new("raw-import", &row, "Imported fields", "relative", 1e-10);
        compare(&imported.case, &expected_case(&data), &mut fields);
        fields.finish();
        let mut vm = Check::new("raw-import", &row, "Voltage magnitude", "p.u.", 1e-6);
        let mut va = Check::new("raw-import", &row, "Voltage angle", "deg", 1e-5);
        let mut power = Check::new(
            "raw-import",
            &row,
            "Generator output and branch flows",
            "MW, Mvar",
            1e-4,
        );
        let names: Vec<String> = data["bus_name"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect();
        assert_eq!(imported.case.labels["bus_name"], names);
        // Imported DC data and star buses survive literal MATPOWER export/import.
        assert_eq!(
            parse_matpower(&format_matpower(&imported.case).unwrap()).unwrap(),
            imported.case
        );
        for (name, model) in [("ac", Model::Ac), ("dc", Model::Dc)] {
            if data[name].is_null() {
                continue;
            }
            assert!(data[name]["success"] == true || data[name]["success"].as_f64() == Some(1.0));
            solved += 1;
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
            for (bus, row) in result
                .case
                .buses
                .iter()
                .zip(data[name]["bus"].as_array().unwrap())
            {
                assert!(vm.compare(bus.vm, row[7].as_f64().unwrap()) < 1e-6);
                assert!(va.compare(bus.va, row[8].as_f64().unwrap()) < 1e-5);
            }
            let gen_table = data[name]["gen"].as_array().unwrap();
            let gen_rows = if gen_table.first().is_some_and(Value::is_number) {
                vec![&data[name]["gen"]]
            } else {
                gen_table.iter().collect()
            };
            for (g, row) in result.case.generators.iter().zip(gen_rows) {
                assert!(power.compare(g.pg, row[1].as_f64().unwrap()) < 1e-4);
                assert!(power.compare(g.qg, row[2].as_f64().unwrap()) < 1e-4);
            }
            for (flows, row) in result
                .branch_flows
                .iter()
                .zip(data[name]["branch"].as_array().unwrap())
            {
                for i in 0..4 {
                    assert!(power.compare(flows[i], row[13 + i].as_f64().unwrap()) < 1e-4);
                }
            }
        }
        for check in [vm, va, power] {
            if check.compared() > 0 {
                check.finish();
            }
        }
        count += 1;
    }
    assert_eq!(count, 16);
    assert_eq!(solved, 26);
}
