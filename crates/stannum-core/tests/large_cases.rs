//! Real cases at full size, from tests/large (or STANNUM_LARGE_CASES).
//!
//! `cargo test --release -p stannum-core --test large_cases -- --ignored`
use stannum_core::Workspace;
use stannum_core::power::SolveSettings;
use std::path::PathBuf;

fn large(name: &str) -> Option<Workspace> {
    let dir = std::env::var("STANNUM_LARGE_CASES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/large"));
    let path = dir.join(name);
    if !path.exists() {
        return None;
    }
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    Some(ws)
}

#[test]
#[ignore = "needs tests/large/SyntheticUSA.RAW"]
fn q_limits_after_a_solve_without_them_converge() {
    let Some(mut ws) = large("SyntheticUSA.RAW") else {
        return;
    };
    let case = ws.current_mut().unwrap();
    let free = case.solve(SolveSettings::default()).unwrap();
    assert!(free.converged);
    // Without limits a 25 Mvar unit at 2020453 holds its setpoint with
    // thousands of Mvar; that solution is a poor start for the limited one,
    // which then restarts from the case voltages.
    let limited = case
        .solve(SolveSettings {
            enforce_q_limits: true,
            ..Default::default()
        })
        .unwrap();
    assert!(limited.converged, "{}", limited.status);
    assert_eq!(limited.limits.reactive_violations, 0);
}

#[test]
#[ignore = "needs tests/large/SyntheticUSA.RAW"]
fn sensitivities_fit_at_82000_buses() {
    let Some(mut ws) = large("SyntheticUSA.RAW") else {
        return;
    };
    let case = ws.current_mut().unwrap();
    let start = std::time::Instant::now();
    let summary = case.sensitivities(SolveSettings::default()).unwrap();
    eprintln!("sensitivities in {:?}", start.elapsed());
    assert!(summary.injections.len() > 100_000);
    let table = case.responses(0, "flow0", true).unwrap();
    assert_eq!(table.rows.len(), 104_121);
    assert!(table.rows.iter().any(|r| r.value.abs() > 0.1));
}
