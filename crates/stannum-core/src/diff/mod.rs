//! RAW case comparison (ported from case-diff).
//!
//! The comparison is kept in memory on the backend; the frontend asks for a
//! summary, then one section's record list, then one record's fields. This
//! keeps large cases (thousands of buses) responsive.

pub mod compare;
pub mod parser;
pub mod tokenizer;
pub mod value;

pub use compare::{Comparison, Status, compare_cases};
pub use parser::{DiffCase, RawParseError, parse_bytes, parse_path};

use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct CaseInfo {
    pub name: String,
    pub revision: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct StatusCounts {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub unchanged: usize,
}

#[derive(Serialize, Debug, Clone)]
pub struct SectionSummary {
    pub name: String,
    pub counts: StatusCounts,
}

#[derive(Serialize, Debug, Clone)]
pub struct DiagnosticRow {
    /// "baseline" or "comparison".
    pub side: String,
    pub file: String,
    pub section: String,
    pub line: usize,
    pub message: String,
    pub raw_lines: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DiffSummary {
    pub baseline: CaseInfo,
    pub comparison: CaseInfo,
    pub complete: bool,
    /// Records that are added, removed or changed, across all sections.
    pub differing: usize,
    pub sections: Vec<SectionSummary>,
    pub diagnostics: Vec<DiagnosticRow>,
}

#[derive(Serialize, Debug, Clone)]
pub struct RecordRow {
    /// Position within the section, used to request details.
    pub index: usize,
    pub key: String,
    pub name: String,
    pub status: Status,
    /// Shared element id (see `crate::elements`) for sections other modules show.
    pub element: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct FieldRow {
    pub name: String,
    /// `None` means the field is absent on that side.
    pub before: Option<String>,
    pub after: Option<String>,
    pub status: Status,
}

#[derive(Serialize, Debug, Clone)]
pub struct RecordDetail {
    pub section: String,
    pub key: String,
    pub status: Status,
    pub fields: Vec<FieldRow>,
    pub before_lines: Option<Vec<String>>,
    pub after_lines: Option<Vec<String>>,
}

/// The shared element id of a record, for sections that hold one.
///
/// Key fields follow raw.schema.json: BUS `I`; LOAD and GENERATOR `I, ID`;
/// BRANCH and SYSTEM SWITCHING DEVICE `I, J, CKT`; TRANSFORMER `I, J, K, CKT`.
pub fn record_element(section: &str, key: &[value::Value]) -> Option<String> {
    let int = |i: usize| -> Option<u32> {
        match key.get(i)? {
            value::Value::Number(n) => n.to_string().parse().ok(),
            value::Value::Text(t) => t.trim().parse().ok(),
        }
    };
    let id = |i: usize| -> String {
        match key.get(i) {
            Some(value::Value::Text(t)) if !t.trim().is_empty() => t.trim().to_string(),
            Some(value::Value::Number(n)) => n.to_string(),
            _ => "1".to_string(),
        }
    };
    let branch_id = |kind: &str, buses: &mut Vec<u32>, ckt: String| {
        buses.sort_unstable();
        let joined = buses
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(":");
        format!("{kind}:{joined}:{ckt}")
    };
    match section {
        "BUS" => Some(crate::elements::bus(int(0)?)),
        "GENERATOR" => Some(crate::elements::generator(int(0)?, &id(1))),
        "LOAD" => Some(format!("load:{}:{}", int(0)?, id(1))),
        "BRANCH" => Some(branch_id("line", &mut vec![int(0)?, int(1)?], id(2))),
        "SYSTEM SWITCHING DEVICE" => Some(branch_id("switch", &mut vec![int(0)?, int(1)?], id(2))),
        "TRANSFORMER" => {
            let mut buses = vec![int(0)?, int(1)?];
            if let Some(k) = int(2).filter(|k| *k != 0) {
                buses.push(k);
            }
            Some(branch_id("transformer", &mut buses, id(3)))
        }
        _ => None,
    }
}

fn key_text(key: &[value::Value]) -> String {
    key.iter()
        .map(value::Value::display)
        .collect::<Vec<_>>()
        .join(" / ")
}

impl Comparison {
    pub fn summary(&self) -> DiffSummary {
        let mut differing = 0;
        let sections = self
            .sections
            .iter()
            .map(|(name, records)| {
                let [added, removed, changed, unchanged] = self.status_counts(records);
                differing += added + removed + changed;
                SectionSummary {
                    name: name.clone(),
                    counts: StatusCounts {
                        added,
                        removed,
                        changed,
                        unchanged,
                    },
                }
            })
            .collect();
        let mut diagnostics = Vec::new();
        for (side, case) in [("baseline", &self.before), ("comparison", &self.after)] {
            for d in &case.diagnostics {
                diagnostics.push(DiagnosticRow {
                    side: side.into(),
                    file: case.name.clone(),
                    section: d.section.clone(),
                    line: d.line_number,
                    message: d.message.clone(),
                    raw_lines: d.raw_lines.clone(),
                });
            }
        }
        DiffSummary {
            baseline: CaseInfo {
                name: self.before.name.clone(),
                revision: self.before.revision,
            },
            comparison: CaseInfo {
                name: self.after.name.clone(),
                revision: self.after.revision,
            },
            complete: self.complete(),
            differing,
            sections,
            diagnostics,
        }
    }

    pub fn section_records(&self, section: &str) -> Vec<RecordRow> {
        let name_field = match section {
            "AREA" => "ARNAME",
            "ZONE" => "ZONAME",
            "OWNER" => "OWNAME",
            _ => "NAME",
        };
        self.sections
            .iter()
            .find(|(n, _)| n == section)
            .map(|(_, records)| {
                records
                    .iter()
                    .enumerate()
                    .map(|(index, r)| RecordRow {
                        index,
                        key: key_text(&r.key),
                        name: r
                            .after
                            .as_ref()
                            .or(r.before.as_ref())
                            .and_then(|rec| rec.field(name_field))
                            .map(value::Value::display)
                            .unwrap_or_default(),
                        status: r.status,
                        element: record_element(section, &r.key),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn record_detail(&self, section: &str, index: usize) -> Option<RecordDetail> {
        let (_, records) = self.sections.iter().find(|(n, _)| n == section)?;
        let r = records.get(index)?;
        Some(RecordDetail {
            section: section.to_string(),
            key: key_text(&r.key),
            status: r.status,
            fields: r
                .fields
                .iter()
                .map(|f| FieldRow {
                    name: f.name.clone(),
                    before: f.before.as_ref().map(value::Value::display),
                    after: f.after.as_ref().map(value::Value::display),
                    status: f.status,
                })
                .collect(),
            before_lines: r.before.as_ref().map(|x| x.raw_lines.clone()),
            after_lines: r.after.as_ref().map(|x| x.raw_lines.clone()),
        })
    }
}
