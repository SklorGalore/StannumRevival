//! One name for each bus, branch and generator, shared by every module.
//!
//! The ids follow nodal's equipment ids, so topology results need no
//! translation: `bus:101`, `line:101:102:1`, `transformer:101:102:1`,
//! `transformer:101:102:103:1` (three-winding), `generator:101:1`,
//! `load:101:1`. Bus numbers in branch ids are sorted, and the last part is
//! the RAW circuit or machine id.
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

/// What one rs-power branch row is, in shared terms.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BranchElement {
    pub element: String,
    pub circuit: String,
    /// 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise.
    pub winding: u8,
}

/// What one rs-power generator row is, in shared terms.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct GeneratorElement {
    pub element: String,
    pub machine_id: String,
}

/// Shared ids for every branch and generator row of a case, in row order.
#[derive(Clone, Debug, Default)]
pub struct Elements {
    pub branches: Vec<BranchElement>,
    pub generators: Vec<GeneratorElement>,
}

impl Elements {
    /// From the RAW records rs-power kept for each row.
    pub fn from_raw(branches: &[BranchSource], generators: &[GeneratorSource]) -> Self {
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
        Self {
            branches,
            generators,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_ids_sort_buses_like_nodal() {
        assert_eq!(branch(BranchKind::Line, &[102, 101], "2"), "line:101:102:2");
        assert_eq!(
            branch(BranchKind::Transformer, &[30, 10, 20], "T1"),
            "transformer:10:20:30:T1"
        );
    }
}
