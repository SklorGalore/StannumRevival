//! Exact record-by-record comparison of two parsed cases.

use super::parser::{DiffCase, Key, Record};
use super::value::Value;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "DiffStatus")]
pub enum Status {
    Added,
    Removed,
    Changed,
    Unchanged,
}

impl Status {
    pub const ALL: [Status; 4] = [
        Status::Added,
        Status::Removed,
        Status::Changed,
        Status::Unchanged,
    ];
}

#[derive(Clone, Debug)]
pub struct FieldDiff {
    pub name: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub status: Status,
}

#[derive(Clone, Debug)]
pub struct RecordDiff {
    pub section: String,
    pub key: Key,
    pub before: Option<Record>,
    pub after: Option<Record>,
    pub fields: Vec<FieldDiff>,
    pub status: Status,
}

#[derive(Clone, Debug)]
pub struct Comparison {
    pub before: DiffCase,
    pub after: DiffCase,
    pub sections: Vec<(String, Vec<RecordDiff>)>,
}

impl Comparison {
    /// False when either case had content that could not be compared.
    pub fn complete(&self) -> bool {
        self.before.diagnostics.is_empty() && self.after.diagnostics.is_empty()
    }

    pub fn status_counts(&self, records: &[RecordDiff]) -> [usize; 4] {
        let mut counts = [0; 4];
        for r in records {
            counts[Status::ALL.iter().position(|s| *s == r.status).unwrap()] += 1;
        }
        counts
    }
}

/// Keep first-seen order of `a` then `b`, without duplicates.
fn ordered_union<'a, T: Eq + std::hash::Hash + 'a>(
    a: impl Iterator<Item = &'a T>,
    b: impl Iterator<Item = &'a T>,
) -> Vec<&'a T> {
    let mut seen = std::collections::HashSet::new();
    a.chain(b).filter(|item| seen.insert(*item)).collect()
}

fn compare_record(section: &str, before: Option<&Record>, after: Option<&Record>) -> RecordDiff {
    let empty = Vec::new();
    let bf = before.map_or(&empty, |r| &r.fields);
    let af = after.map_or(&empty, |r| &r.fields);
    let names = ordered_union(bf.iter().map(|(n, _)| n), af.iter().map(|(n, _)| n));
    let lookup = |fields: &Vec<(String, Value)>, name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let fields: Vec<FieldDiff> = names
        .into_iter()
        .map(|name| {
            let b = lookup(bf, name);
            let a = lookup(af, name);
            let status = match (&b, &a) {
                (None, _) => Status::Added,
                (_, None) => Status::Removed,
                (Some(x), Some(y)) if x == y => Status::Unchanged,
                _ => Status::Changed,
            };
            FieldDiff {
                name: name.clone(),
                before: b,
                after: a,
                status,
            }
        })
        .collect();
    let status = match (before, after) {
        (None, _) => Status::Added,
        (_, None) => Status::Removed,
        _ if fields.iter().any(|f| f.status != Status::Unchanged) => Status::Changed,
        _ => Status::Unchanged,
    };
    let key = before.or(after).expect("one side exists").key.clone();
    RecordDiff {
        section: section.to_string(),
        key,
        before: before.cloned(),
        after: after.cloned(),
        fields,
        status,
    }
}

/// Match records by their full normalized key, keeping baseline order and
/// then records only present in the comparison case.
pub fn compare_cases(before: DiffCase, after: DiffCase) -> Comparison {
    let names = ordered_union(
        before.sections.iter().map(|(n, _)| n),
        after.sections.iter().map(|(n, _)| n),
    );
    let records_of = |case: &DiffCase, name: &str| -> Vec<Record> {
        case.sections
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.clone())
            .unwrap_or_default()
    };
    let sections = names
        .into_iter()
        .map(|name| {
            let b = records_of(&before, name);
            let a = records_of(&after, name);
            let bmap: HashMap<&Key, &Record> = b.iter().map(|r| (&r.key, r)).collect();
            let amap: HashMap<&Key, &Record> = a.iter().map(|r| (&r.key, r)).collect();
            let keys = ordered_union(b.iter().map(|r| &r.key), a.iter().map(|r| &r.key));
            let diffs = keys
                .into_iter()
                .map(|k| compare_record(name, bmap.get(k).copied(), amap.get(k).copied()))
                .collect();
            (name.clone(), diffs)
        })
        .collect();
    Comparison {
        before,
        after,
        sections,
    }
}
