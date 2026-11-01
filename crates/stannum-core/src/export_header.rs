//! The metadata rows at the top of every CSV export.
//!
//! A table handed on as CSV loses where it came from. Each export opens with
//! a few `name,value` rows (the case file and its SHA-256, study edits, the
//! program and version, when it was exported and the run's settings), then a
//! blank row, then the table. The first row says how many rows come before
//! the table's column names, so a script can skip them:
//!
//! ```text
//! header_rows,9
//! title,Contingency violations
//! case,case30.m
//! ...
//!
//! rank,category,contingency,...
//! ```
//!
//! `pandas.read_csv(path, skiprows=9)` reads the table.

use crate::OpenCase;
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

static PROGRAM: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Name the program that runs the core (such as "Stannum 1.0.0") for the
/// metadata rows of exports. The first call wins.
pub fn set_program(name: &str, version: &str) {
    PROGRAM.get_or_init(|| format!("{name} {version}"));
}

/// The program named by [`set_program`], or the core's own version.
pub fn program() -> String {
    PROGRAM
        .get()
        .cloned()
        .unwrap_or_else(|| format!("stannum-core {}", env!("CARGO_PKG_VERSION")))
}

/// The metadata rows as CSV text, ready to go in front of a table.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct ExportHeader {
    /// The rows, the blank row after them included, each ending in CRLF.
    pub text: String,
    /// How many rows come before the table's column names (`header_rows`).
    pub rows: usize,
}

impl OpenCase {
    /// The metadata rows for an export of `title` from this case, with
    /// `settings` (a JSON object) one field per row: nested objects as
    /// `outer.inner`, lists joined with spaces, empty values left out.
    pub fn export_header(&self, title: &str, settings: Option<&Value>) -> ExportHeader {
        let mut rows = vec![
            ("title".to_string(), title.to_string()),
            ("case".to_string(), self.name.clone()),
            ("file".to_string(), self.path.display().to_string()),
        ];
        if let Ok(f) = crate::study_file::fingerprint(&self.path) {
            rows.push(("sha256".to_string(), f.sha256));
        }
        if !self.edits.is_empty() {
            rows.push((
                "study_edits".to_string(),
                format!("{} (results include them)", self.edits.len()),
            ));
        }
        rows.push(("program".to_string(), program()));
        rows.push(("exported".to_string(), utc_now()));
        if let Some(settings) = settings {
            flatten("", settings, &mut rows);
        }
        header(&rows)
    }
}

/// `rows` as CSV, after a `header_rows` row and before a blank one.
pub fn header(rows: &[(String, String)]) -> ExportHeader {
    let count = rows.len() + 2;
    let mut text = format!("header_rows,{count}\r\n");
    for (name, value) in rows {
        text.push_str(&field(name));
        text.push(',');
        text.push_str(&field(value));
        text.push_str("\r\n");
    }
    text.push_str("\r\n");
    ExportHeader { text, rows: count }
}

/// A CSV field: quoted when it holds a comma, quote or line break, and
/// with a leading quote when a spreadsheet would read it as a formula.
fn field(v: &str) -> String {
    let v = if v.trim_start().starts_with(['=', '+', '@', '-']) && v.parse::<f64>().is_err() {
        format!("'{v}")
    } else {
        v.to_string()
    };
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// Settings as `name,value` rows.
fn flatten(prefix: &str, value: &Value, out: &mut Vec<(String, String)>) {
    let text = match value {
        Value::Null => return,
        Value::String(s) if s.is_empty() => return,
        Value::Array(a) if a.is_empty() => return,
        Value::Object(fields) => {
            for (key, v) in fields {
                // Lists file text is in the lists file itself.
                if key.ends_with("_text") {
                    continue;
                }
                let key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&key, v, out);
            }
            return;
        }
        Value::String(s) => s.clone(),
        Value::Array(items) if items.iter().all(|v| !v.is_object()) => items
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" "),
        other => other.to_string(),
    };
    out.push((prefix.to_string(), text));
}

/// The time now in UTC, as `2026-11-01T17:20:03Z`.
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    utc(secs)
}

fn utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Days to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_utc_dates() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_793_553_603), "2026-11-01T17:20:03Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn the_first_row_counts_the_rows_to_skip() {
        let mut rows = vec![("title".to_string(), "A, \"B\"".to_string())];
        flatten(
            "",
            &serde_json::json!({ "sink": { "areas": [7, 8] }, "cutoff": 0.05, "lists_text": "x", "none": null }),
            &mut rows,
        );
        let h = header(&rows);
        let lines: Vec<&str> = h.text.split("\r\n").collect();
        assert_eq!(lines[0], format!("header_rows,{}", h.rows));
        assert_eq!(h.rows, 5);
        assert_eq!(lines[1], "title,\"A, \"\"B\"\"\"");
        assert!(lines.contains(&"sink.areas,7 8"));
        assert!(lines.contains(&"cutoff,0.05"));
        assert!(!h.text.contains("lists_text"));
        // header_rows lines, then the blank row; the table starts after.
        assert_eq!(lines[h.rows - 1], "");
    }

    #[test]
    fn formulas_are_neutralized() {
        assert_eq!(field("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(field("-0.5"), "-0.5");
    }
}
