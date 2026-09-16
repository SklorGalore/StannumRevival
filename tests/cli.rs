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

#[test]
fn sensitivity_commands_report_labeled_matrices() {
    let output = run(
        "load tests/fixtures/case9.m\nsensitivities dc\nflows\njacobian reduced\nptdf 2 3\nlodf mask\noptions\n",
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{text}");
    assert!(text.contains("Sensitivity analysis\n  model: DC"));
    assert!(text.contains("case: 9 buses, 3 generators, 9 branches"));
    assert!(text.contains("solver: Case initialization"));
    assert!(text.contains("active-power balance: reference bus in each island"));
    assert!(text.contains("build: debug"));
    assert!(text.contains("Computing operating point and linearization..."));
    assert!(text.contains("Sensitivity analysis completed in"));
    assert!(text.contains("Converged: 1 iterations"));
    assert!(text.contains("state matrix: 8 states x 8 injection columns"));
    assert!(text.contains("branch-flow tensor: 9 branches x 8 injections x 4"));
    assert!(text.contains("State sensitivities"));
    assert!(text.contains("Va(2)        P(2)"));
    assert!(text.contains("Branch-flow sensitivities"));
    assert!(
        text.contains("PF (MW)"),
        "the sensitivity result was not retained"
    );
    assert!(text.contains("AC power-flow Jacobian"));
    assert!(text.contains("DC PTDF (MW/MW; reference-bus slack)"));
    assert!(text.contains("Branch From To P(2) P(3)"));
    assert!(text.contains("DC LODF (MW/MW; bridge handling: Mask)"));
    assert!(text.contains("model: Dc"));
}

#[test]
fn failed_sensitivity_run_reports_progress_and_failure() {
    let output =
        run("load tests/fixtures/case9.m\nset max_iterations 0\nsensitivities ac\nsummary\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success());
    assert!(text.contains("Sensitivity analysis\n  model: AC"));
    assert!(text.contains("Computing operating point and linearization..."));
    assert!(text.contains("Sensitivity analysis failed after"));
    assert!(text.contains("sensitivities are unavailable"));
    assert!(text.contains("9 buses, 3 generators, 9 branches"));
}

#[test]
fn sensitivity_command_errors_do_not_end_a_batch_session() {
    let output = run("load tests/fixtures/case9.m\nptdf unknown\nptdf 999\nlodf invalid\nptdf 2\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success());
    assert!(text.contains("invalid bus ID unknown"));
    assert!(text.contains("unknown PTDF bus ID 999"));
    assert!(text.contains("unknown command or invalid arguments"));
    assert!(text.contains("Branch From To P(2)"));
}
