//! Records how close each reference test comes to its reference.
//!
//! A test makes a [`Check`] for each quantity it compares, feeds it every
//! difference, and calls [`Check::finish`]. Finishing asserts the largest
//! difference is within the tolerance. When `STANNUM_VERIFY` names a file,
//! it also appends one JSON line to it; `scripts/verify.ps1` runs the tests
//! that way and the `collect` binary turns the lines into the table the
//! documentation's Verification page shows. Without the variable a check
//! costs a few comparisons and writes nothing.

use serde::{Deserialize, Serialize};
use std::io::Write;

/// The environment variable naming the file records are appended to.
pub const ENV: &str = "STANNUM_VERIFY";

/// The order suites appear on the page.
pub const SUITES: &[&str] = &[
    "matpower",
    "raw-import",
    "saved-solution",
    "vsc-paper",
    "matacdc",
    "contingency",
    "transfer",
    "pv",
    "transformer-detail",
];

/// One finished check: the largest difference from the reference for one
/// quantity of one case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    /// Which group of checks on the page, such as `matpower`.
    pub suite: String,
    /// The case, or case and run, the row is about.
    pub case: String,
    /// The column: what was compared.
    pub quantity: String,
    pub unit: String,
    /// The largest absolute difference (or relative, when the unit says so).
    pub max_diff: f64,
    pub tolerance: f64,
    /// How many values were compared.
    pub compared: usize,
}

impl Record {
    /// Whether the largest difference is within the tolerance.
    pub fn passed(&self) -> bool {
        self.max_diff <= self.tolerance
    }
}

/// Tracks the largest difference for one quantity of one case.
#[derive(Debug, Clone)]
pub struct Check {
    record: Record,
}

impl Check {
    pub fn new(suite: &str, case: &str, quantity: &str, unit: &str, tolerance: f64) -> Self {
        Check {
            record: Record {
                suite: suite.into(),
                case: case.into(),
                quantity: quantity.into(),
                unit: unit.into(),
                max_diff: 0.0,
                tolerance,
                compared: 0,
            },
        }
    }

    /// Notes one difference and returns its size. A NaN counts as infinite.
    pub fn diff(&mut self, d: f64) -> f64 {
        let d = if d.is_nan() { f64::INFINITY } else { d.abs() };
        self.record.max_diff = self.record.max_diff.max(d);
        self.record.compared += 1;
        d
    }

    /// Notes `actual - expected` and returns its size.
    pub fn compare(&mut self, actual: f64, expected: f64) -> f64 {
        self.diff(actual - expected)
    }

    pub fn tolerance(&self) -> f64 {
        self.record.tolerance
    }

    /// How many values were compared so far.
    pub fn compared(&self) -> usize {
        self.record.compared
    }

    /// The largest difference so far.
    pub fn max(&self) -> f64 {
        self.record.max_diff
    }

    /// Asserts the largest difference is within the tolerance and, when
    /// [`ENV`] is set, appends the record. A check that compared nothing
    /// panics, so a reference that silently went empty can't pass.
    pub fn finish(self) -> Record {
        let r = self.record;
        assert!(
            r.compared > 0,
            "{} {} {}: nothing compared",
            r.suite,
            r.case,
            r.quantity
        );
        assert!(
            r.passed(),
            "{} {} {}: {:e} {} against a tolerance of {:e}",
            r.suite,
            r.case,
            r.quantity,
            r.max_diff,
            r.unit,
            r.tolerance
        );
        if let Some(path) = std::env::var_os(ENV) {
            let mut line = serde_json::to_string(&r).expect("a record serializes");
            line.push('\n');
            // One write per line, so parallel tests append whole lines.
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap_or_else(|e| panic!("can't open {}: {e}", path.to_string_lossy()));
            file.write_all(line.as_bytes())
                .unwrap_or_else(|e| panic!("can't write {}: {e}", path.to_string_lossy()));
        }
        r
    }
}

/// The table the Verification page reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    /// The commit the checks ran on.
    pub commit: String,
    /// When they ran, as an RFC 3339 time.
    pub date: String,
    pub checks: usize,
    pub suites: Vec<Suite>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suite {
    pub id: String,
    /// Column names, in the order the tests recorded them.
    pub quantities: Vec<Quantity>,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quantity {
    pub name: String,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub case: String,
    /// One cell per quantity, `None` where the case doesn't compare it.
    pub cells: Vec<Option<Cell>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cell {
    pub max_diff: f64,
    pub tolerance: f64,
    pub compared: usize,
}

/// Groups records by suite and case. Suites keep the order of `order`
/// (others follow alphabetically) and cases sort with numbers compared as
/// numbers, so case9 comes before case14. A record repeated for the same
/// cell keeps its largest difference.
pub fn report(records: &[Record], order: &[&str], commit: &str, date: &str) -> Report {
    let mut ids: Vec<&str> = Vec::new();
    for r in records {
        if !ids.contains(&r.suite.as_str()) {
            ids.push(&r.suite);
        }
    }
    let rank = |id: &str| order.iter().position(|o| *o == id).unwrap_or(order.len());
    ids.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.cmp(b)));
    let suites = ids
        .into_iter()
        .map(|id| {
            let mine: Vec<&Record> = records.iter().filter(|r| r.suite == id).collect();
            let mut quantities: Vec<Quantity> = Vec::new();
            let mut cases: Vec<&str> = Vec::new();
            for r in &mine {
                if !quantities.iter().any(|q| q.name == r.quantity) {
                    quantities.push(Quantity {
                        name: r.quantity.clone(),
                        unit: r.unit.clone(),
                    });
                }
                if !cases.contains(&r.case.as_str()) {
                    cases.push(&r.case);
                }
            }
            cases.sort_by(|a, b| natural(a, b));
            let rows = cases
                .into_iter()
                .map(|case| Row {
                    case: case.into(),
                    cells: quantities
                        .iter()
                        .map(|q| {
                            mine.iter()
                                .filter(|r| r.case == case && r.quantity == q.name)
                                .map(|r| Cell {
                                    max_diff: r.max_diff,
                                    tolerance: r.tolerance,
                                    compared: r.compared,
                                })
                                .reduce(|a, b| if b.max_diff > a.max_diff { b } else { a })
                        })
                        .collect(),
                })
                .collect();
            Suite {
                id: id.into(),
                quantities,
                rows,
            }
        })
        .collect();
    Report {
        commit: commit.into(),
        date: date.into(),
        checks: records.len(),
        suites,
    }
}

/// Compares strings with runs of digits compared by value.
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let mut parts: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match parts.last_mut() {
                Some((d, p)) if *d == digit => p.push(c),
                _ => parts.push((digit, c.to_string())),
            }
        }
        parts
    };
    let (pa, pb) = (split(a), split(b));
    for (x, y) in pa.iter().zip(&pb) {
        let o = match (x.0, y.0) {
            (true, true) => {
                let (u, v) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                u.len().cmp(&v.len()).then(u.cmp(v))
            }
            _ => x.1.cmp(&y.1),
        };
        if o.is_ne() {
            return o;
        }
    }
    pa.len().cmp(&pb.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_keeps_the_largest_difference() {
        let mut c = Check::new("s", "case9", "Vm", "p.u.", 1e-6);
        c.compare(1.0, 1.0 + 2e-7);
        c.compare(1.0, 1.0 - 5e-7);
        c.diff(1e-7);
        let r = c.finish();
        assert!((r.max_diff - 5e-7).abs() < 1e-12);
        assert_eq!(r.compared, 3);
    }

    #[test]
    #[should_panic(expected = "against a tolerance")]
    fn a_check_over_its_tolerance_fails() {
        let mut c = Check::new("s", "case9", "Vm", "p.u.", 1e-6);
        c.compare(1.0, 1.1);
        c.finish();
    }

    #[test]
    #[should_panic(expected = "nothing compared")]
    fn an_empty_check_fails() {
        Check::new("s", "case9", "Vm", "p.u.", 1e-6).finish();
    }

    #[test]
    fn nan_counts_as_infinite() {
        let mut c = Check::new("s", "c", "q", "u", 1.0);
        assert_eq!(c.diff(f64::NAN), f64::INFINITY);
    }

    #[test]
    fn records_group_into_suites_and_rows() {
        let rec = |suite: &str, case: &str, q: &str, d: f64| Record {
            suite: suite.into(),
            case: case.into(),
            quantity: q.into(),
            unit: "p.u.".into(),
            max_diff: d,
            tolerance: 1.0,
            compared: 1,
        };
        let records = [
            rec("b", "x", "q", 0.1),
            rec("a", "case14", "Va", 0.2),
            rec("a", "case9", "Vm", 0.3),
            rec("a", "case9", "Va", 0.4),
            rec("a", "case9", "Vm", 0.5),
        ];
        let r = report(&records, &["a"], "abc", "2026-10-31");
        assert_eq!(r.checks, 5);
        let ids: Vec<_> = r.suites.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        let a = &r.suites[0];
        let names: Vec<_> = a.quantities.iter().map(|q| q.name.as_str()).collect();
        assert_eq!(names, ["Va", "Vm"]);
        let cases: Vec<_> = a.rows.iter().map(|r| r.case.as_str()).collect();
        assert_eq!(cases, ["case9", "case14"]);
        // case9 Vm was recorded twice; the larger difference stands.
        assert_eq!(a.rows[0].cells[1].as_ref().unwrap().max_diff, 0.5);
        assert!(a.rows[1].cells[1].is_none());
    }
}
