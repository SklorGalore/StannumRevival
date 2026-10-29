//! RAW system switching devices: read into the power flow as lossless
//! branches, drawn on one-lines, inspected, and left out of default N-1.
use stannum_core::Workspace;
use stannum_core::contingency::{ContingencySettings, Method};
use stannum_core::diagram::network;
use stannum_core::inspect::Value;
use stannum_core::power::SolveSettings;
use std::path::PathBuf;

fn open() -> Workspace {
    let mut ws = Workspace::default();
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/five_bus_switches.raw");
    ws.open(&path).unwrap();
    ws
}

fn text(fields: &[stannum_core::inspect::Field], label: &str) -> String {
    match fields.iter().find(|f| f.label == label).map(|f| &f.value) {
        Some(Value::Text(t)) => t.clone(),
        other => panic!("{label}: {other:?}"),
    }
}

#[test]
fn closed_devices_carry_power_and_open_ones_dont() {
    let mut ws = open();
    let case = ws.current_mut().unwrap();
    let power = case.power.as_ref().unwrap().clone();
    assert!(
        power
            .warnings
            .iter()
            .all(|w| !w.message.contains("switching")),
        "{:?}",
        power.warnings
    );
    assert_eq!(power.switching_devices.len(), 3);
    let report = case.solve(SolveSettings::default()).unwrap();
    assert!(report.converged, "{}", report.status);
    let row = |element: &str| {
        report
            .branches
            .iter()
            .find(|r| r.element == element)
            .unwrap_or_else(|| panic!("{element}"))
    };
    // Bus 6 passes on what the line to East and disconnector D1 take.
    let tie = row("switch:2:6:BT");
    let east = row("line:3:6:1");
    let d1 = row("switch:6:7:D1");
    assert!(
        (tie.pf - (east.pf + d1.pf)).abs() < 0.5,
        "{} vs {} + {}",
        tie.pf,
        east.pf,
        d1.pf
    );
    // D1 feeds bus 7's 15 MW load; the open coupler carries nothing.
    assert!((d1.pf - 15.0).abs() < 0.1, "{}", d1.pf);
    let bc = row("switch:2:7:BC");
    assert!(!bc.online);
    assert_eq!(bc.pf, 0.0);
    // Lossless: no voltage drop across a closed breaker to speak of.
    let v = |id: u32| report.buses.iter().find(|b| b.id == id).unwrap().vm;
    assert!((v(2) - v(6)).abs() < 1e-3);
}

#[test]
fn one_lines_know_what_each_device_is() {
    let ws = open();
    let power = ws.case.as_ref().unwrap().power.as_ref().unwrap();
    let net = network(power);
    let device = |element: &str| {
        let br = net.branches.iter().find(|b| b.element == element).unwrap();
        assert!(!br.transformer);
        (br.online, br.switching.clone().expect("a switching device"))
    };
    let (online, bt) = device("switch:2:6:BT");
    assert!(online);
    assert_eq!(
        (bt.kind, bt.name.as_str(), bt.normally_open),
        ("breaker", "Central BT", false)
    );
    let (_, d1) = device("switch:6:7:D1");
    assert_eq!(d1.kind, "disconnector");
    let (online, bc) = device("switch:2:7:BC");
    assert!(!online);
    assert!(bc.normally_open);
    assert!(
        net.branches
            .iter()
            .find(|b| b.element == "line:1:2:1")
            .unwrap()
            .switching
            .is_none()
    );
}

#[test]
fn the_inspector_describes_a_breaker() {
    let ws = open();
    let case = ws.case.as_ref().unwrap();
    let d = case.element_detail("switch:2:7:BC").unwrap();
    assert!(!d.online);
    assert_eq!(
        d.buses.iter().map(|b| b.number).collect::<Vec<_>>(),
        vec![2, 7]
    );
    let fields = &d.data[0].fields;
    assert_eq!(text(fields, "Type"), "Circuit breaker");
    assert_eq!(text(fields, "Name"), "Central BC");
    assert_eq!(text(fields, "Status"), "Open");
    assert_eq!(text(fields, "Normal status"), "Open");
    assert!(
        case.element_detail("bus:2")
            .unwrap()
            .connected
            .contains(&"switch:2:6:BT".to_string())
    );
}

#[test]
fn default_n_minus_1_leaves_devices_out_unless_listed() {
    let mut ws = open();
    let case = ws.current_mut().unwrap();
    case.solve(SolveSettings::default()).unwrap();
    let settings = ContingencySettings {
        elements: None,
        branches: true,
        generators: false,
        method: Method::Screen,
        ac_count: 0,
        limit_pct: 100.0,
        solve: SolveSettings::default(),
        planning: None,
    };
    case.run_contingencies(settings.clone()).unwrap();
    let rows = case.contingency_rows(0, 100).unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| !r.element.starts_with("switch:")));
    case.run_contingencies(ContingencySettings {
        elements: Some(vec!["switch:2:6:BT".into()]),
        ..settings
    })
    .unwrap();
    let rows = case.contingency_rows(0, 100).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].element, "switch:2:6:BT");
}
