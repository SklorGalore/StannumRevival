use std::io::Write;
use std::process::{Command, Stdio};

fn run(commands: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rs-power"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(commands.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn solve_and_inspect_session() {
    let output = run(
        "load \"tests/fixtures/case9.m\"\nset initialization flat\nsolve ac\nbuses\ngenerators\nflows\nsolve dc\nquit\n",
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{text}");
    assert!(text.contains("9 buses, 3 generators, 9 branches"));
    assert_eq!(text.matches("Converged:").count(), 2);
    assert!(text.contains("Vm (p.u.)"));
    assert!(text.contains("Qg (MVAr)"));
    assert!(text.contains("PF (MW)"));
    assert!(!text.contains("rs-power>"));
}

#[test]
fn errors_recover_and_batch_exit_reports_failure() {
    let output =
        run("solve\nload tests/fixtures/case9.m\nload missing_file.m\nset tolerance NaN\nsolve\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success());
    assert!(text.contains("load a case first"));
    assert!(text.contains("tolerance must be finite and positive"));
    assert!(text.contains("Converged:"));
}

#[test]
fn nonconvergence_is_inspectable() {
    let output = run("load tests/fixtures/case9.m\nset max_iterations 0\nsolve\nbuses\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success());
    assert!(text.contains("IterationLimit:"));
    assert!(text.contains("Vm (p.u.)"));
}

#[test]
fn update_on_solve_defaults_to_true_and_can_be_disabled() {
    let output = run("options\nload tests/fixtures/case9.m\nsolve\nset max_iterations 0\nsolve\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{text}");
    assert!(text.contains("update_on_solve: true"));
    assert!(text.contains("Converged: 0 iterations"));

    let output = run(
        "set update_on_solve false\noptions\nload tests/fixtures/case9.m\nsolve\nset max_iterations 0\nsolve\n",
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success());
    assert!(text.contains("update_on_solve: false"));
    assert!(text.contains("IterationLimit:"));
}

#[test]
fn residual_is_reported_in_mva_for_solve_and_summary() {
    let case = rs_power::load_matpower("tests/fixtures/case9.m").unwrap();
    let result = rs_power::solve(
        &case,
        &rs_power::PowerFlowOptions {
            max_iterations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let expected = format!("residual {:.3e} MVA", result.residual * case.base_mva);
    let output = run("load tests/fixtures/case9.m\nset max_iterations 0\nsolve\nsummary\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.matches(&expected).count(), 2, "{text}");
}

#[test]
fn residual_units_switch_without_resolving() {
    let case = rs_power::load_matpower("tests/fixtures/case9.m").unwrap();
    let result = rs_power::solve(
        &case,
        &rs_power::PowerFlowOptions {
            max_iterations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let output = run(
        "load tests/fixtures/case9.m\nset residual pu\noptions\nset max_iterations 0\nsolve\nsummary\nset residual mva\noptions\nsummary\nset residual invalid\nsummary\n",
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("residual: pu"));
    assert!(text.contains("residual: mva"));
    assert_eq!(
        text.matches(&format!("residual {:.3e} p.u.", result.residual))
            .count(),
        2,
        "{text}"
    );
    assert_eq!(
        text.matches(&format!(
            "residual {:.3e} MVA",
            result.residual * case.base_mva
        ))
        .count(),
        2,
        "{text}"
    );
    assert!(text.contains("invalid setting"));
}
