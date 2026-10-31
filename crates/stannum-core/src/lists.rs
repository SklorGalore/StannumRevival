//! Contingencies from a lists file (`.stnl`): its `contingency` records and
//! the `contingency_set` rules that generate them, read against one case.
//!
//! Its `monitor` records (with the `interface` records they name) say what
//! to check after each outage, and its `transfer` records name transfers to
//! find the limit of. Lists name equipment by natural keys and
//! don't belong to a case, so what the case lacks is reported and skipped
//! rather than refused.

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

/// What a `monitor` record watches.
#[derive(Clone, Debug, PartialEq)]
pub enum Watched {
    /// Branch flows: one element, or every branch with an end in the
    /// buses (the whole case when `None`).
    Flow {
        element: Option<String>,
        buses: Option<HashSet<u32>>,
        /// Rating slot 1-3 (A-C) when the record names one.
        rating: Option<u8>,
    },
    /// Bus voltages, with limits that replace the buses' own.
    Voltage {
        element: Option<String>,
        buses: Option<HashSet<u32>>,
        vmin: Option<f64>,
        vmax: Option<f64>,
    },
    /// An interface's summed flow against its MW limit.
    Interface(Interface),
}

/// One `monitor` record.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: String,
    pub watched: Watched,
    /// Percent of the limit that counts as a violation.
    pub limit_pct: f64,
}

/// One `interface` record: branches as written (`[element, weight]`), the
/// flow of each measured from the first bus of its id.
#[derive(Clone, Debug, PartialEq)]
pub struct Interface {
    pub name: String,
    pub elements: Vec<(String, f64)>,
    pub limit_mw: Option<f64>,
}

/// One `transfer` record, its subsystems resolved to the case's buses.
#[derive(Clone, Debug, PartialEq)]
pub struct Transfer {
    pub name: String,
    pub source: HashSet<u32>,
    pub sink: HashSet<u32>,
    /// `generation`, `reserve` or `load`.
    pub scaling: String,
    pub cutoff_pct: f64,
    /// Rating slots 1-3 (A-C): in the base case, and after an outage.
    pub base_rating: u8,
    pub rating: u8,
    pub limit_pct: f64,
}

/// A lists file read against a case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lists {
    pub named: Vec<Named>,
    pub sets: Vec<Set>,
    pub monitors: Vec<Monitor>,
    pub transfers: Vec<Transfer>,
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
    for r in doc.records("monitor") {
        if let Some(m) = monitor(&doc, r, &subsystems, &mut out.notes) {
            out.monitors.push(m);
        }
    }
    for r in doc.records("transfer") {
        if let Some(t) = transfer(&doc, r, &subsystems, &mut out.notes) {
            out.transfers.push(t);
        }
    }
    Ok(out)
}

/// A `transfer` record, or `None` (with a note) when it can't be applied.
fn transfer(
    doc: &Document,
    r: &Record,
    subsystems: &Subsystems,
    notes: &mut Vec<String>,
) -> Option<Transfer> {
    let mut skip = |why: String| {
        notes.push(format!("Transfer \"{}\": {why}; skipped.", r.id));
        None
    };
    let side = |field: &str| -> Result<HashSet<u32>, String> {
        let name = r.str(field).unwrap_or("");
        subsystems
            .buses(name)
            .ok_or_else(|| format!("subsystem {name} isn't in the file"))
    };
    let (source, sink) = match (side("source"), side("sink")) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return skip(e),
    };
    let float = |field: &str, default: f64| {
        doc.value("transfer", r, field)
            .and_then(Value::as_f64)
            .unwrap_or(default)
    };
    let slot = |field: &str| match doc
        .value("transfer", r, field)
        .and_then(Value::as_i64)
        .unwrap_or(1)
    {
        n @ 1..=3 => Ok(n as u8),
        n => Err(format!("{field} slot {n} isn't read yet (only 1-3, A-C)")),
    };
    let (base_rating, rating) = match (slot("base_rating"), slot("rating")) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return skip(e),
    };
    Some(Transfer {
        name: r.id.clone(),
        source,
        sink,
        scaling: doc
            .value("transfer", r, "scaling")
            .and_then(Value::as_str)
            .unwrap_or("generation")
            .to_string(),
        cutoff_pct: float("cutoff_pct", 5.0),
        base_rating,
        rating,
        limit_pct: float("limit_pct", 100.0),
    })
}

/// A `monitor` record, or `None` (with a note) when it can't be applied.
fn monitor(
    doc: &Document,
    r: &Record,
    subsystems: &Subsystems,
    notes: &mut Vec<String>,
) -> Option<Monitor> {
    let mut skip = |why: String| {
        notes.push(format!("Monitor \"{}\": {why}; skipped.", r.id));
        None
    };
    let buses = match r.str("subsystem") {
        Some(name) => match subsystems.buses(name) {
            Some(b) => Some(b),
            None => return skip(format!("subsystem {name} isn't in the file")),
        },
        None => None,
    };
    let element = r.str("element").map(str::to_string);
    let kind = doc
        .value("monitor", r, "kind")
        .and_then(Value::as_str)
        .unwrap_or("flow");
    let watched = match kind {
        "flow" => {
            let rating = match r.int("rating") {
                None => None,
                Some(n @ 1..=3) => Some(n as u8),
                Some(n) => return skip(format!("rating slot {n} isn't read yet (only 1-3, A-C)")),
            };
            Watched::Flow {
                element,
                buses,
                rating,
            }
        }
        "voltage" => Watched::Voltage {
            element,
            buses,
            vmin: r.float("vmin_pu"),
            vmax: r.float("vmax_pu"),
        },
        "interface" => {
            let Some(name) = r.str("interface") else {
                return skip("no interface named".into());
            };
            let Some(i) = doc.records("interface").iter().find(|i| i.id == name) else {
                return skip(format!("interface {name} isn't in the file"));
            };
            let elements = doc
                .value("interface", i, "elements")
                .and_then(Value::as_list)
                .unwrap_or(&[])
                .iter()
                .filter_map(|e| {
                    let parts = e.as_list()?;
                    let id = parts.first()?.as_str()?.to_string();
                    let w = parts.get(1).and_then(Value::as_f64).unwrap_or(1.0);
                    Some((id, w))
                })
                .collect();
            Watched::Interface(Interface {
                // Its display name, when it has one.
                name: i
                    .str("name")
                    .filter(|n| !n.is_empty())
                    .unwrap_or(name)
                    .to_string(),
                elements,
                limit_mw: i.float("limit_mw"),
            })
        }
        other => return skip(format!("kind {other} isn't known")),
    };
    let limit_pct = doc
        .value("monitor", r, "limit_pct")
        .and_then(Value::as_f64)
        .unwrap_or(100.0);
    Some(Monitor {
        name: r.id.clone(),
        watched,
        limit_pct,
    })
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
