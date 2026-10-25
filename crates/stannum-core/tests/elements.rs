//! Shared element ids agree between rs-power rows and nodal equipment.
use stannum_core::Workspace;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn open(path: PathBuf) -> Workspace {
    let mut ws = Workspace::default();
    ws.open(&path).unwrap();
    ws
}

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../nodal")
        .join(name)
}

/// Every rs-power branch and generator names a nodal element, and every
/// nodal line and transformer is found among the rs-power rows.
fn check(path: PathBuf) -> (usize, usize) {
    let ws = open(path);
    let case = ws.case.as_ref().unwrap();
    let power = case.power.as_ref().unwrap();
    let topo = case.topology.as_ref().unwrap();

    let equipment: HashSet<&str> = topo.case.equipment.iter().map(|e| e.id.as_str()).collect();
    let generators: HashSet<&str> = topo.case.generators.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(power.elements.branches.len(), power.case.branches.len());
    assert_eq!(power.elements.generators.len(), power.case.generators.len());

    let mut rows: HashMap<&str, usize> = HashMap::new();
    for b in &power.elements.branches {
        assert!(
            equipment.contains(b.element.as_str()),
            "{} not in nodal",
            b.element
        );
        *rows.entry(&b.element).or_default() += 1;
    }
    for g in &power.elements.generators {
        assert!(
            generators.contains(g.element.as_str()),
            "{} not in nodal",
            g.element
        );
    }
    for e in &topo.case.equipment {
        let expected = match e.kind {
            nodal::model::EquipmentKind::Switch => continue,
            _ if e.buses.len() == 3 => 3,
            _ => 1,
        };
        assert_eq!(rows.get(e.id.as_str()).copied(), Some(expected), "{}", e.id);
    }
    let parallel = power
        .elements
        .branches
        .iter()
        .filter(|b| b.circuit != "1" && b.winding == 0)
        .count();
    let three_winding = power
        .elements
        .branches
        .iter()
        .filter(|b| b.winding > 0)
        .count();
    (parallel, three_winding)
}

#[test]
fn texas2k_ids_match_nodal() {
    // Three-winding legs are covered by rs-power's RAW tests; this sample has none.
    let (parallel, _) = check(sample("Texas2k_series25_case1_summerpeak.RAW"));
    assert!(parallel > 0, "sample should exercise circuit ids");
}

#[test]
fn memphis_ids_match_nodal() {
    check(sample("MemphisCase2026_Mar7.RAW"));
}

#[test]
fn matpower_parallel_branches_are_numbered() {
    let ws = open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/case9.m"));
    let power = ws.case.as_ref().unwrap().power.as_ref().unwrap();
    let ids: HashSet<&str> = power
        .elements
        .branches
        .iter()
        .map(|b| b.element.as_str())
        .collect();
    assert_eq!(ids.len(), power.case.branches.len());
    assert!(ids.contains("line:1:4:1"));
    assert_eq!(power.elements.generators[0].element, "generator:1:1");
}
