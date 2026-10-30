//! NERC TPL-001 planning events, generated from the case.
//!
//! The categories that need nothing beyond a bus-branch case:
//!
//! - P0: the base case, checked against the limits as it stands.
//! - P1.1 generator, P1.2 transmission circuit, P1.3 transformer, P1.4
//!   shunt device, P1.5 one pole of a DC line.
//! - P2.1: one section of a multi-section line opened.
//! - P7.2: a bipolar DC line.
//!
//! A multi-section line goes out whole in P1.2, as its breakers would take
//! it out. RAW gives a two-terminal DC line one record for both poles, so
//! P1.5 halves its schedule, and only line-commutated lines get one. The
//! categories that need breakers (P2.2 to P2.4, P4, P5), two stages (P3,
//! P6) or tower data (P7.1) are not generated yet.

use crate::elements;
use crate::power::PowerCase;
use rs_power::{BranchKind, ShuntKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Every category generated, with what it takes out.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("P0", "Base case"),
    ("P1.1", "Generator"),
    ("P1.2", "Transmission circuit"),
    ("P1.3", "Transformer"),
    ("P1.4", "Shunt device"),
    ("P1.5", "One pole of a DC line"),
    ("P2.1", "Line section opened"),
    ("P7.2", "Bipolar DC line"),
];

fn all_categories() -> Vec<String> {
    CATEGORIES.iter().map(|(c, _)| c.to_string()).collect()
}

fn default_kv_min() -> f64 {
    100.0
}

/// Which planning events to generate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct PlanningSettings {
    /// Categories or groups of them, such as "P1" (every P1 event) or
    /// "P1.4". Every category Stannum generates when absent.
    #[serde(default = "all_categories")]
    pub categories: Vec<String>,
    /// Only branches, shunts and DC lines with a terminal bus at or above
    /// this voltage, kV. A bus with no base voltage counts as above.
    /// Generators are not filtered by voltage: most connect below it
    /// through a step-up transformer.
    #[serde(default = "default_kv_min")]
    pub kv_min: f64,
    /// Only elements with a terminal bus in one of these areas; every area
    /// when empty.
    #[serde(default)]
    pub areas: Vec<u32>,
}

impl Default for PlanningSettings {
    fn default() -> Self {
        Self {
            categories: all_categories(),
            kv_min: default_kv_min(),
            areas: Vec::new(),
        }
    }
}

/// One generated event: what goes out, as rows of the case.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanningEvent {
    pub category: &'static str,
    /// The element taken out; empty for P0.
    pub element: String,
    /// "base", "generator", "branch", "shunt" or "dc_line".
    pub kind: &'static str,
    /// What the event is, in words, when the element alone doesn't say.
    pub description: Option<String>,
    pub branch_rows: Vec<usize>,
    pub generator_rows: Vec<usize>,
    /// Rows of [`PowerCase::shunts`].
    pub shunt_rows: Vec<usize>,
    /// `case.dcline` rows, each with the share of its schedule that stays:
    /// 0 out of service, 0.5 for one pole of two.
    pub dc_rows: Vec<(usize, f64)>,
}

impl PlanningEvent {
    fn new(category: &'static str, element: String, kind: &'static str) -> Self {
        Self {
            category,
            element,
            kind,
            description: None,
            branch_rows: vec![],
            generator_rows: vec![],
            shunt_rows: vec![],
            dc_rows: vec![],
        }
    }
}

/// How many events a category gives, for showing before a run.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct CategoryCount {
    pub category: String,
    /// What it takes out, such as "Transformer".
    pub label: String,
    pub count: usize,
}

/// The categories asked for, as exact category names, in table order.
fn chosen(categories: &[String]) -> Result<Vec<&'static str>, String> {
    let mut out: Vec<&'static str> = Vec::new();
    for asked in categories {
        let asked = asked.trim().to_ascii_uppercase();
        let matches: Vec<&'static str> = CATEGORIES
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| {
                *c == asked
                    || c.strip_prefix(asked.as_str())
                        .is_some_and(|r| r.starts_with('.'))
            })
            .collect();
        if matches.is_empty() {
            return Err(format!(
                "Stannum doesn't generate {asked} events yet. It generates P0, P1.1 to P1.5, P2.1 and P7.2."
            ));
        }
        for m in matches {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    out.sort_by_key(|c| CATEGORIES.iter().position(|(k, _)| k == c));
    Ok(out)
}

/// The events `settings` asks for, in category order, then case order.
/// Elements already out of service give no event.
pub fn events(
    power: &PowerCase,
    settings: &PlanningSettings,
) -> Result<Vec<PlanningEvent>, String> {
    let wanted = chosen(&settings.categories)?;
    let case = &power.case;
    let bus: HashMap<u32, &rs_power::Bus> = case.buses.iter().map(|b| (b.id, b)).collect();
    let areas: HashSet<u32> = settings.areas.iter().copied().collect();
    let in_area =
        |id: u32| areas.is_empty() || bus.get(&id).is_some_and(|b| areas.contains(&b.area));
    let high_kv = |id: u32| {
        bus.get(&id)
            .is_none_or(|b| b.base_kv <= 0.0 || b.base_kv >= settings.kv_min)
    };
    // Any terminal in the areas, and any at or above the voltage.
    let keep = |ends: &[u32]| ends.iter().any(|&b| in_area(b)) && ends.iter().any(|&b| high_kv(b));

    // Branches by element, with every bus they touch (three-winding
    // transformers have several rows).
    let mut by_element: BTreeMap<usize, (String, Vec<usize>)> = BTreeMap::new();
    let mut first_row: HashMap<&str, usize> = HashMap::new();
    for (row, e) in power.elements.branches.iter().enumerate() {
        let first = *first_row.entry(e.element.as_str()).or_insert(row);
        by_element
            .entry(first)
            .or_insert_with(|| (e.element.clone(), vec![]))
            .1
            .push(row);
    }
    let sections: HashSet<usize> = power
        .multi_section_lines
        .iter()
        .flat_map(|m| m.branches.iter().copied())
        .collect();
    let ends = |rows: &[usize]| -> Vec<u32> {
        rows.iter()
            .flat_map(|&r| [case.branches[r].from, case.branches[r].to])
            .collect()
    };
    let online = |rows: &[usize]| rows.iter().any(|&r| case.branches[r].online);

    let mut out = Vec::new();
    for category in wanted {
        match category {
            "P0" => {
                let mut e = PlanningEvent::new("P0", String::new(), "base");
                e.description = Some("Base case".into());
                out.push(e);
            }
            "P1.1" => {
                for (row, g) in case.generators.iter().enumerate() {
                    if g.online && in_area(g.bus) {
                        let mut e = PlanningEvent::new(
                            "P1.1",
                            power.elements.generators[row].element.clone(),
                            "generator",
                        );
                        e.generator_rows.push(row);
                        out.push(e);
                    }
                }
            }
            "P1.2" => {
                for (element, rows) in by_element.values() {
                    if element.starts_with("line:")
                        && !rows.iter().any(|r| sections.contains(r))
                        && online(rows)
                        && keep(&ends(rows))
                    {
                        let mut e = PlanningEvent::new("P1.2", element.clone(), "branch");
                        e.branch_rows = rows.clone();
                        out.push(e);
                    }
                }
                for m in &power.multi_section_lines {
                    if online(&m.branches) && keep(&m.buses) {
                        let mut e = PlanningEvent::new(
                            "P1.2",
                            elements::branch(BranchKind::Line, &[m.from, m.to], &m.id),
                            "branch",
                        );
                        e.description =
                            Some(format!("Multi-section line, {} sections", m.branches.len()));
                        e.branch_rows = m.branches.clone();
                        out.push(e);
                    }
                }
            }
            "P1.3" => {
                for (element, rows) in by_element.values() {
                    if element.starts_with("transformer:") && online(rows) && keep(&ends(rows)) {
                        let mut e = PlanningEvent::new("P1.3", element.clone(), "branch");
                        e.branch_rows = rows.clone();
                        out.push(e);
                    }
                }
            }
            "P1.4" => {
                let controlled: HashSet<usize> = power
                    .voltage_controls
                    .switched_shunts
                    .iter()
                    .map(|s| s.line)
                    .collect();
                for (row, s) in power.shunts.iter().enumerate() {
                    let switched = s.kind == ShuntKind::Switched;
                    let does_something = s.g != 0.0
                        || s.b != 0.0
                        || (switched && s.line != 0 && controlled.contains(&s.line));
                    if s.online && does_something && keep(&[s.bus]) {
                        let element = if switched {
                            elements::switched_shunt(s.bus, &s.id)
                        } else {
                            elements::shunt(s.bus, &s.id)
                        };
                        let mut e = PlanningEvent::new("P1.4", element, "shunt");
                        e.shunt_rows.push(row);
                        out.push(e);
                    }
                }
            }
            "P1.5" | "P7.2" => {
                let poles = category == "P1.5";
                for (row, line) in case.dcline.iter().flatten().enumerate() {
                    let kind = power
                        .dc_line_kinds
                        .get(row)
                        .copied()
                        .unwrap_or("two_terminal");
                    let fits = if poles {
                        kind == "two_terminal"
                    } else {
                        kind == "two_terminal" || kind == "vsc"
                    };
                    let (from, to) = (line[0] as u32, line[1] as u32);
                    if fits && line[2] != 0.0 && keep(&[from, to]) {
                        let element = power
                            .elements
                            .dc_lines
                            .get(row)
                            .cloned()
                            .unwrap_or_default();
                        let mut e = PlanningEvent::new(category, element, "dc_line");
                        if poles {
                            e.description = Some("One pole: half the scheduled power".into());
                            e.dc_rows.push((row, 0.5));
                        } else {
                            e.description = Some("Both poles".into());
                            e.dc_rows.push((row, 0.0));
                        }
                        out.push(e);
                    }
                }
            }
            "P2.1" => {
                for m in &power.multi_section_lines {
                    if !keep(&m.buses) {
                        continue;
                    }
                    let n = m.branches.len();
                    for (k, &row) in m.branches.iter().enumerate() {
                        if !case.branches[row].online {
                            continue;
                        }
                        let mut e = PlanningEvent::new(
                            "P2.1",
                            power.elements.branches[row].element.clone(),
                            "branch",
                        );
                        e.description = Some(format!(
                            "Section {} of {n} of multi-section line {}-{} {}",
                            k + 1,
                            m.from,
                            m.to,
                            m.id
                        ));
                        e.branch_rows.push(row);
                        out.push(e);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// How many events each category asked for gives.
pub fn counts(
    power: &PowerCase,
    settings: &PlanningSettings,
) -> Result<Vec<CategoryCount>, String> {
    let wanted = chosen(&settings.categories)?;
    let events = events(power, settings)?;
    Ok(wanted
        .into_iter()
        .map(|c| CategoryCount {
            category: c.to_string(),
            label: CATEGORIES
                .iter()
                .find(|(k, _)| *k == c)
                .map_or("", |(_, l)| *l)
                .to_string(),
            count: events.iter().filter(|e| e.category == c).count(),
        })
        .collect())
}
