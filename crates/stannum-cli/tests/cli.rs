//! The CLI end to end: exit codes and what it prints.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_stannum-cli"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stannum-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs-power/tests/fixtures/case9.m"),
        dir.join("case9.m"),
    )
    .unwrap();
    dir
}

#[test]
fn a_run_that_works_exits_zero() {
    let dir = scratch("ok");
    let file = dir.join("solve.stnrun");
    std::fs::write(
        &file,
        "open_case path=case9.m\nsolve_power_flow method=newton start=flat\nexport_matpower path=solved9.m solved=true\n",
    )
    .unwrap();
    let out = cli().arg("run").arg(&file).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(err.contains("ok    solve_power_flow"), "{err}");
    assert!(err.contains("Ran 3 steps"), "{err}");
    assert!(dir.join("solved9.m").exists());

    // --json puts the report, with answers, on stdout.
    let out = cli().args(["run", "--json"]).arg(&file).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["outcomes"][1]["result"]["converged"], true);
    assert_eq!(report["outcomes"][1]["result"]["method"], "newton");
}

#[test]
fn a_failing_line_exits_one_and_a_bad_file_two() {
    let dir = scratch("fail");
    let file = dir.join("fail.stnrun");
    std::fs::write(
        &file,
        "open_case path=case9.m\nelement_detail element=bus:99\n",
    )
    .unwrap();
    let out = cli().arg("run").arg(&file).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("FAIL  element_detail"));

    std::fs::write(&file, "open_cas path=case9.m\n").unwrap();
    let out = cli().arg("check").arg(&file).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("Line 1: Stannum has no command called open_cas.")
    );

    let out = cli().arg("run").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn commands_lists_what_a_run_file_can_use() {
    let out = cli().arg("commands").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("open_case path="));
    assert!(!text.contains("run_file"));
}
