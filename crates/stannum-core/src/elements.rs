//! One name for each element of a case, shared by every module.
//!
//! The ids follow nodal's equipment ids, so topology results need no
//! translation: `bus:101`, `line:101:102:1`, `transformer:101:102:1`,
//! `transformer:101:102:103:1` (three-winding), `generator:101:1`,
//! `load:101:1`. Bus numbers in branch ids are sorted, and the last part is
//! the RAW circuit or machine id. [`KINDS`] lists every kind: its id format,
//! the RAW section and MATPOWER table it comes from, and where the inspector
//! reads it.
//!
//! MATPOWER files have no circuit ids, so parallel branches between the same
//! buses are numbered 1, 2, ... in file order, as are generators at a bus.

use rs_power::{BranchKind, BranchSource, Case, GeneratorSource};
use serde::Serialize;
use std::collections::HashMap;

/// The shared id of a bus.
pub fn bus(number: u32) -> String {
    format!("bus:{number}")
}

/// The shared id of a line or transformer.
pub fn branch(kind: BranchKind, buses: &[u32], circuit: &str) -> String {
    let mut sorted = buses.to_vec();
    sorted.sort_unstable();
    let buses = sorted
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(":");
    let kind = match kind {
        BranchKind::Line => "line",
        BranchKind::Transformer => "transformer",
    };
    format!("{kind}:{buses}:{circuit}")
}

/// The shared id of a generator.
pub fn generator(bus: u32, id: &str) -> String {
    format!("generator:{bus}:{id}")
}

/// The shared id of a two-terminal DC line, such as `dcline:101:205:DC1`.
pub fn dc_line(from: u32, to: u32, name: &str) -> String {
    let (a, b) = if from <= to { (from, to) } else { (to, from) };
    format!("dcline:{a}:{b}:{name}")
}

/// The shared id of a fixed shunt, such as `shunt:101:1`.
pub fn shunt(bus: u32, id: &str) -> String {
    format!("shunt:{bus}:{id}")
}

/// The shared id of a switched shunt, such as `switched-shunt:101:1`.
pub fn switched_shunt(bus: u32, id: &str) -> String {
    format!("switched-shunt:{bus}:{id}")
}

/// The shared id of an induction machine, such as `machine:101:1`.
pub fn machine(bus: u32, id: &str) -> String {
    format!("machine:{bus}:{id}")
}

/// The shared id of a FACTS device, such as `facts:SVC_1`.
pub fn facts(name: &str) -> String {
    format!("facts:{name}")
}

/// Where the element inspector reads an element from.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The power flow model (every file power flow reads).
    Model,
    /// nodal's topology model (RAW revision 35).
    Topology,
    /// The file's own record, field by field (RAW revisions 33 and 35).
    Record,
}

/// One kind of element: how its id is written and where it comes from.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct ElementKind {
    /// The id's first part, such as "line".
    pub kind: &'static str,
    /// What people call it, such as "Line".
    pub label: &'static str,
    /// The id's parts after the kind, such as `<bus>:<bus>:<circuit>`.
    pub format: &'static str,
    /// How many bus numbers follow the kind: 0 when the id is a name, and
    /// for a transformer 2, or 3 for a three-winding one.
    pub buses: u8,
    /// The RAW sections its records are in; empty when no record is one.
    pub raw_sections: &'static [&'static str],
    /// The MATPOWER table its rows are in; null when MATPOWER has none.
    pub matpower_table: Option<&'static str>,
    /// Where the element inspector reads it.
    pub inspector: Source,
    /// Whether a solve report has a row for it.
    pub solved: bool,
    /// Whether a study edit or a contingency can take it out.
    pub outage: bool,
}

/// Every kind of element, in the order the inspector and search list them.
pub const KINDS: &[ElementKind] = &[
    ElementKind {
        kind: "bus",
        label: "Bus",
        format: "<bus>",
        buses: 1,
        raw_sections: &["BUS"],
        matpower_table: Some("bus"),
        inspector: Source::Model,
        solved: true,
        outage: false,
    },
    ElementKind {
        kind: "line",
        label: "Line",
        format: "<bus>:<bus>:<circuit>",
        buses: 2,
        raw_sections: &["BRANCH"],
        matpower_table: Some("branch"),
        inspector: Source::Model,
        solved: true,
        outage: true,
    },
    ElementKind {
        kind: "transformer",
        label: "Transformer",
        format: "<bus>:<bus>[:<bus>]:<circuit>",
        buses: 2,
        raw_sections: &["TRANSFORMER"],
        matpower_table: Some("branch"),
        inspector: Source::Model,
        solved: true,
        outage: true,
    },
    ElementKind {
        kind: "switch",
        label: "Switch",
        format: "<bus>:<bus>:<circuit>",
        buses: 2,
        raw_sections: &["SYSTEM SWITCHING DEVICE"],
        matpower_table: None,
        inspector: Source::Topology,
        solved: false,
        outage: false,
    },
    ElementKind {
        kind: "generator",
        label: "Generator",
        format: "<bus>:<machine>",
        buses: 1,
        raw_sections: &["GENERATOR"],
        matpower_table: Some("gen"),
        inspector: Source::Model,
        solved: true,
        outage: true,
    },
    ElementKind {
        kind: "load",
        label: "Load",
        format: "<bus>:<id>",
        buses: 1,
        raw_sections: &["LOAD"],
        matpower_table: None,
        inspector: Source::Model,
        solved: false,
        outage: false,
    },
    ElementKind {
        kind: "shunt",
        label: "Fixed shunt",
        format: "<bus>:<id>",
        buses: 1,
        raw_sections: &["FIXED SHUNT"],
        matpower_table: None,
        inspector: Source::Record,
        solved: false,
        outage: false,
    },
    ElementKind {
        kind: "switched-shunt",
        label: "Switched shunt",
        format: "<bus>:<id>",
        buses: 1,
        raw_sections: &["SWITCHED SHUNT"],
        matpower_table: None,
        inspector: Source::Record,
        solved: false,
        outage: false,
    },
    ElementKind {
        kind: "machine",
        label: "Induction machine",
        format: "<bus>:<id>",
        buses: 1,
        raw_sections: &["INDUCTION MACHINE"],
        matpower_table: None,
        inspector: Source::Record,
        solved: false,
        outage: false,
    },
    ElementKind {
        kind: "dcline",
        label: "DC line",
        format: "<bus>:<bus>:<name>",
        buses: 2,
        raw_sections: &["TWO-TERMINAL DC", "VSC DC LINE"],
        matpower_table: Some("dcline"),
        inspector: Source::Model,
        solved: true,
        outage: false,
    },
    ElementKind {
        kind: "facts",
        label: "FACTS device",
        format: "<name>",
        buses: 0,
        raw_sections: &["FACTS DEVICE"],
        matpower_table: None,
        inspector: Source::Record,
        solved: false,
        outage: false,
    },
];

/// The kind of an element id, from its first part.
pub fn kind_of(id: &str) -> Option<&'static ElementKind> {
    let kind = id.split(':').next()?;
    KINDS.iter().find(|k| k.kind == kind)
}

/// What one rs-power branch row is, in shared terms.
#[derive(Clone, Debug, Serialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct BranchElement {
    pub element: String,
    pub circuit: String,
    /// 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise.
    pub winding: u8,
}

/// What one rs-power generator row is, in shared terms.
#[derive(Clone, Debug, Serialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct GeneratorElement {
    pub element: String,
    pub machine_id: String,
}

/// Shared ids for every branch and generator row of a case, in row order.
#[derive(Clone, Debug, Default)]
pub struct Elements {
    pub branches: Vec<BranchElement>,
    pub generators: Vec<GeneratorElement>,
    /// One id per row of `case.dcline`.
    pub dc_lines: Vec<String>,
}

impl Elements {
    /// From the RAW records rs-power kept for each row.
    pub fn from_raw(
        case: &Case,
        branches: &[BranchSource],
        generators: &[GeneratorSource],
        dc_names: &[String],
    ) -> Self {
        Self {
            branches: branches
                .iter()
                .map(|s| BranchElement {
                    element: branch(s.kind, &s.buses, &s.circuit),
                    circuit: s.circuit.clone(),
                    winding: s.winding,
                })
                .collect(),
            generators: generators
                .iter()
                .map(|s| GeneratorElement {
                    element: generator(s.bus, &s.id),
                    machine_id: s.id.clone(),
                })
                .collect(),
            dc_lines: dc_rows(case)
                .enumerate()
                .map(|(i, (from, to))| {
                    let name = dc_names.get(i).map_or("1", String::as_str);
                    dc_line(from, to, name)
                })
                .collect(),
        }
    }

    /// For a MATPOWER case, numbering repeats in file order.
    pub fn from_matpower(case: &Case) -> Self {
        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut next = |base: String| {
            let n = seen.entry(base.clone()).or_default();
            *n += 1;
            (base, n.to_string())
        };
        let branches = case
            .branches
            .iter()
            .map(|br| {
                let transformer = (br.tap != 0.0 && br.tap != 1.0) || br.shift != 0.0;
                let kind = if transformer {
                    BranchKind::Transformer
                } else {
                    BranchKind::Line
                };
                let (_, circuit) = next(branch(kind, &[br.from, br.to], ""));
                BranchElement {
                    element: branch(kind, &[br.from, br.to], &circuit),
                    circuit,
                    winding: 0,
                }
            })
            .collect();
        let generators = case
            .generators
            .iter()
            .map(|g| {
                let (_, id) = next(generator(g.bus, ""));
                GeneratorElement {
                    element: generator(g.bus, &id),
                    machine_id: id,
                }
            })
            .collect();
        let dc_lines = dc_rows(case)
            .map(|(from, to)| {
                let (_, n) = next(dc_line(from, to, ""));
                dc_line(from, to, &n)
            })
            .collect();
        Self {
            branches,
            generators,
            dc_lines,
        }
    }
}

/// Rectifier and inverter bus of each DC line row.
fn dc_rows(case: &Case) -> impl Iterator<Item = (u32, u32)> + '_ {
    case.dcline.iter().flatten().map(|row| {
        (
            row.first().copied().unwrap_or(0.0) as u32,
            row.get(1).copied().unwrap_or(0.0) as u32,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_maker_has_a_kind() {
        for id in [
            bus(1),
            branch(BranchKind::Line, &[2, 1], "1"),
            branch(BranchKind::Transformer, &[1, 2, 3], "1"),
            generator(1, "1"),
            dc_line(1, 2, "DC1"),
            shunt(1, "1"),
            switched_shunt(1, "1"),
            machine(1, "1"),
            facts("SVC 1"),
        ] {
            assert!(kind_of(&id).is_some(), "{id}");
        }
        assert!(kind_of("nothing:1").is_none());
        let mut seen = std::collections::HashSet::new();
        assert!(
            KINDS.iter().all(|k| seen.insert(k.kind)),
            "kinds are unique"
        );
    }

    #[test]
    fn branch_ids_sort_buses_like_nodal() {
        assert_eq!(branch(BranchKind::Line, &[102, 101], "2"), "line:101:102:2");
        assert_eq!(
            branch(BranchKind::Transformer, &[30, 10, 20], "T1"),
            "transformer:10:20:30:T1"
        );
    }
}
