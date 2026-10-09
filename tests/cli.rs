use std::io::Write;
use std::process::{Command, Stdio};

fn run(commands: &str) -> std::process::Output {
    run_with_log(commands, None)
}

fn run_with_log(commands: &str, filter: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rs-power"));
    command.env_remove("RUST_LOG");
    if let Some(filter) = filter {
        command.env("RUST_LOG", filter);
    }
    let mut child = command
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
fn selected_bus_report_uses_engineering_units_on_the_case_base() {
    let mut case = rs_power::load_matpower("tests/fixtures/case9.m").unwrap();
    case.base_mva = 50.0;
    let path =
        std::env::temp_dir().join(format!("rs_power_selected_base_{}.m", std::process::id()));
    rs_power::save_matpower(&path, &case).unwrap();
    let reference = rs_power::calculate_sensitivities(&case, &Default::default()).unwrap();
    let output = run(&format!(
        "load {}\nsensitivities ac bus 2 all\n",
        path.display()
    ));
    std::fs::remove_file(path).unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{text}");
    assert!(text.contains("Injection: Bus 2 active power"), "{text}");
    assert!(text.contains("degrees/MW"), "{text}");
    assert!(text.contains("p.u./MW"), "{text}");
    assert!(text.contains("MW/MW"), "{text}");
    assert!(text.contains("MVAr/MW"), "{text}");
    let column = reference
        .injection_variables
        .iter()
        .position(|v| *v == rs_power::InjectionVariable::ActivePower(2))
        .unwrap();
    for (state, raw) in reference
        .state_variables
        .iter()
        .zip(&reference.state_sensitivities)
    {
        let (label, expected) = match state {
            rs_power::StateVariable::VoltageAngle(bus) => (
                format!("Bus {bus} voltage angle"),
                raw[column].to_degrees() / 50.0,
            ),
            rs_power::StateVariable::VoltageMagnitude(bus) => {
                (format!("Bus {bus} voltage magnitude"), raw[column] / 50.0)
            }
        };
        let line = text
            .lines()
            .find(|line| line.trim_start().starts_with(&label))
            .unwrap();
        let actual: f64 = line.split_whitespace().last().unwrap().parse().unwrap();
        assert!(
            (actual - expected).abs() <= expected.abs() * 6e-6 + 1e-12,
            "{line}: expected {expected}"
        );
    }
}

#[test]
fn selected_bus_reports_limit_each_quantity_and_keep_signs() {
    let output = run("load tests/fixtures/case9.m\nsensitivities dc bus 2 top 1\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("Voltage angle (degrees/MW; showing 1 of 8)"),
        "{text}"
    );
    assert!(
        text.contains("From-end active flow (MW/MW; showing 1 of 9)"),
        "{text}"
    );
    assert!(
        text.contains("To-end active flow (MW/MW; showing 1 of 9)"),
        "{text}"
    );
    assert!(!text.contains("reactive flow"), "{text}");
    assert!(!text.contains("Voltage magnitude ("), "{text}");
    let values: Vec<f64> = text
        .lines()
        .filter(|l| l.trim_start().starts_with("Branch "))
        .map(|l| l.split_whitespace().last().unwrap().parse().unwrap())
        .collect();
    assert_eq!(values.len(), 2, "{text}");
    assert!(values[0] * values[1] < 0.0, "{text}");
}

#[test]
fn selected_reactive_injection_supports_raw_units_and_text_export() {
    let path = std::env::temp_dir().join(format!("rs-power-selected-q-{}.txt", std::process::id()));
    let output = run(&format!(
        "load tests/fixtures/case9.m\nsensitivities ac bus 5 q units raw all to \"{}\"\n",
        path.display()
    ));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{text}");
    let report = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        report.contains("Injection: Bus 5 reactive power"),
        "{report}"
    );
    assert!(report.contains("rad/p.u."), "{report}");
    assert!(report.contains("p.u./p.u."), "{report}");
    assert!(report.contains("showing 6 of 6"), "{report}");
}

#[test]
fn invalid_selected_injections_are_rejected_before_solving() {
    for (command, message) in [
        ("sensitivities dc bus 999", "unknown sensitivity bus ID 999"),
        (
            "sensitivities dc bus 1",
            "reference-bus injection columns are omitted",
        ),
        (
            "sensitivities dc bus 2 q",
            "reactive injections are unavailable in DC",
        ),
        (
            "sensitivities ac bus 2 q",
            "reactive injection requires a PQ bus",
        ),
        (
            "sensitivities ac bus 2 top 0",
            "top must be a positive integer",
        ),
    ] {
        let output = run(&format!(
            "load tests/fixtures/case9.m\n{command}\nsummary\n"
        ));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!output.status.success(), "{command}: {text}");
        assert!(text.contains(message), "{command}: {text}");
        assert!(
            !text.contains("Computing operating point"),
            "{command}: {text}"
        );
    }
}

#[test]
fn selected_sensitivity_failure_does_not_publish_a_report() {
    let output =
        run("load tests/fixtures/case9.m\nset max_iterations 0\nsensitivities ac bus 2\nsummary\n");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "{text}");
    assert!(text.contains("sensitivities are unavailable"), "{text}");
    assert!(!text.contains("Injection: Bus 2"), "{text}");
}

#[test]
fn batch_logging_is_opt_in_and_separate_from_command_output() {
    let commands = "load tests/fixtures/case9.m\nsolve dc\n";
    let quiet = run(commands);
    assert!(quiet.status.success());
    assert!(quiet.stderr.is_empty());

    let logged = run_with_log(commands, Some("rs_power=debug"));
    assert!(logged.status.success());
    let stdout = String::from_utf8(logged.stdout).unwrap();
    let stderr = String::from_utf8(logged.stderr).unwrap();
    assert!(stdout.contains("Converged:"));
    assert!(stderr.contains("loading MATPOWER case"), "{stderr}");
    assert!(stderr.contains("starting Dc power-flow solve"), "{stderr}");
    assert!(stderr.contains("DC solve converged"), "{stderr}");
    assert!(stderr.contains("finished Dc power-flow solve"), "{stderr}");
}

#[test]
fn reports_support_text_and_csv_destinations() {
    let path = std::env::temp_dir().join(format!("rs-power-report-{}.txt", std::process::id()));
    let matrix_path =
        std::env::temp_dir().join(format!("rs-power PTDF report {}.csv", std::process::id()));
    let table_path =
        std::env::temp_dir().join(format!("rs-power bus report {}.csv", std::process::id()));
    let path_text = path.to_string_lossy();
    let matrix_path_text = matrix_path.to_string_lossy();
    let table_path_text = table_path.to_string_lossy();
    let output = run(&format!(
        "load tests/fixtures/case9.m\nsolve dc\nsummary to \"{path_text}\"\nbuses to \"{table_path_text}\"\nptdf 2 3 to \"{matrix_path_text}\"\n"
    ));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("Wrote summary report to"));
    assert!(stdout.contains("Wrote buses report to"));
    assert!(!stdout.contains("Va (degrees)"));

    let report = std::fs::read_to_string(&path).unwrap();
    assert!(report.starts_with("9 buses, 3 generators, 9 branches"));
    let table_report = std::fs::read_to_string(&table_path).unwrap();
    assert!(table_report.starts_with("bus_id,vm_pu,va_degrees\n"));
    assert!(table_report.contains("1,1,0\n"));
    let matrix_report = std::fs::read_to_string(&matrix_path).unwrap();
    assert!(matrix_report.starts_with("branch,from_bus,to_bus,P(2),P(3)\n"));
    assert_eq!(matrix_report.lines().count(), 10);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(table_path).unwrap();
    std::fs::remove_file(matrix_path).unwrap();
}

#[test]
fn failed_reports_do_not_overwrite_the_destination() {
    let path =
        std::env::temp_dir().join(format!("rs-power-failed-report-{}.txt", std::process::id()));
    std::fs::write(&path, "existing report\n").unwrap();
    let output = run(&format!(
        "load tests/fixtures/case9.m\nptdf 999 to \"{}\"\n",
        path.to_string_lossy()
    ));
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing report\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn csv_rejects_the_three_dimensional_sensitivity_report() {
    let path = std::env::temp_dir().join(format!(
        "rs-power-sensitivity-report-{}.csv",
        std::process::id()
    ));
    let output = run(&format!(
        "load tests/fixtures/case9.m\nsensitivities dc to \"{}\"\n",
        path.to_string_lossy()
    ));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success());
    assert!(
        stdout.contains("includes a 3D branch-flow tensor"),
        "{stdout}"
    );
    assert!(!path.exists());
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
