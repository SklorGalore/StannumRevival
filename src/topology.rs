use crate::model::*;
use rustworkx_core::{
    connectivity,
    petgraph::graph::{NodeIndex, UnGraph},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Island {
    pub id: usize,
    pub buses: Vec<i32>,
    pub synthetic_nodes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Degree {
    pub bus: i32,
    pub degree: usize,
    pub island: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub equipment_id: String,
    pub source: String,
    pub target: String,
    pub winding: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub bus: Option<i32>,
    pub equipment_id: Option<String>,
    pub base_kv: Option<f64>,
    #[serde(default)]
    pub member_buses: Vec<i32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<Connection>,
    pub zero_impedance_groups: Vec<Vec<i32>>,
    pub merge_zero_impedance: bool,
    pub absorbed_equipment_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Analysis {
    pub active_buses: usize,
    pub original_active_buses: usize,
    pub absorbed_equipment: usize,
    pub excluded_buses: usize,
    pub active_equipment: usize,
    pub excluded_equipment: usize,
    pub synthetic_junctions: usize,
    pub islands: Vec<Island>,
    pub degrees: Vec<Degree>,
    pub bridges: Vec<Connection>,
    pub articulation_buses: Vec<i32>,
    pub synthetic_articulations: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TopologyOptions {
    pub merge_zero_impedance: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Path {
    pub buses: Vec<i32>,
    pub equipment_ids: Vec<String>,
    pub node_ids: Vec<String>,
    pub connection_ids: Vec<String>,
    pub hops: usize,
}

pub struct Topology {
    // The simple projection is used only for cut algorithms. Its edge weights
    // retain every original circuit, so degree and bridges remain multigraph-safe.
    graph: UnGraph<GraphNode, Vec<Connection>>,
    bus_nodes: BTreeMap<i32, NodeIndex>,
    bus_hops: BTreeMap<i32, Vec<(i32, usize)>>,
    active_equipment: usize,
    original_active_buses: usize,
    absorbed_equipment: Vec<String>,
    zero_impedance_groups: Vec<Vec<i32>>,
    merge_zero_impedance: bool,
}

fn bus_id(bus: i32) -> String {
    format!("bus:{bus}")
}
fn junction_id(equipment: &str) -> String {
    format!("junction:{equipment}")
}
fn connection_id(equipment: &str, winding: Option<usize>) -> String {
    match winding {
        Some(w) => format!("connection:{equipment}:w{w}"),
        None => format!("connection:{equipment}"),
    }
}

impl Topology {
    pub fn new(case: &Case) -> Self {
        Self::with_options(case, TopologyOptions::default())
    }

    pub fn with_options(case: &Case, options: TopologyOptions) -> Self {
        let groups = zero_impedance_groups(case);
        let mut representatives = case
            .buses
            .iter()
            .filter(|bus| bus.bus_type != 4)
            .map(|bus| (bus.number, bus.number))
            .collect::<BTreeMap<_, _>>();
        if options.merge_zero_impedance {
            for group in &groups {
                if let Some(rep) = group.first() {
                    for member in group {
                        representatives.insert(*member, *rep);
                    }
                }
            }
        }
        let mut members = BTreeMap::<i32, Vec<i32>>::new();
        for (bus, representative) in &representatives {
            members.entry(*representative).or_default().push(*bus);
        }
        let mut topology = Self {
            graph: UnGraph::default(),
            bus_nodes: BTreeMap::new(),
            bus_hops: BTreeMap::new(),
            active_equipment: 0,
            original_active_buses: representatives.len(),
            absorbed_equipment: vec![],
            zero_impedance_groups: groups,
            merge_zero_impedance: options.merge_zero_impedance,
        };
        let bus_data = case
            .buses
            .iter()
            .map(|bus| (bus.number, bus))
            .collect::<BTreeMap<_, _>>();
        for (representative, member_buses) in &members {
            let bus = bus_data[representative];
            let index = topology.graph.add_node(GraphNode {
                id: bus_id(*representative),
                label: format!("{} · {}", representative, bus.name.trim()),
                bus: Some(*representative),
                equipment_id: None,
                base_kv: Some(bus.base_kv),
                member_buses: member_buses.clone(),
            });
            for member in member_buses {
                topology.bus_nodes.insert(*member, index);
            }
            topology.bus_hops.insert(*representative, vec![]);
        }
        let mut equipment = case.equipment.iter().enumerate().collect::<Vec<_>>();
        equipment.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
        for (eq_index, eq) in equipment {
            if eq.status == 0 {
                continue;
            }
            let active = eq
                .buses
                .iter()
                .enumerate()
                .filter(|(i, b)| {
                    eq.active_terminals.get(*i) == Some(&true) && topology.bus_nodes.contains_key(b)
                })
                .map(|(i, b)| (i, representatives[b]))
                .collect::<Vec<_>>();
            let mut effective = active.clone();
            effective.sort_by_key(|(_, bus)| *bus);
            effective.dedup_by_key(|(_, bus)| *bus);
            if effective.len() < 2 {
                if active.len() >= 2 && options.merge_zero_impedance {
                    topology.absorbed_equipment.push(eq.id.clone());
                }
                continue;
            }
            topology.active_equipment += 1;
            for (_, from) in &effective {
                for (_, to) in &effective {
                    if from != to {
                        topology
                            .bus_hops
                            .get_mut(from)
                            .unwrap()
                            .push((*to, eq_index));
                    }
                }
            }
            let preserve_winding_star =
                eq.buses.len() == 3 && (effective.len() >= 3 || !options.merge_zero_impedance);
            if preserve_winding_star {
                let junction = topology.graph.add_node(GraphNode {
                    id: junction_id(&eq.id),
                    label: format!("Transformer {}", eq.circuit),
                    bus: None,
                    equipment_id: Some(eq.id.clone()),
                    base_kv: None,
                    member_buses: vec![],
                });
                for (winding, bus) in effective {
                    topology.connect(topology.bus_nodes[&bus], junction, eq, Some(winding + 1));
                }
            } else {
                topology.connect(
                    topology.bus_nodes[&effective[0].1],
                    topology.bus_nodes[&effective[1].1],
                    eq,
                    if eq.buses.len() == 3 {
                        Some(effective[0].0 + 1)
                    } else {
                        None
                    },
                );
            }
        }
        topology
    }

    fn connect(&mut self, from: NodeIndex, to: NodeIndex, eq: &Equipment, winding: Option<usize>) {
        let c = Connection {
            id: connection_id(&eq.id, winding),
            equipment_id: eq.id.clone(),
            source: self.graph[from].id.clone(),
            target: self.graph[to].id.clone(),
            winding,
        };
        if let Some(edge) = self.graph.find_edge(from, to) {
            self.graph[edge].push(c);
        } else {
            self.graph.add_edge(from, to, vec![c]);
        }
    }

    pub fn graph_data(&self) -> GraphData {
        GraphData {
            nodes: self.graph.node_weights().cloned().collect(),
            edges: self.graph.edge_weights().flatten().cloned().collect(),
            zero_impedance_groups: self.zero_impedance_groups.clone(),
            merge_zero_impedance: self.merge_zero_impedance,
            absorbed_equipment_ids: self.absorbed_equipment.clone(),
        }
    }

    pub fn analyze(&self, case: &Case) -> Analysis {
        let cuts = connectivity::articulation_points(&self.graph, None);
        let mut articulation_buses = vec![];
        let mut synthetic_articulations = vec![];
        for node in cuts {
            if let Some(bus) = self.graph[node].bus {
                articulation_buses.push(bus);
            } else {
                synthetic_articulations.push(self.graph[node].id.clone());
            }
        }
        articulation_buses.sort_unstable();
        synthetic_articulations.sort();
        let mut bridges = vec![];
        for (a, b) in connectivity::bridges(&self.graph) {
            let connections = &self.graph[self.graph.find_edge(a, b).unwrap()];
            if connections.len() == 1 {
                bridges.push(connections[0].clone());
            }
        }
        bridges.sort_by(|a, b| a.id.cmp(&b.id));
        let mut islands = connectivity::connected_components(&self.graph)
            .into_iter()
            .map(|nodes| {
                let mut buses = vec![];
                let mut synthetic_nodes = vec![];
                for index in nodes {
                    if let Some(bus) = self.graph[index].bus {
                        buses.push(bus);
                    } else {
                        synthetic_nodes.push(self.graph[index].id.clone());
                    }
                }
                buses.sort_unstable();
                synthetic_nodes.sort();
                Island {
                    id: 0,
                    buses,
                    synthetic_nodes,
                }
            })
            .collect::<Vec<_>>();
        islands.sort_by(|a, b| {
            b.buses
                .len()
                .cmp(&a.buses.len())
                .then(a.buses.first().cmp(&b.buses.first()))
        });
        let mut bus_islands = BTreeMap::new();
        for (i, island) in islands.iter_mut().enumerate() {
            island.id = i + 1;
            for bus in &island.buses {
                bus_islands.insert(*bus, island.id);
            }
        }
        let degrees = self
            .graph
            .node_indices()
            .filter_map(|node| {
                let bus = self.graph[node].bus?;
                Some(Degree {
                    bus,
                    degree: self.graph.edges(node).map(|edge| edge.weight().len()).sum(),
                    island: bus_islands[&bus],
                })
            })
            .collect();
        Analysis {
            active_buses: self
                .graph
                .node_indices()
                .filter(|index| self.graph[*index].bus.is_some())
                .count(),
            original_active_buses: self.original_active_buses,
            absorbed_equipment: self.absorbed_equipment.len(),
            excluded_buses: case.buses.len() - self.original_active_buses,
            active_equipment: self.active_equipment,
            excluded_equipment: case.equipment.len()
                - self.active_equipment
                - self.absorbed_equipment.len(),
            synthetic_junctions: self.graph.node_count()
                - self
                    .graph
                    .node_indices()
                    .filter(|index| self.graph[*index].bus.is_some())
                    .count(),
            islands,
            degrees,
            bridges,
            articulation_buses,
            synthetic_articulations,
        }
    }

    pub fn shortest_path(
        &self,
        case: &Case,
        from: i32,
        to: i32,
    ) -> Result<Option<Path>, Diagnostic> {
        for bus in [from, to] {
            if !self.bus_nodes.contains_key(&bus) {
                return Err(Diagnostic {
                    severity: "error".into(),
                    section: "PATH".into(),
                    line: 0,
                    message: format!("Bus {bus} does not exist or is out of service"),
                });
            }
        }
        let from = self.graph[self.bus_nodes[&from]].bus.unwrap();
        let to = self.graph[self.bus_nodes[&to]].bus.unwrap();
        if from == to {
            return Ok(Some(Path {
                buses: vec![from],
                equipment_ids: vec![],
                node_ids: vec![bus_id(from)],
                connection_ids: vec![],
                hops: 0,
            }));
        }
        // BFS on physical bus adjacency treats every equipment traversal as one
        // hop. The star representation is reconstructed only for visualization.
        let mut parents = BTreeMap::new();
        let mut visited = BTreeSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(bus) = queue.pop_front() {
            if bus == to {
                break;
            }
            for (next, equipment) in &self.bus_hops[&bus] {
                if visited.insert(*next) {
                    parents.insert(*next, (bus, *equipment));
                    queue.push_back(*next);
                }
            }
        }
        if !visited.contains(&to) {
            return Ok(None);
        }
        let mut buses = vec![to];
        let mut equipment = vec![];
        let mut current = to;
        while current != from {
            let (parent, eq) = parents[&current];
            equipment.push(eq);
            buses.push(parent);
            current = parent;
        }
        buses.reverse();
        equipment.reverse();
        let mut node_ids = vec![bus_id(from)];
        let mut connection_ids = vec![];
        let mut equipment_ids = vec![];
        for (i, index) in equipment.iter().enumerate() {
            let eq = &case.equipment[*index];
            equipment_ids.push(eq.id.clone());
            let terminals = eq
                .buses
                .iter()
                .enumerate()
                .filter_map(|(winding, bus)| {
                    if eq.active_terminals.get(winding) != Some(&true) {
                        return None;
                    }
                    let node = self.bus_nodes.get(bus)?;
                    Some((self.graph[*node].bus?, winding))
                })
                .collect::<Vec<_>>();
            let mut effective = terminals.iter().map(|(bus, _)| *bus).collect::<Vec<_>>();
            effective.sort_unstable();
            effective.dedup();
            let preserve_winding_star =
                eq.buses.len() == 3 && (effective.len() >= 3 || !self.merge_zero_impedance);
            if preserve_winding_star {
                node_ids.push(junction_id(&eq.id));
                for bus in [buses[i], buses[i + 1]] {
                    let winding = terminals
                        .iter()
                        .find(|(terminal, _)| *terminal == bus)
                        .unwrap()
                        .1;
                    connection_ids.push(connection_id(&eq.id, Some(winding + 1)));
                }
            } else {
                let edge = self
                    .graph
                    .find_edge(self.bus_nodes[&buses[i]], self.bus_nodes[&buses[i + 1]])
                    .unwrap();
                let connection = self.graph[edge]
                    .iter()
                    .find(|connection| connection.equipment_id == eq.id)
                    .unwrap();
                connection_ids.push(connection.id.clone());
            }
            node_ids.push(bus_id(buses[i + 1]));
        }
        Ok(Some(Path {
            hops: equipment_ids.len(),
            buses,
            equipment_ids,
            node_ids,
            connection_ids,
        }))
    }
}

fn zero_impedance_groups(case: &Case) -> Vec<Vec<i32>> {
    if case.zero_impedance_threshold <= 0.0 {
        return vec![];
    }

    let active_buses = case
        .buses
        .iter()
        .filter(|bus| bus.bus_type != 4)
        .map(|bus| bus.number)
        .collect::<BTreeSet<_>>();
    let mut adjacency = active_buses
        .iter()
        .map(|bus| (*bus, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for eq in &case.equipment {
        if eq.kind != EquipmentKind::Line || eq.status == 0 || eq.buses.len() != 2 {
            continue;
        }
        let resistance = match eq.source.fields.get("R") {
            None => Some(0.0),
            Some(value) if value.trim().is_empty() => Some(0.0),
            Some(value) => value.trim().parse::<f64>().ok(),
        };
        let reactance = eq
            .source
            .fields
            .get("X")
            .and_then(|value| value.trim().parse::<f64>().ok());
        let (Some(resistance), Some(reactance)) = (resistance, reactance) else {
            continue;
        };
        if !resistance.is_finite()
            || resistance != 0.0
            || !reactance.is_finite()
            || reactance.abs() > case.zero_impedance_threshold
        {
            continue;
        }
        let [from, to] = [eq.buses[0], eq.buses[1]];
        if eq.active_terminals.get(0) != Some(&true)
            || eq.active_terminals.get(1) != Some(&true)
            || !active_buses.contains(&from)
            || !active_buses.contains(&to)
        {
            continue;
        }
        adjacency.get_mut(&from).unwrap().insert(to);
        adjacency.get_mut(&to).unwrap().insert(from);
    }

    let mut visited = BTreeSet::new();
    let mut groups = vec![];
    for start in active_buses {
        if visited.contains(&start) {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut group = vec![];
        visited.insert(start);
        while let Some(bus) = queue.pop_front() {
            group.push(bus);
            for neighbor in &adjacency[&bus] {
                if visited.insert(*neighbor) {
                    queue.push_back(*neighbor);
                }
            }
        }
        if group.len() > 1 {
            group.sort_unstable();
            groups.push(group);
        }
    }
    groups.sort();
    groups
}

pub fn analyze(case: &Case) -> Analysis {
    Topology::new(case).analyze(case)
}
pub fn shortest_path(case: &Case, from: i32, to: i32) -> Result<Option<Path>, Diagnostic> {
    Topology::new(case).shortest_path(case, from, to)
}
