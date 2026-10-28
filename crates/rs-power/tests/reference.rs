use rs_power::{
    AcMethod, BusType, Model, PowerFlowOptions, SolveStatus, load_matpower, parse_matpower, solve,
};
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn input(data: &Value) -> rs_power::Case {
    let c = &data["input"];
    let mut text = format!("mpc.version = '2';\nmpc.baseMVA = {};\n", c["baseMVA"]);
    for field in ["bus", "gen", "branch"] {
        text.push_str(&format!("mpc.{field} = [\n"));
        for row in c[field].as_array().unwrap() {
            for (j, v) in row.as_array().unwrap().iter().enumerate() {
                if v.is_null() {
                    assert_eq!(field, "gen");
                    assert!(j == 3 || j == 4);
                    text.push_str(if j == 3 { "Inf " } else { "-Inf " });
                } else {
                    text.push_str(&format!("{} ", v.as_f64().unwrap()));
                }
            }
            text.push_str(";\n");
        }
        text.push_str("];\n");
    }
    parse_matpower(&text).unwrap()
}

fn close(actual: f64, expected: &Value, tolerance: f64, context: &str) {
    let expected = expected.as_f64().unwrap();
    assert!(
        actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{context}: actual={actual:.12}, expected={expected:.12}, error={:.3e}",
        (actual - expected).abs()
    );
}

fn reference(name: &str) {
    let data = fixture(name);
    assert_eq!(data["matpower_version"], "8.1");
    let case = input(&data);
    // Every AC method stops at the same tolerance, so each must match
    // MATPOWER's Newton solution as closely as Newton does.
    let runs = [
        AcMethod::Newton,
        AcMethod::FixedSlope,
        AcMethod::FastDecoupledXb,
        AcMethod::FastDecoupledBx,
        AcMethod::Auto,
    ]
    .map(|m| (Model::Ac, m))
    .into_iter()
    .chain([(Model::Dc, AcMethod::Newton)]);
    for (model, method) in runs {
        let key = if model == Model::Ac { "ac" } else { "dc" };
        let opt = PowerFlowOptions {
            model,
            method,
            enforce_q_limits: model == Model::Ac && data["enforce_q_limits"].as_bool().unwrap(),
            ..Default::default()
        };
        let key = &format!("{key} ({})", method.label());
        let expected = &data[if model == Model::Ac { "ac" } else { "dc" }];
        let result = solve(&case, &opt).unwrap();
        assert_eq!(
            result.status == SolveStatus::Converged,
            expected["success"].as_bool().unwrap(),
            "{name} {key}: {:?}",
            result.status
        );
        if result.status != SolveStatus::Converged {
            assert_eq!(result.status, SolveStatus::ReactiveLimitsInfeasible);
            continue;
        }
        assert!(result.residual < opt.tolerance);
        for (i, b) in result.case.buses.iter().enumerate() {
            close(
                b.vm,
                &expected["bus"][i][7],
                1e-6,
                &format!("{name} {key} bus {i} Vm"),
            );
            close(
                b.va,
                &expected["bus"][i][8],
                1e-5,
                &format!("{name} {key} bus {i} Va"),
            );
            let kind = match b.kind {
                BusType::Pq => 1,
                BusType::Pv => 2,
                BusType::Reference => 3,
                BusType::Isolated => 4,
            };
            assert_eq!(
                kind,
                expected["bus"][i][1].as_f64().unwrap() as i32,
                "bus kind {name} {key} {i}"
            );
            assert_eq!(b.id, case.buses[i].id);
        }
        for (i, g) in result.case.generators.iter().enumerate() {
            close(
                g.pg,
                &expected["gen"][i][1],
                1e-4,
                &format!("{name} {key} generator {i} Pg"),
            );
            close(
                g.qg,
                &expected["gen"][i][2],
                1e-4,
                &format!("{name} {key} generator {i} Qg"),
            );
        }
        for (i, f) in result.branch_flows.iter().enumerate() {
            for (j, &x) in f.iter().enumerate() {
                close(
                    x,
                    &expected["branch"][i][13 + j],
                    1e-4,
                    &format!("{name} {key} branch {i} component {j}"),
                );
            }
        }
    }
}

macro_rules! references { ($($name:ident),*) => { $(#[test] fn $name() { reference(stringify!($name)); })* }; }
references!(
    case9,
    case14,
    case30,
    case57,
    case118,
    case300,
    q_limit,
    slack_limit,
    q_infeasible,
    shared_gen,
    infinite_q,
    zero_q_range,
    equipment
);

#[test]
fn upstream_files_load_without_conversion() {
    for name in ["case9", "case14", "case30", "case57", "case118", "case300"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("{name}.m"));
        let c = load_matpower(path).unwrap();
        let canonical = input(&fixture(name));
        assert_eq!(c.buses.len(), canonical.buses.len());
        assert!(c.gencost.is_some());
        let result = solve(&c, &PowerFlowOptions::default()).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
    }
}
