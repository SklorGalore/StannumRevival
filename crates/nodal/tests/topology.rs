use nodal::model::*;
use nodal::{analyze, parse_raw, shortest_path};
use std::collections::BTreeMap;

fn source() -> SourceRecord {
    SourceRecord {
        line: 1,
        section: "TEST".into(),
        fields: BTreeMap::new(),
        raw_lines: vec![],
    }
}
fn case(n: i32, pairs: &[(i32, i32)]) -> Case {
    Case {
        revision: 35,
        base_mva: 100.0,
        frequency_hz: 60.0,
        zero_impedance_threshold: 0.0001,
        titles: vec![],
        encoding: "UTF-8".into(),
        buses: (1..=n)
            .map(|number| Bus {
                number,
                name: format!("Bus {number}"),
                base_kv: 138.0,
                bus_type: 1,
                area: None,
                zone: None,
                voltage_pu: None,
                source: source(),
            })
            .collect(),
        equipment: pairs
            .iter()
            .enumerate()
            .map(|(i, (a, b))| Equipment {
                id: format!("line:{i}"),
                kind: EquipmentKind::Line,
                name: String::new(),
                circuit: i.to_string(),
                buses: vec![*a, *b],
                status: 1,
                active_terminals: vec![true, true],
                source: source(),
            })
            .collect(),
        loads: vec![],
        generators: vec![],
        diagnostics: vec![],
        skipped_sections: BTreeMap::new(),
    }
}

#[test]
fn chain_and_isolated_bus_have_expected_cuts_degrees_and_paths() {
    let c = case(4, &[(1, 2), (2, 3)]);
    let a = analyze(&c);
    assert_eq!(
        a.islands
            .iter()
            .map(|i| i.buses.clone())
            .collect::<Vec<_>>(),
        [vec![1, 2, 3], vec![4]]
    );
    assert_eq!(a.articulation_buses, [2]);
    assert_eq!(
        a.bridges
            .iter()
            .map(|e| e.equipment_id.as_str())
            .collect::<Vec<_>>(),
        ["line:0", "line:1"]
    );
    assert_eq!(
        a.degrees.iter().map(|d| d.degree).collect::<Vec<_>>(),
        [1, 2, 1, 0]
    );
    let p = shortest_path(&c, 1, 3).unwrap().unwrap();
    assert_eq!(p.buses, [1, 2, 3]);
    assert_eq!(p.hops, 2);
    assert!(shortest_path(&c, 1, 4).unwrap().is_none());
    assert_eq!(shortest_path(&c, 2, 2).unwrap().unwrap().hops, 0);
    assert!(shortest_path(&c, 0, 2).is_err());
}

#[test]
fn parallel_circuits_are_not_bridges_but_contribute_to_degree() {
    let c = case(3, &[(1, 2), (1, 2), (2, 3)]);
    let a = analyze(&c);
    assert_eq!(a.articulation_buses, [2]);
    assert_eq!(a.bridges.len(), 1);
    assert_eq!(a.bridges[0].equipment_id, "line:2");
    assert_eq!(
        a.degrees.iter().map(|d| d.degree).collect::<Vec<_>>(),
        [2, 3, 1]
    );
}

#[test]
fn graph_data_groups_transitive_active_zero_impedance_lines_deterministically() {
    let mut c = case(6, &[(1, 2), (2, 3), (3, 4), (4, 5)]);
    c.zero_impedance_threshold = 0.1;
    for (i, (r, x)) in [("0", "-.1"), ("0", ".08"), ("0", ".10001"), (".001", "0")]
        .into_iter()
        .enumerate()
    {
        c.equipment[i].source.fields.insert("R".into(), r.into());
        c.equipment[i].source.fields.insert("X".into(), x.into());
    }
    c.equipment.push(Equipment {
        id: "line:4".into(),
        kind: EquipmentKind::Line,
        name: String::new(),
        circuit: "4".into(),
        buses: vec![5, 6],
        status: 0,
        active_terminals: vec![false, false],
        source: source(),
    });
    c.equipment[4].source.fields.insert("R".into(), "0".into());
    c.equipment[4].source.fields.insert("X".into(), "0".into());
    c.equipment.push(Equipment {
        id: "transformer:1:6:A".into(),
        kind: EquipmentKind::Transformer,
        name: String::new(),
        circuit: "A".into(),
        buses: vec![1, 6],
        status: 1,
        active_terminals: vec![true, true],
        source: source(),
    });
    c.equipment[5].source.fields.insert("R".into(), "0".into());
    c.equipment[5].source.fields.insert("X".into(), "0".into());

    let graph = nodal::topology::Topology::new(&c).graph_data();
    assert_eq!(graph.zero_impedance_groups, [vec![1, 2, 3]]);
    assert_eq!(graph.nodes.len(), 6);
}

#[test]
fn zero_threshold_disables_groups_and_excluded_buses_are_omitted() {
    let mut c = case(2, &[(1, 2)]);
    c.equipment[0].source.fields.insert("R".into(), "0".into());
    c.equipment[0].source.fields.insert("X".into(), "0".into());
    c.zero_impedance_threshold = 0.0;
    assert!(
        nodal::topology::Topology::new(&c)
            .graph_data()
            .zero_impedance_groups
            .is_empty()
    );

    c.zero_impedance_threshold = 0.1;
    c.buses[1].bus_type = 4;
    assert!(
        nodal::topology::Topology::new(&c)
            .graph_data()
            .zero_impedance_groups
            .is_empty()
    );
}

#[test]
fn zero_impedance_line_uses_default_zero_resistance_when_field_is_missing_or_blank() {
    for resistance in [None, Some("")] {
        let mut c = case(2, &[(1, 2)]);
        c.zero_impedance_threshold = 0.1;
        if let Some(resistance) = resistance {
            c.equipment[0]
                .source
                .fields
                .insert("R".into(), resistance.into());
        }
        c.equipment[0]
            .source
            .fields
            .insert("X".into(), "0.05".into());
        assert_eq!(
            nodal::topology::Topology::new(&c)
                .graph_data()
                .zero_impedance_groups,
            [vec![1, 2]]
        );
    }
}

#[test]
fn cycle_has_no_cuts_and_excludes_inactive_equipment_and_buses() {
    let mut c = case(4, &[(1, 2), (2, 3), (3, 1), (3, 4)]);
    let a = analyze(&c);
    assert_eq!(a.bridges.len(), 1);
    c.equipment[3].status = 0;
    c.equipment[3].active_terminals = vec![false, false];
    let a = analyze(&c);
    assert!(a.bridges.is_empty());
    assert!(a.articulation_buses.is_empty());
    assert_eq!(a.active_equipment, 3);
    assert_eq!(a.excluded_equipment, 1);
    c.buses[3].bus_type = 4;
    let a = analyze(&c);
    assert_eq!(a.active_buses, 3);
    assert_eq!(a.excluded_buses, 1);
    assert!(shortest_path(&c, 1, 4).is_err());
}

#[test]
fn three_winding_transformer_counts_as_one_equipment_hop() {
    let mut c = case(4, &[(3, 4)]);
    c.equipment.push(Equipment {
        id: "transformer:1:2:3:A".into(),
        kind: EquipmentKind::Transformer,
        name: "Three".into(),
        circuit: "A".into(),
        buses: vec![1, 2, 3],
        status: 1,
        active_terminals: vec![true, true, true],
        source: source(),
    });
    let a = analyze(&c);
    assert_eq!(a.synthetic_junctions, 1);
    assert_eq!(a.active_equipment, 2);
    assert_eq!(a.articulation_buses, [3]);
    assert_eq!(a.synthetic_articulations.len(), 1);
    let p = shortest_path(&c, 1, 4).unwrap().unwrap();
    assert_eq!(p.buses, [1, 3, 4]);
    assert_eq!(p.hops, 2);
    assert_eq!(p.equipment_ids, ["transformer:1:2:3:A", "line:0"]);
    c.equipment[1].status = 2;
    c.equipment[1].active_terminals = vec![true, false, true];
    let a = analyze(&c);
    assert_eq!(
        a.islands
            .iter()
            .map(|i| i.buses.clone())
            .collect::<Vec<_>>(),
        [vec![1, 3, 4], vec![2]]
    );
    assert!(shortest_path(&c, 1, 2).unwrap().is_none());
}

#[test]
fn paths_choose_equipment_hops_rather_than_synthetic_edge_count() {
    let mut c = case(4, &[(1, 2), (2, 3), (3, 4)]);
    c.equipment.push(Equipment {
        id: "transformer".into(),
        kind: EquipmentKind::Transformer,
        name: String::new(),
        circuit: "1".into(),
        buses: vec![1, 3, 4],
        status: 1,
        active_terminals: vec![true; 3],
        source: source(),
    });
    let p = shortest_path(&c, 1, 4).unwrap().unwrap();
    assert_eq!(p.hops, 1);
    assert_eq!(p.equipment_ids, ["transformer"]);
}

#[test]
fn supplied_cases_analyze_without_losing_equipment_identities() {
    for path in [
        "MemphisCase2026_Mar7.RAW",
        "Texas2k_series25_case1_summerpeak.RAW",
    ] {
        let c = parse_raw(&std::fs::read(path).unwrap()).unwrap();
        let a = analyze(&c);
        assert_eq!(a.active_buses + a.excluded_buses, c.buses.len());
        assert_eq!(a.active_equipment + a.excluded_equipment, c.equipment.len());
        assert!(a.active_equipment > 0);
        assert!(
            a.bridges
                .iter()
                .all(|b| c.equipment.iter().any(|e| e.id == b.equipment_id))
        );
    }
}

#[test]
fn merged_chain_contracts_zero_line_and_keeps_unmerged_default() {
    let mut c = case(3, &[(1, 2), (2, 3)]);
    c.equipment[0].source.fields.insert("R".into(), "0".into());
    c.equipment[0]
        .source
        .fields
        .insert("X".into(), "0.0001".into());
    let unmerged = nodal::topology::Topology::new(&c);
    assert_eq!(unmerged.graph_data().nodes.len(), 3);

    let merged = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let graph = merged.graph_data();
    assert_eq!(
        graph.nodes.iter().filter_map(|n| n.bus).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|n| n.member_buses.clone())
            .collect::<Vec<_>>(),
        [vec![1, 2], vec![3]]
    );
    assert!(graph.merge_zero_impedance);
    assert_eq!(graph.edges.len(), 1);
    let analysis = merged.analyze(&c);
    assert_eq!(analysis.original_active_buses, 3);
    assert_eq!(analysis.active_buses, 2);
    assert_eq!(analysis.absorbed_equipment, 1);
    assert_eq!(analysis.active_equipment, 1);
}

#[test]
fn merged_paths_resolve_any_group_member_and_return_representatives() {
    let mut c = case(4, &[(1, 4), (4, 2), (2, 3)]);
    for index in [0, 1] {
        c.equipment[index]
            .source
            .fields
            .insert("R".into(), "0".into());
        c.equipment[index]
            .source
            .fields
            .insert("X".into(), "0".into());
    }
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    assert_eq!(topology.graph_data().nodes[0].member_buses, [1, 2, 4]);
    let zero = topology.shortest_path(&c, 2, 4).unwrap().unwrap();
    assert_eq!(zero.hops, 0);
    assert_eq!(zero.buses, [1]);
    assert_eq!(zero.node_ids, ["bus:1"]);
    let external = topology.shortest_path(&c, 2, 3).unwrap().unwrap();
    assert_eq!(external.buses, [1, 3]);
    assert_eq!(external.equipment_ids, ["line:2"]);
}

#[test]
fn merged_three_winding_collisions_contract_to_effective_terminals() {
    let mut c = case(5, &[(1, 2), (3, 4)]);
    c.equipment[0].source.fields.insert("R".into(), "0".into());
    c.equipment[0].source.fields.insert("X".into(), "0".into());
    c.equipment.push(Equipment {
        id: "transformer:1:2:5:A".into(),
        kind: EquipmentKind::Transformer,
        name: "Three".into(),
        circuit: "A".into(),
        buses: vec![1, 2, 5],
        status: 1,
        active_terminals: vec![true; 3],
        source: source(),
    });
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let graph = topology.graph_data();
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| n.equipment_id.is_some())
            .count(),
        0
    );
    assert_eq!(graph.edges.len(), 2);
    let path = topology.shortest_path(&c, 2, 5).unwrap().unwrap();
    assert_eq!(path.hops, 1);
    assert_eq!(path.buses, [1, 5]);
    assert_eq!(path.equipment_ids, ["transformer:1:2:5:A"]);
}

#[test]
fn contractions_keep_parallel_external_circuits_and_update_cut_analysis() {
    let mut c = case(4, &[(1, 2), (1, 3), (2, 3), (3, 4)]);
    {
        let index = 0;
        c.equipment[index]
            .source
            .fields
            .insert("R".into(), "0".into());
        c.equipment[index]
            .source
            .fields
            .insert("X".into(), "0".into());
    }
    let merged = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let analysis = merged.analyze(&c);
    assert_eq!(analysis.active_buses, 3);
    assert_eq!(
        analysis
            .degrees
            .iter()
            .map(|degree| degree.degree)
            .collect::<Vec<_>>(),
        [2, 3, 1]
    );
    assert_eq!(analysis.articulation_buses, [3]);
    assert_eq!(
        analysis
            .bridges
            .iter()
            .map(|edge| edge.equipment_id.as_str())
            .collect::<Vec<_>>(),
        ["line:3"]
    );
    let contracted_edge = merged
        .graph_data()
        .edges
        .into_iter()
        .filter(|edge| {
            edge.source == "bus:1" && edge.target == "bus:3"
                || edge.source == "bus:3" && edge.target == "bus:1"
        })
        .count();
    assert_eq!(contracted_edge, 2);
}

#[test]
fn merged_mode_absorbs_one_node_transformer_and_preserves_exclusions() {
    let mut c = case(4, &[(1, 2), (2, 3), (3, 4)]);
    for index in 0..2 {
        c.equipment[index]
            .source
            .fields
            .insert("R".into(), "0".into());
        c.equipment[index]
            .source
            .fields
            .insert("X".into(), "0".into());
    }
    c.buses[3].bus_type = 4;
    c.equipment.push(Equipment {
        id: "transformer:1:2:3:A".into(),
        kind: EquipmentKind::Transformer,
        name: "Three".into(),
        circuit: "A".into(),
        buses: vec![1, 2, 3],
        status: 1,
        active_terminals: vec![true; 3],
        source: source(),
    });
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let analysis = topology.analyze(&c);
    assert_eq!(analysis.original_active_buses, 3);
    assert_eq!(analysis.active_buses, 1);
    assert_eq!(analysis.excluded_buses, 1);
    assert_eq!(analysis.absorbed_equipment, 3);
    assert_eq!(analysis.excluded_equipment, 1);
    assert_eq!(
        topology.graph_data().absorbed_equipment_ids,
        ["line:0", "line:1", "transformer:1:2:3:A"]
    );
    assert_eq!(topology.graph_data().nodes[0].member_buses, [1, 2, 3]);
    assert!(
        topology
            .shortest_path(&c, 1, 2)
            .unwrap()
            .unwrap()
            .equipment_ids
            .is_empty()
    );
    assert!(topology.shortest_path(&c, 1, 4).is_err());
}

#[test]
fn merged_three_winding_with_three_terminals_uses_junction_and_one_node_is_absorbed() {
    let mut c = case(4, &[(1, 2)]);
    c.equipment[0].source.fields.insert("R".into(), "0".into());
    c.equipment[0].source.fields.insert("X".into(), "0".into());
    c.equipment.push(Equipment {
        id: "transformer:1:2:3:A".into(),
        kind: EquipmentKind::Transformer,
        name: "Three".into(),
        circuit: "A".into(),
        buses: vec![1, 3, 4],
        status: 1,
        active_terminals: vec![true; 3],
        source: source(),
    });
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let graph = topology.graph_data();
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.equipment_id.is_some())
            .count(),
        1
    );
    let path = topology.shortest_path(&c, 1, 4).unwrap().unwrap();
    assert_eq!(path.hops, 1);
    assert_eq!(path.buses, [1, 4]);
    assert_eq!(
        path.node_ids,
        ["bus:1", "junction:transformer:1:2:3:A", "bus:4"]
    );
}

#[test]
fn merged_option_respects_zero_threshold_and_all_excluded_cases() {
    let mut c = case(2, &[(1, 2)]);
    c.equipment[0].source.fields.insert("R".into(), "0".into());
    c.equipment[0].source.fields.insert("X".into(), "0".into());
    c.zero_impedance_threshold = 0.0;
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    assert_eq!(topology.graph_data().nodes.len(), 2);
    assert!(topology.graph_data().zero_impedance_groups.is_empty());

    c.buses.iter_mut().for_each(|bus| bus.bus_type = 4);
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let analysis = topology.analyze(&c);
    assert_eq!(analysis.original_active_buses, 0);
    assert_eq!(analysis.active_buses, 0);
    assert_eq!(analysis.excluded_buses, 2);
    assert!(analysis.islands.is_empty());
    assert_eq!(analysis.excluded_equipment, 1);
}

#[test]
fn unmerged_partial_three_winding_still_uses_star_in_paths() {
    let mut c = case(3, &[]);
    c.equipment.push(Equipment {
        id: "transformer:1:2:3:A".into(),
        kind: EquipmentKind::Transformer,
        name: "Three".into(),
        circuit: "A".into(),
        buses: vec![1, 2, 3],
        status: 2,
        active_terminals: vec![true, false, true],
        source: source(),
    });
    let topology = nodal::topology::Topology::new(&c);
    let path = topology.shortest_path(&c, 1, 3).unwrap().unwrap();
    assert_eq!(
        path.node_ids,
        ["bus:1", "junction:transformer:1:2:3:A", "bus:3"]
    );
    assert_eq!(
        path.connection_ids,
        [
            "connection:transformer:1:2:3:A:w1",
            "connection:transformer:1:2:3:A:w3"
        ]
    );
}

#[test]
fn merged_partial_three_winding_uses_one_direct_effective_connection() {
    let mut c = case(3, &[]);
    c.equipment.push(Equipment {
        id: "transformer:1:2:3:A".into(),
        kind: EquipmentKind::Transformer,
        name: "Three".into(),
        circuit: "A".into(),
        buses: vec![1, 2, 3],
        status: 2,
        active_terminals: vec![true, false, true],
        source: source(),
    });
    let topology = nodal::topology::Topology::with_options(
        &c,
        nodal::topology::TopologyOptions {
            merge_zero_impedance: true,
        },
    );
    let graph = topology.graph_data();
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.equipment_id.is_some())
            .count(),
        0
    );
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].id, "connection:transformer:1:2:3:A:w1");
    let path = topology.shortest_path(&c, 1, 3).unwrap().unwrap();
    assert_eq!(path.hops, 1);
    assert_eq!(path.node_ids, ["bus:1", "bus:3"]);
    assert_eq!(path.connection_ids, [graph.edges[0].id.as_str()]);
}
