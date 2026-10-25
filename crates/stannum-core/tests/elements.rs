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

/// Case diff records name the same elements as nodal, so a selection can
/// jump between Topology and Case diff.
#[test]
fn diff_records_match_nodal_ids() {
    let path = sample("MemphisCase2026_Mar7.RAW");
    let mut ws = open(path.clone());
    let topo = &ws.case.as_ref().unwrap().topology.as_ref().unwrap().case;
    let mut known: HashSet<String> = topo
        .buses
        .iter()
        .map(|b| format!("bus:{}", b.number))
        .collect();
    known.extend(topo.equipment.iter().map(|e| e.id.clone()));
    known.extend(topo.loads.iter().map(|e| e.id.clone()));
    known.extend(topo.generators.iter().map(|e| e.id.clone()));
    ws.compare(&path, &path).unwrap();
    let cmp = ws.comparison().unwrap();
    let mut mapped = 0;
    for section in [
        "BUS",
        "LOAD",
        "GENERATOR",
        "BRANCH",
        "TRANSFORMER",
        "SYSTEM SWITCHING DEVICE",
    ] {
        for row in cmp.section_records(section) {
            let element = row.element.expect("record has an element id");
            assert!(
                known.contains(&element),
                "{section}: {element} not in nodal"
            );
            mapped += 1;
        }
    }
    assert_eq!(mapped, known.len());
    assert!(
        cmp.section_records("AREA")
            .iter()
            .all(|r| r.element.is_none())
    );
}

fn rs_power_raw(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs-power/tests/fixtures/raw")
        .join(name)
}

#[test]
fn dc_lines_reach_the_diagram_network() {
    let ws = open(rs_power_raw("t_psse_case3.raw"));
    let net = ws.current().unwrap().diagram_network().unwrap();
    let lines: Vec<_> = net
        .dc_lines
        .iter()
        .map(|d| (d.element.as_str(), d.kind, d.from, d.to, d.online))
        .collect();
    assert_eq!(
        lines,
        [
            ("dcline:301:3021:1", "two_terminal", 301, 3021, true),
            ("dcline:301:3022:2", "two_terminal", 301, 3022, true),
            ("dcline:3005:3008:VDCLINE1", "vsc", 3005, 3008, true),
            ("dcline:203:205:VDCLINE2", "vsc", 203, 205, true),
            ("dcline:212:402:1", "multi_terminal", 402, 212, true),
            ("dcline:212:401:1", "multi_terminal", 401, 212, true),
            ("dcline:212:213:1", "multi_terminal", 212, 213, true),
        ]
    );
    assert!(net.dc_lines.iter().all(|d| d.mw > 0.0));

    // Names keep their spaces, and MATPOWER-style numbering is not used for RAW.
    let ws = open(rs_power_raw("dc_modes.raw"));
    let net = ws.current().unwrap().diagram_network().unwrap();
    let names: Vec<_> = net.dc_lines.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["DC 0", "DC 1", "DC 2"]);
    assert!(!net.dc_lines[0].online, "mode 0 is blocked");
}
