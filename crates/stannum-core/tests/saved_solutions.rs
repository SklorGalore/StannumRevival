//! Cases saved solved by other tools: opened and solved as the app does,
//! with its default settings, the voltages land on the saved ones.
//!
//! The RAW files hold the other tool's solution in each bus record. The
//! limits are set by what a planner needs when checking one tool against
//! another (0.001 p.u. at any bus, 0.05 degrees), not by what the cases
//! happen to reach; the Verification page shows the measured differences
//! beside them.

use stannum_core::Workspace;
use stannum_core::power::SolveSettings;
use stannum_verify::Check;
use std::collections::HashMap;
use std::path::PathBuf;

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../nodal")
        .join(name)
}

/// Saved voltage magnitude and angle by bus number.
fn saved(name: &str) -> HashMap<u32, (f64, f64)> {
    let import =
        rs_power::load_raw_with_options(path(name), &rs_power::RawOptions::default()).unwrap();
    import
        .case
        .buses
        .iter()
        .map(|b| (b.id, (b.vm, b.va)))
        .collect()
}

/// Mean and largest magnitude difference, and largest angle difference.
fn compare(name: &str, row: &str, tolerance: [f64; 3]) {
    let saved = saved(name);
    let mut ws = Workspace::default();
    ws.open(&path(name)).unwrap();
    let report = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings::default())
        .unwrap();
    assert!(report.converged, "{name}: {}", report.status);
    let mut mean = Check::new(
        "saved-solution",
        row,
        "Mean voltage magnitude",
        "p.u.",
        tolerance[0],
    );
    let mut vm = Check::new(
        "saved-solution",
        row,
        "Largest voltage magnitude",
        "p.u.",
        tolerance[1],
    );
    let mut va = Check::new(
        "saved-solution",
        row,
        "Largest voltage angle",
        "deg",
        tolerance[2],
    );
    let mut total = 0.0;
    let mut n = 0;
    let mut worst = (0.0, 0);
    for bus in report.buses.iter() {
        let Some(&(svm, sva)) = saved.get(&bus.id) else {
            continue;
        };
        let d = vm.compare(bus.vm, svm);
        if d > worst.0 {
            worst = (d, bus.id);
        }
        va.compare(bus.va, sva);
        total += d;
        n += 1;
    }
    assert!(
        n * 10 > saved.len() * 9,
        "{name}: {n} of {} buses matched",
        saved.len()
    );
    mean.diff(total / n as f64);
    eprintln!(
        "{name}: mean {:.5} p.u., worst {:.5} p.u. at bus {}, angle {:.4} deg over {n} buses",
        total / n as f64,
        worst.0,
        worst.1,
        va.max()
    );
    mean.finish();
    vm.finish();
    va.finish();
}

#[test]
fn texas_2000_bus_matches_its_saved_powerworld_solution() {
    compare(
        "Texas2k_series25_case1_summerpeak.RAW",
        "Texas 2000-bus summer peak (RAW 35, PowerWorld)",
        [0.0001, 0.001, 0.05],
    );
}

#[test]
fn memphis_matches_its_saved_solution() {
    compare(
        "MemphisCase2026_Mar7.RAW",
        "Memphis 993-bus (RAW 35, PowerWorld)",
        [0.0001, 0.001, 0.05],
    );
}

#[test]
fn compare_file_voltages_matches_the_saved_solution_check() {
    // The app's "Compare with the file" gives the same numbers as the
    // check above, and sorts the largest difference first.
    let name = "Texas2k_series25_case1_summerpeak.RAW";
    let mut ws = Workspace::default();
    ws.open(&path(name)).unwrap();
    let case = ws.current_mut().unwrap();
    case.solve(SolveSettings::default()).unwrap();
    let c = case.compare_file_voltages(10).unwrap();
    assert!(c.compared > 2000, "{}", c.compared);
    assert_eq!(c.rows.len(), 10);
    assert_eq!(c.rows[0].dvm.abs(), c.max_dvm);
    assert_eq!(Some(c.rows[0].bus), c.max_dvm_bus);
    assert!(
        c.mean_dvm < 0.0001 && c.max_dvm < 0.001,
        "{} {}",
        c.mean_dvm,
        c.max_dvm
    );
    assert_eq!(c.within_0_001, c.compared);
    assert!(!c.with_study_edits);
}
