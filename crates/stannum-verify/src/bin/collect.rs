//! Turns the records the reference tests appended into the Verification
//! page's table.
//!
//! ```text
//! collect <records.jsonl> <out.json> <commit> <date>
//! ```
//!
//! Fails when there are no records, or a record is over its tolerance (the
//! test that wrote it should already have failed).

use stannum_verify::{Record, SUITES, report};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [records, out, commit, date] = args.as_slice() else {
        eprintln!("usage: collect <records.jsonl> <out.json> <commit> <date>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(records)
        .unwrap_or_else(|e| fail(&format!("can't read {records}: {e}")));
    let records: Vec<Record> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| fail(&format!("line {} of {records}: {e}", i + 1)))
        })
        .collect();
    if records.is_empty() {
        fail("no records: did the tests run with STANNUM_VERIFY set?");
    }
    let over: Vec<&Record> = records.iter().filter(|r| !r.passed()).collect();
    for r in &over {
        eprintln!(
            "over tolerance: {} {} {}: {:e} against {:e}",
            r.suite, r.case, r.quantity, r.max_diff, r.tolerance
        );
    }
    let report = report(&records, SUITES, commit, date);
    let mut json = serde_json::to_string_pretty(&report).expect("the report serializes");
    json.push('\n');
    std::fs::write(out, json).unwrap_or_else(|e| fail(&format!("can't write {out}: {e}")));
    let rows: usize = report.suites.iter().map(|s| s.rows.len()).sum();
    println!(
        "{} checks in {} rows of {} suites written to {out}",
        report.checks,
        rows,
        report.suites.len()
    );
    if !over.is_empty() {
        std::process::exit(1);
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}
