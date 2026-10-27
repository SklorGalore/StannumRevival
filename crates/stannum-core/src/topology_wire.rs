//! The topology snapshot in the compact form the app receives.
//!
//! A snapshot of a large case is mostly repetition: every record carries its
//! source fields, and every graph node and connection repeats ids and labels
//! the app can rebuild from the bus and equipment tables. This form sends
//! each table as columns, refers to buses, equipment and graph nodes by
//! position, and leaves out the source fields, which `topology_source` sends
//! for one element when it is inspected. `src/lib/analysis/topologyWire.ts`
//! turns it back into the full snapshot.
//!
//! What the app rebuilds instead of receiving:
//! - a bus node's id (`bus:N`), label (`N · name`) and base kV, from its bus;
//! - a junction node's id (`junction:<equipment>`) and label
//!   (`Transformer <circuit>`), from its equipment;
//! - a connection's id, from its equipment and winding;
//! - each island's buses, from the degree table;
//! - a bridge, from its position in the connection table.
//!
//! Names and circuits are sent trimmed: RAW pads names to their field width,
//! which was a fifth of a large case's snapshot, and every view trims them.

use nodal::model::{Case, Equipment, EquipmentKind, Injection, SourceRecord};
use nodal::topology::{Analysis, GraphData};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize)]
pub struct TopologyWire<'a> {
    pub case: CaseWire<'a>,
    pub analysis: AnalysisWire<'a>,
    pub graph: GraphWire<'a>,
}

/// The topology after a change of options: the case stays as it was.
#[derive(Serialize)]
pub struct TopologyViewWire<'a> {
    pub analysis: AnalysisWire<'a>,
    pub graph: GraphWire<'a>,
}

#[derive(Serialize)]
pub struct CaseWire<'a> {
    pub revision: i32,
    pub base_mva: f64,
    pub frequency_hz: f64,
    pub zero_impedance_threshold: f64,
    pub titles: &'a [String],
    pub encoding: &'a str,
    pub diagnostics: &'a [nodal::model::Diagnostic],
    pub skipped_sections: &'a BTreeMap<String, usize>,
    /// Section names, which the tables refer to by position.
    pub sections: Vec<&'a str>,
    pub buses: BusColumns<'a>,
    pub equipment: EquipmentColumns<'a>,
    pub loads: InjectionColumns<'a>,
    pub generators: InjectionColumns<'a>,
}

#[derive(Serialize, Default)]
pub struct BusColumns<'a> {
    pub number: Vec<i32>,
    pub name: Vec<&'a str>,
    pub base_kv: Vec<f64>,
    pub bus_type: Vec<i32>,
    pub area: Vec<Option<i32>>,
    pub zone: Vec<Option<i32>>,
    pub voltage_pu: Vec<Option<f64>>,
    pub line: Vec<usize>,
    pub section: Vec<u32>,
}

#[derive(Serialize, Default)]
pub struct EquipmentColumns<'a> {
    pub id: Vec<&'a str>,
    /// 0 line, 1 transformer, 2 switch.
    pub kind: Vec<u8>,
    pub name: Vec<&'a str>,
    pub circuit: Vec<&'a str>,
    pub buses: Vec<&'a [i32]>,
    pub status: Vec<i32>,
    /// Bit `i` set when terminal `i` is open: 0 for most equipment.
    pub open_terminals: Vec<u32>,
    pub line: Vec<usize>,
    pub section: Vec<u32>,
}

#[derive(Serialize, Default)]
pub struct InjectionColumns<'a> {
    pub id: Vec<&'a str>,
    pub bus: Vec<i32>,
    pub status: Vec<i32>,
    pub p_mw: Vec<f64>,
    pub q_mvar: Vec<f64>,
    pub line: Vec<usize>,
    pub section: Vec<u32>,
}

#[derive(Serialize)]
pub struct GraphWire<'a> {
    pub nodes: NodeColumns,
    pub edges: EdgeColumns,
    pub zero_impedance_groups: &'a [Vec<i32>],
    pub merge_zero_impedance: bool,
    pub absorbed_equipment_ids: &'a [String],
}

#[derive(Serialize, Default)]
pub struct NodeColumns {
    /// The bus a bus node stands for, or null for a junction.
    pub bus: Vec<Option<i32>>,
    /// A junction's equipment, by position in the equipment table.
    pub equipment: Vec<Option<u32>>,
    /// Bus nodes whose member buses are not just their own bus, as
    /// `[node, members]`. Junctions have no members.
    pub members: Vec<(u32, Vec<i32>)>,
}

#[derive(Serialize, Default)]
pub struct EdgeColumns {
    /// By position in the equipment table.
    pub equipment: Vec<u32>,
    /// By position in the node table.
    pub source: Vec<u32>,
    pub target: Vec<u32>,
    pub winding: Vec<Option<usize>>,
}

#[derive(Serialize)]
pub struct AnalysisWire<'a> {
    pub active_buses: usize,
    pub original_active_buses: usize,
    pub absorbed_equipment: usize,
    pub excluded_buses: usize,
    pub active_equipment: usize,
    pub excluded_equipment: usize,
    pub synthetic_junctions: usize,
    /// Each island's id and junctions; its buses come from `degrees`.
    pub islands: Vec<IslandWire<'a>>,
    pub degrees: DegreeColumns,
    /// By position in the connection table.
    pub bridges: Vec<u32>,
    pub articulation_buses: &'a [i32],
    pub synthetic_articulations: &'a [String],
}

#[derive(Serialize)]
pub struct IslandWire<'a> {
    pub id: usize,
    pub synthetic_nodes: &'a [String],
}

#[derive(Serialize, Default)]
pub struct DegreeColumns {
    pub bus: Vec<i32>,
    pub degree: Vec<usize>,
    pub island: Vec<usize>,
}

/// Section names, numbered in the order first seen.
#[derive(Default)]
struct Sections<'a> {
    names: Vec<&'a str>,
    index: HashMap<&'a str, u32>,
}

impl<'a> Sections<'a> {
    fn of(&mut self, source: &'a SourceRecord) -> u32 {
        let name = source.section.as_str();
        *self.index.entry(name).or_insert_with(|| {
            self.names.push(name);
            (self.names.len() - 1) as u32
        })
    }
}

fn injections<'a>(rows: &'a [Injection], sections: &mut Sections<'a>) -> InjectionColumns<'a> {
    let mut c = InjectionColumns::default();
    for r in rows {
        c.id.push(&r.id);
        c.bus.push(r.bus);
        c.status.push(r.status);
        c.p_mw.push(r.p_mw);
        c.q_mvar.push(r.q_mvar);
        c.line.push(r.source.line);
        c.section.push(sections.of(&r.source));
    }
    c
}

fn kind(e: &Equipment) -> u8 {
    match e.kind {
        EquipmentKind::Line => 0,
        EquipmentKind::Transformer => 1,
        EquipmentKind::Switch => 2,
    }
}

pub fn case_wire(case: &Case) -> CaseWire<'_> {
    let mut sections = Sections::default();
    let mut buses = BusColumns::default();
    for b in &case.buses {
        buses.number.push(b.number);
        buses.name.push(b.name.trim());
        buses.base_kv.push(b.base_kv);
        buses.bus_type.push(b.bus_type);
        buses.area.push(b.area);
        buses.zone.push(b.zone);
        buses.voltage_pu.push(b.voltage_pu);
        buses.line.push(b.source.line);
        buses.section.push(sections.of(&b.source));
    }
    let mut equipment = EquipmentColumns::default();
    for e in &case.equipment {
        equipment.id.push(&e.id);
        equipment.kind.push(kind(e));
        equipment.name.push(e.name.trim());
        equipment.circuit.push(e.circuit.trim());
        equipment.buses.push(&e.buses);
        equipment.status.push(e.status);
        equipment.open_terminals.push(
            e.active_terminals
                .iter()
                .enumerate()
                .filter(|(_, on)| !**on)
                .fold(0, |bits, (i, _)| bits | 1 << i),
        );
        equipment.line.push(e.source.line);
        equipment.section.push(sections.of(&e.source));
    }
    let loads = injections(&case.loads, &mut sections);
    let generators = injections(&case.generators, &mut sections);
    CaseWire {
        revision: case.revision,
        base_mva: case.base_mva,
        frequency_hz: case.frequency_hz,
        zero_impedance_threshold: case.zero_impedance_threshold,
        titles: &case.titles,
        encoding: &case.encoding,
        diagnostics: &case.diagnostics,
        skipped_sections: &case.skipped_sections,
        sections: sections.names,
        buses,
        equipment,
        loads,
        generators,
    }
}

/// The analysis and graph, with ids replaced by positions. `case` is the
/// case the graph was built from.
pub fn view_wire<'a>(
    case: &Case,
    analysis: &'a Analysis,
    graph: &'a GraphData,
) -> Result<TopologyViewWire<'a>, String> {
    let equipment: HashMap<&str, u32> = case
        .equipment
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i as u32))
        .collect();
    let eq = |id: &str| -> Result<u32, String> {
        equipment
            .get(id)
            .copied()
            .ok_or_else(|| format!("The topology graph names {id}, which isn't in the case."))
    };
    let missing = |what: &str| format!("The topology graph names {what}, which it doesn't hold.");
    let mut nodes = NodeColumns::default();
    let mut node_index: HashMap<&str, u32> = HashMap::with_capacity(graph.nodes.len());
    for (i, n) in graph.nodes.iter().enumerate() {
        node_index.insert(&n.id, i as u32);
        nodes.bus.push(n.bus);
        nodes
            .equipment
            .push(n.equipment_id.as_deref().map(eq).transpose()?);
        let plain = match n.bus {
            Some(b) => n.member_buses.len() == 1 && n.member_buses[0] == b,
            None => n.member_buses.is_empty(),
        };
        if !plain {
            nodes.members.push((i as u32, n.member_buses.clone()));
        }
    }
    let mut edges = EdgeColumns::default();
    let mut edge_index: HashMap<&str, u32> = HashMap::with_capacity(graph.edges.len());
    for (i, e) in graph.edges.iter().enumerate() {
        edge_index.insert(&e.id, i as u32);
        edges.equipment.push(eq(&e.equipment_id)?);
        let node = |id: &str| node_index.get(id).copied().ok_or_else(|| missing(id));
        edges.source.push(node(&e.source)?);
        edges.target.push(node(&e.target)?);
        edges.winding.push(e.winding);
    }
    let mut degrees = DegreeColumns::default();
    for d in &analysis.degrees {
        degrees.bus.push(d.bus);
        degrees.degree.push(d.degree);
        degrees.island.push(d.island);
    }
    let bridges = analysis
        .bridges
        .iter()
        .map(|b| {
            edge_index
                .get(b.id.as_str())
                .copied()
                .ok_or_else(|| missing(&b.id))
        })
        .collect::<Result<_, _>>()?;
    Ok(TopologyViewWire {
        analysis: AnalysisWire {
            active_buses: analysis.active_buses,
            original_active_buses: analysis.original_active_buses,
            absorbed_equipment: analysis.absorbed_equipment,
            excluded_buses: analysis.excluded_buses,
            active_equipment: analysis.active_equipment,
            excluded_equipment: analysis.excluded_equipment,
            synthetic_junctions: analysis.synthetic_junctions,
            islands: analysis
                .islands
                .iter()
                .map(|i| IslandWire {
                    id: i.id,
                    synthetic_nodes: &i.synthetic_nodes,
                })
                .collect(),
            degrees,
            bridges,
            articulation_buses: &analysis.articulation_buses,
            synthetic_articulations: &analysis.synthetic_articulations,
        },
        graph: GraphWire {
            nodes,
            edges,
            zero_impedance_groups: &graph.zero_impedance_groups,
            merge_zero_impedance: graph.merge_zero_impedance,
            absorbed_equipment_ids: &graph.absorbed_equipment_ids,
        },
    })
}

/// The record an element was read from: a bus (`bus:N`), equipment, load
/// or generator, by its id.
pub fn source<'a>(case: &'a Case, element: &str) -> Option<&'a SourceRecord> {
    if let Some(n) = element.strip_prefix("bus:") {
        let n: i32 = n.parse().ok()?;
        return case.buses.iter().find(|b| b.number == n).map(|b| &b.source);
    }
    case.equipment
        .iter()
        .find(|e| e.id == element)
        .map(|e| &e.source)
        .or_else(|| {
            case.loads
                .iter()
                .chain(&case.generators)
                .find(|i| i.id == element)
                .map(|i| &i.source)
        })
}
