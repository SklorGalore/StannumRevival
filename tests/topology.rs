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
