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
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<Connection>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Analysis {
    pub active_buses: usize,
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
        let mut topology = Self {
            graph: UnGraph::default(),
            bus_nodes: BTreeMap::new(),
            bus_hops: BTreeMap::new(),
            active_equipment: 0,
        };
        let mut buses = case
            .buses
            .iter()
            .filter(|b| b.bus_type != 4)
            .collect::<Vec<_>>();
        buses.sort_by_key(|b| b.number);
        for bus in buses {
            let index = topology.graph.add_node(GraphNode {
                id: bus_id(bus.number),
                label: format!("{} · {}", bus.number, bus.name.trim()),
                bus: Some(bus.number),
                equipment_id: None,
                base_kv: Some(bus.base_kv),
            });
            topology.bus_nodes.insert(bus.number, index);
            topology.bus_hops.insert(bus.number, vec![]);
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
                .map(|(i, b)| (i, *b))
                .collect::<Vec<_>>();
            if active.len() < 2 {
                continue;
            }
            topology.active_equipment += 1;
            for (_, from) in &active {
                for (_, to) in &active {
                    if from != to {
                        topology
                            .bus_hops
                            .get_mut(from)
                            .unwrap()
                            .push((*to, eq_index));
                    }
                }
            }
            if eq.buses.len() == 3 {
                let junction = topology.graph.add_node(GraphNode {
                    id: junction_id(&eq.id),
                    label: format!("Transformer {}", eq.circuit),
                    bus: None,
                    equipment_id: Some(eq.id.clone()),
                    base_kv: None,
                });
                for (winding, bus) in active {
                    topology.connect(topology.bus_nodes[&bus], junction, eq, Some(winding + 1));
                }
            } else {
                topology.connect(
                    topology.bus_nodes[&active[0].1],
                    topology.bus_nodes[&active[1].1],
                    eq,
                    None,
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
            .bus_nodes
            .iter()
            .map(|(bus, node)| Degree {
                bus: *bus,
                degree: self
                    .graph
                    .edges(*node)
                    .map(|edge| edge.weight().len())
                    .sum(),
                island: bus_islands[bus],
            })
            .collect();
        Analysis {
            active_buses: self.bus_nodes.len(),
            excluded_buses: case.buses.len() - self.bus_nodes.len(),
            active_equipment: self.active_equipment,
            excluded_equipment: case.equipment.len() - self.active_equipment,
            synthetic_junctions: self.graph.node_count() - self.bus_nodes.len(),
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
            if eq.buses.len() == 3 {
                node_ids.push(junction_id(&eq.id));
                for bus in [buses[i], buses[i + 1]] {
                    connection_ids.push(connection_id(
                        &eq.id,
                        Some(eq.buses.iter().position(|b| *b == bus).unwrap() + 1),
                    ));
                }
            } else {
                connection_ids.push(connection_id(&eq.id, None));
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

pub fn analyze(case: &Case) -> Analysis {
    Topology::new(case).analyze(case)
}
pub fn shortest_path(case: &Case, from: i32, to: i32) -> Result<Option<Path>, Diagnostic> {
    Topology::new(case).shortest_path(case, from, to)
}
