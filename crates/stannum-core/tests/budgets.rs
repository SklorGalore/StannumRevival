//! Time and size budgets on a large case.
//!
//! A synthetic case (see `stannum-synth`) is built, opened and worked through
//! the same commands the app sends. Each command must answer within its time
//! budget, its answer must stay under its size budget, and quick commands
//! must keep answering while a solve runs. Budgets have room for a slower
//! machine: a miss means something got several times slower, not noise.
//!
//! It takes a few minutes, so it only runs when asked, in release:
//! `cargo test --release -p stannum-core --test budgets -- --ignored --nocapture`.
//! `STANNUM_BUDGET_BUSES` sets the case size (12,000 by default), and
//! `STANNUM_BUDGET_CACHE` names a folder to keep the built case in and reuse.

use serde_json::{Value, json};
use stannum_core::commands::Session;
use stannum_core::power::SolveSettings;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The case size the budgets below are set for. Other sizes scale them.
const BUDGET_BUSES: f64 = 12_000.0;

/// Each command's budget at [`BUDGET_BUSES`]: seconds, and megabytes of JSON
/// in its answer. Times have four to six times the room a developer machine
/// needs; sizes, which don't depend on the machine, about half as much again.
const BUDGETS: &[(&str, f64, f64)] = &[
    ("open_case", 2.0, 0.1),
    ("load_coordinates", 1.0, 0.1),
    ("solve_power_flow", 3.0, 4.0),
    // How long a quick command waits while a solve runs.
    ("case_summary during a solve", 0.25, 0.1),
    ("result_report base", 0.5, 4.0),
    ("diagram_network", 0.5, 6.5),
    ("bus_coordinates", 0.5, 0.75),
    ("topology_snapshot", 2.0, 20.0),
    ("case_records", 0.5, 0.1),
    ("record_table (largest)", 0.5, 4.5),
    ("run_contingencies", 90.0, 0.5),
    ("contingency_rows 100", 0.5, 0.1),
];

struct Measured {
    what: String,
    seconds: f64,
    bytes: usize,
}

fn run(s: &Session, out: &mut Vec<Measured>, what: &str, command: &str, args: Value) -> Value {
    let start = Instant::now();
    let text = s
        .invoke(command, args)
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    out.push(Measured {
        what: what.into(),
        seconds: start.elapsed().as_secs_f64(),
        bytes: text.len(),
    });
    serde_json::from_str(&text).unwrap()
}

#[test]
#[ignore = "large: a few minutes in release"]
fn a_large_case_stays_within_budget() {
    let buses: usize = std::env::var("STANNUM_BUDGET_BUSES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(BUDGET_BUSES as usize);
    // The same seed always builds the same case, so a kept copy is reused
    // (CI keeps one until the generator changes).
    let (dir, keep) = match std::env::var("STANNUM_BUDGET_CACHE") {
        Ok(d) => (std::path::PathBuf::from(d), true),
        Err(_) => (
            std::env::temp_dir().join(format!("stannum-budgets-{}", std::process::id())),
            false,
        ),
    };
    std::fs::create_dir_all(&dir).unwrap();
    let case_path = dir.join(format!("synthetic_{buses}_seed1.m"));
    let coords_path = dir.join(format!("synthetic_{buses}_seed1.coords.csv"));
    if case_path.exists() && coords_path.exists() {
        eprintln!("reusing {}", case_path.display());
    } else {
        let start = Instant::now();
        let mut config = stannum_synth::Config::for_buses(buses);
        config.seed = 1;
        let synth = stannum_synth::generate(&config);
        std::fs::write(&case_path, rs_power::format_matpower(&synth.case).unwrap()).unwrap();
        std::fs::write(&coords_path, synth.coordinates_csv()).unwrap();
        eprintln!(
            "built a {}-bus case in {:.0} s (ac solve converged: {})",
            synth.report.buses,
            start.elapsed().as_secs_f64(),
            synth.report.ac.converged
        );
    }
    let bus_count = std::fs::read_to_string(&coords_path)
        .unwrap()
        .lines()
        .count()
        - 1;

    let s = Arc::new(Session::new());
    let mut out = Vec::new();
    let settings = serde_json::to_value(SolveSettings::default()).unwrap();

    run(
        &s,
        &mut out,
        "open_case",
        "open_case",
        json!({ "path": case_path }),
    );
    run(
        &s,
        &mut out,
        "load_coordinates",
        "load_coordinates",
        json!({ "path": coords_path }),
    );

    // A solve, with a second caller asking for the summary while it runs.
    let done = Arc::new(AtomicBool::new(false));
    let waits = {
        let (s, done) = (s.clone(), done.clone());
        std::thread::spawn(move || {
            let (mut longest, mut bytes) = (Duration::ZERO, 0);
            // Ask straight away and then every 10 ms until the solve ends.
            loop {
                let t = Instant::now();
                bytes = s
                    .invoke("case_summary", json!({}))
                    .unwrap()
                    .len()
                    .max(bytes);
                longest = longest.max(t.elapsed());
                if done.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            (longest, bytes)
        })
    };
    let report = run(
        &s,
        &mut out,
        "solve_power_flow",
        "solve_power_flow",
        json!({ "settings": settings }),
    );
    done.store(true, Ordering::Relaxed);
    let (longest, bytes) = waits.join().unwrap();
    out.push(Measured {
        what: "case_summary during a solve".into(),
        seconds: longest.as_secs_f64(),
        bytes,
    });
    assert_eq!(report["converged"], json!(true), "the case solves");

    run(
        &s,
        &mut out,
        "result_report base",
        "result_report",
        json!({ "name": "base" }),
    );
    run(
        &s,
        &mut out,
        "diagram_network",
        "diagram_network",
        json!({}),
    );
    run(
        &s,
        &mut out,
        "bus_coordinates",
        "bus_coordinates",
        json!({}),
    );
    run(
        &s,
        &mut out,
        "topology_snapshot",
        "topology_snapshot",
        json!({}),
    );
    let book = run(&s, &mut out, "case_records", "case_records", json!({}));
    let largest = book["sections"]
        .as_array()
        .unwrap()
        .iter()
        .max_by_key(|s| s["count"].as_u64().unwrap_or(0))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    run(
        &s,
        &mut out,
        "record_table (largest)",
        "record_table",
        json!({ "section": largest }),
    );
    run(
        &s,
        &mut out,
        "run_contingencies",
        "run_contingencies",
        json!({ "settings": { "solve": settings } }),
    );
    run(
        &s,
        &mut out,
        "contingency_rows 100",
        "contingency_rows",
        json!({ "offset": 0, "limit": 100 }),
    );
    if !keep {
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Report every command, then fail on any over budget.
    let mut table = format!(
        "### Large case budgets ({} buses)\n\n| Command | Seconds | Budget | MB | Budget |\n| --- | --- | --- | --- | --- |\n",
        bus_count
    );
    let scale = bus_count as f64 / BUDGET_BUSES;
    let mut over = Vec::new();
    for m in &out {
        let &(_, seconds, megabytes) = BUDGETS
            .iter()
            .find(|b| b.0 == m.what)
            .unwrap_or_else(|| panic!("no budget for {}", m.what));
        let (seconds, megabytes) = (seconds * scale.max(1.0), megabytes * scale.max(1.0));
        let mb = m.bytes as f64 / 1e6;
        let _ = writeln!(
            table,
            "| {} | {:.2} | {seconds:.2} | {mb:.2} | {megabytes:.2} |",
            m.what, m.seconds
        );
        if m.seconds > seconds {
            over.push(format!(
                "{} took {:.2} s (budget {seconds:.2} s)",
                m.what, m.seconds
            ));
        }
        if mb > megabytes {
            over.push(format!(
                "{} sent {mb:.2} MB (budget {megabytes:.2} MB)",
                m.what
            ));
        }
    }
    eprintln!("{table}");
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
            let _ = writeln!(f, "{table}");
        }
    }
    assert!(over.is_empty(), "over budget:\n{}", over.join("\n"));
}
