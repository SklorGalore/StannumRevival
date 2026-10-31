//! Contingencies from a lists file (`.stnl`): its `contingency` records and
//! the `contingency_set` rules that generate them, read against one case.
//!
//! Lists name equipment by natural keys and don't belong to a case, so what
//! the case lacks is reported and skipped rather than refused. Monitors and
//! interfaces are read by the format crate but not applied yet.

use crate::power::PowerCase;
use stannum_format::{Document, Record, Value};
use std::collections::{HashMap, HashSet};

/// One `contingency` record: the elements its events take out.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub name: String,
    pub category: Option<String>,
    /// Element ids to take out together; `bus:N` for `trip_bus`.
    pub elements: Vec<String>,
}

/// What a `contingency_set` takes out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetKind {
    Branches,
    Generators,
    BranchPairs,
    Buses,
}

/// One `contingency_set` rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Set {
    pub name: String,
    pub kind: SetKind,
    /// Buses of its subsystem; the whole case when `None`.
    pub buses: Option<HashSet<u32>>,
    pub kv_min: Option<f64>,
}

/// A lists file read against a case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lists {
    pub named: Vec<Named>,
    pub sets: Vec<Set>,
    /// What was skipped and why, in sentences.
    pub notes: Vec<String>,
}

/// Read a lists file from disk against `power`.
pub fn load(path: &str, power: &PowerCase) -> Result<Lists, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Couldn't read the lists file {path}: {e}"))?;
    parse(&text, power)
}

/// Read lists text against `power`.
pub fn parse(text: &str, power: &PowerCase) -> Result<Lists, String> {
    let doc = stannum_format::read_lists(text)
        .map_err(|e| format!("The lists file has problems:\n{e}"))?;
    let mut out = Lists::default();
    for r in doc.records("contingency") {
        if let Some(named) = named(&doc, r, &mut out.notes) {
            out.named.push(named);
        }
    }
    let subsystems = Subsystems::new(&doc, power);
    for r in doc.records("contingency_set") {
        let kind = match doc
            .value("contingency_set", r, "kind")
            .and_then(Value::as_str)
        {
            Some("n_1_branches") => SetKind::Branches,
            Some("n_1_generators") => SetKind::Generators,
            Some("n_2_branches") => SetKind::BranchPairs,
            Some("bus_outages") => SetKind::Buses,
            other => {
                out.notes.push(format!(
                    "Contingency set \"{}\": kind {} isn't known; skipped.",
                    r.id,
                    other.unwrap_or("(none)")
                ));
                continue;
            }
        };
        let buses = match r.str("subsystem") {
            Some(name) => match subsystems.buses(name) {
                Some(b) => Some(b),
                None => {
                    out.notes.push(format!(
                        "Contingency set \"{}\": subsystem {name} isn't in the file; skipped.",
                        r.id
                    ));
                    continue;
                }
            },
            None => None,
        };
        out.sets.push(Set {
            name: r.id.clone(),
            kind,
            buses,
            kv_min: r.float("kv_min"),
        });
    }
    Ok(out)
}

/// A `contingency` record's elements, or `None` (with a note) when it has
/// events that can't be applied yet.
fn named(doc: &Document, r: &Record, notes: &mut Vec<String>) -> Option<Named> {
    let events = doc
        .value("contingency", r, "events")
        .and_then(Value::as_list)
        .unwrap_or(&[]);
    let mut elements = Vec::new();
    for e in events {
        let parts = e.as_list().unwrap_or(&[]);
        let action = parts.first().and_then(Value::as_str).unwrap_or("");
        let element = parts.get(1).and_then(Value::as_str).unwrap_or("");
        match action {
            "open" => elements.push(element.to_string()),
            "trip_bus" => {
                // The element is a bus (bus:101), or anything at one.
                let bus = element
                    .strip_prefix("bus:")
                    .map(str::to_string)
                    .unwrap_or_else(|| element.split(':').nth(1).unwrap_or("").to_string());
                elements.push(format!("bus:{bus}"));
            }
            other => {
                notes.push(format!(
                    "Contingency \"{}\": {other} events aren't applied yet; skipped.",
                    r.id
                ));
                return None;
            }
        }
    }
    if elements.is_empty() {
        notes.push(format!("Contingency \"{}\" has no events; skipped.", r.id));
        return None;
    }
    let category = r
        .str("category")
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    Some(Named {
        name: r.id.clone(),
        category,
        elements,
    })
}

/// Subsystems resolved to the case's buses.
struct Subsystems<'a> {
    doc: &'a Document,
    power: &'a PowerCase,
    by_name: HashMap<&'a str, &'a Record>,
}

impl<'a> Subsystems<'a> {
    fn new(doc: &'a Document, power: &'a PowerCase) -> Self {
        let by_name = doc
            .records("subsystem")
            .iter()
            .map(|r| (r.id.as_str(), r))
            .collect();
        Self {
            doc,
            power,
            by_name,
        }
    }

    fn buses(&self, name: &str) -> Option<HashSet<u32>> {
        self.collect(name, &mut HashSet::new())
    }

    /// A subsystem's buses: the union of its members, then narrowed by its
    /// kV filters. `seen` stops a subsystem that includes itself.
    fn collect(&self, name: &str, seen: &mut HashSet<String>) -> Option<HashSet<u32>> {
        let r = *self.by_name.get(name)?;
        if !seen.insert(name.to_string()) {
            return Some(HashSet::new());
        }
        let ints = |field: &str| -> HashSet<i64> {
            self.doc
                .value("subsystem", r, field)
                .and_then(Value::as_list)
                .unwrap_or(&[])
                .iter()
                .filter_map(Value::as_i64)
                .collect()
        };
        let (areas, zones, owners, buses) =
            (ints("areas"), ints("zones"), ints("owners"), ints("buses"));
        let mut out: HashSet<u32> = HashSet::new();
        for b in &self.power.case.buses {
            let owner = self.power.bus_owners.get(&b.id).copied();
            if areas.contains(&(b.area as i64))
                || zones.contains(&(b.zone as i64))
                || owner.is_some_and(|o| owners.contains(&(o as i64)))
                || buses.contains(&(b.id as i64))
            {
                out.insert(b.id);
            }
        }
        for sub in self
            .doc
            .value("subsystem", r, "subsystems")
            .and_then(Value::as_list)
            .unwrap_or(&[])
        {
            if let Some(n) = sub.as_str()
                && let Some(more) = self.collect(n, seen)
            {
                out.extend(more);
            }
        }
        let (lo, hi) = (r.float("kv_min"), r.float("kv_max"));
        if lo.is_some() || hi.is_some() {
            let kv: HashMap<u32, f64> = self
                .power
                .case
                .buses
                .iter()
                .map(|b| (b.id, b.base_kv))
                .collect();
            out.retain(|id| {
                let v = kv.get(id).copied().unwrap_or(0.0);
                lo.is_none_or(|lo| v >= lo) && hi.is_none_or(|hi| v <= hi)
            });
        }
        Some(out)
    }
}
