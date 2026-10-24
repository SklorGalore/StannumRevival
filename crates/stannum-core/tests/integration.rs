//! End-to-end checks of the three engines through the shared workspace.
use stannum_core::Workspace;
use stannum_core::diff::Status;
use stannum_core::power::{ModelChoice, SolveSettings};
use stannum_core::workspace::CaseFormat;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn matpower_case_solves_and_reports_limits() {
    let mut ws = Workspace::default();
    let summary = ws.open(&fixture("case9.m")).unwrap();
    assert_eq!(summary.format, CaseFormat::Matpower);
    assert!(summary.power.available);
    assert!(!summary.topology.available, "topology needs RAW v35");
    assert!(!summary.diffable);

    let report = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings::default())
        .unwrap();
    assert!(report.converged);
    assert_eq!(report.buses.len(), 9);
    assert_eq!(report.branches.len(), 9);
    assert_eq!(report.limits.branch_overloads, 0);
    let worst = report.limits.worst_branch.unwrap();
    assert!(worst.2 > 0.0 && worst.2 < 100.0);
    // Generation balances load plus losses.
    let balance = report.total_generation_mw - report.total_load_mw - report.total_losses_mw;
    assert!(balance.abs() < 1e-6, "power balance off by {balance}");

    let dc = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings {
            model: ModelChoice::Dc,
            enforce_q_limits: true, // ignored for DC rather than rejected
            ..SolveSettings::default()
        })
        .unwrap();
    assert!(dc.converged);
    assert!(dc.total_losses_mw.abs() < 1e-9);
}

#[test]
fn sensitivities_follow_the_sign_convention() {
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let case = ws.current_mut().unwrap();
    let summary = case.sensitivities(SolveSettings::default()).unwrap();
    assert_eq!(summary.quantities.len(), 6);
    // Bus 1 is the reference bus, so it has no injection column.
    assert!(summary.injections.iter().all(|c| c.bus != 1));
    let column = summary
        .injections
        .iter()
        .find(|c| c.bus == 5 && c.kind == "p")
        .unwrap()
        .column;
    let angles = case.responses(column, "angle", true).unwrap();
    assert_eq!(angles.units, "degrees/MW");
    // Injecting at bus 5 raises its own angle.
    let own = angles.rows.iter().find(|r| r.label == "Bus 5").unwrap();
    assert!(own.value > 0.0);
    let flows = case.responses(column, "flow0", false).unwrap();
    assert_eq!(flows.rows.len(), 9);
    assert!(case.responses(column, "nonsense", true).is_err());
}

#[test]
fn matpower_export_round_trips() {
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let case = ws.current_mut().unwrap();
    let dir = std::env::temp_dir().join(format!("stannum-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("solved_case9.m");
    assert!(
        case.export_matpower(&out, true).is_err(),
        "nothing solved yet"
    );
    case.solve(SolveSettings::default()).unwrap();
    case.export_matpower(&out, true).unwrap();
    let mut again = Workspace::default();
    let summary = again.open(&out).unwrap();
    assert_eq!(summary.power.info.unwrap().counts.buses, 9);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn raw_v35_case_feeds_all_three_engines() {
    let mut ws = Workspace::default();
    let summary = ws.open(&fixture("five_bus.raw")).unwrap();
    assert_eq!(summary.format, CaseFormat::Raw);
    assert!(summary.power.available, "{:?}", summary.power.error);
    assert!(summary.topology.available, "{:?}", summary.topology.error);
    assert!(summary.diffable);

    let case = ws.current_mut().unwrap();
    let report = case.solve(SolveSettings::default()).unwrap();
    assert!(report.converged, "{}", report.status);
    // rs-power adds no star buses for a two-winding transformer.
    assert_eq!(report.buses.len(), 5);

    let snapshot = case.topology_snapshot().unwrap();
    assert_eq!(snapshot.analysis.islands.len(), 1);
    // The 69 kV spur (2 -> 4 -> 5) hangs off the meshed 138 kV triangle.
    assert_eq!(snapshot.analysis.bridges.len(), 2);
    let mut cuts = snapshot.analysis.articulation_buses.clone();
    cuts.sort();
    assert_eq!(cuts, [2, 4]);
    let path = case.shortest_path(1, 5).unwrap().unwrap();
    assert_eq!(path.buses, [1, 2, 4, 5]);
    assert_eq!(path.hops, 3);
    assert!(case.shortest_path(1, 99).is_err());
}

#[test]
fn case_diff_matches_records_by_key() {
    let mut ws = Workspace::default();
    let summary = ws
        .compare(&fixture("five_bus.raw"), &fixture("five_bus_modified.raw"))
        .unwrap();
    assert!(summary.complete);
    assert_eq!(summary.baseline.revision, 35);
    let counts = |name: &str| {
        let s = summary.sections.iter().find(|s| s.name == name).unwrap();
        (
            s.counts.added,
            s.counts.removed,
            s.counts.changed,
            s.counts.unchanged,
        )
    };
    assert_eq!(counts("BUS"), (1, 1, 0, 4));
    assert_eq!(counts("LOAD"), (1, 1, 1, 1));
    assert_eq!(counts("BRANCH"), (1, 1, 1, 2));
    assert_eq!(counts("GENERATOR"), (0, 0, 0, 2));
    assert_eq!(summary.differing, 8);

    let comparison = ws.comparison().unwrap();
    let loads = comparison.section_records("LOAD");
    let changed = loads.iter().find(|r| r.status == Status::Changed).unwrap();
    // Identifier padding is normalized for matching.
    assert_eq!(changed.key, "4 / '1'");
    let detail = comparison.record_detail("LOAD", changed.index).unwrap();
    let pl = detail.fields.iter().find(|f| f.name == "PL").unwrap();
    assert_eq!(pl.status, Status::Changed);
    assert_eq!(pl.before.as_deref(), Some("40.000"));
    assert_eq!(pl.after.as_deref(), Some("45.500"));
    // Formatting-only differences never count: unchanged fields stay unchanged.
    assert!(
        detail
            .fields
            .iter()
            .filter(|f| f.name != "PL")
            .all(|f| f.status == Status::Unchanged)
    );
}

#[test]
fn failed_comparison_clears_previous_results() {
    let mut ws = Workspace::default();
    ws.compare(&fixture("five_bus.raw"), &fixture("five_bus.raw"))
        .unwrap();
    assert!(ws.comparison().is_ok());
    assert!(
        ws.compare(&fixture("five_bus.raw"), &fixture("case9.m"))
            .is_err()
    );
    assert!(ws.comparison().is_err());
}

#[test]
fn diagram_network_describes_the_case() {
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus.raw")).unwrap();
    let net = ws.current().unwrap().diagram_network().unwrap();
    assert_eq!(net.buses.len(), 5);
    assert_eq!(net.generators.len(), 2);
    assert_eq!(net.branches.len(), 5);
    // The 138/69 kV transformer is the only branch drawn as one.
    let xf: Vec<_> = net.branches.iter().filter(|b| b.transformer).collect();
    assert_eq!(xf.len(), 1);
    assert_eq!((xf[0].from, xf[0].to), (2, 4));
    let bus2 = net.buses.iter().find(|b| b.id == 2).unwrap();
    assert_eq!(bus2.name, "Central_138");
    assert!((bus2.pd - 90.0).abs() < 1e-9);
    assert!(net.buses.iter().all(|b| !b.star));

    // Branch and generator indices line up with the power flow report.
    let report = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings::default())
        .unwrap();
    for b in &net.branches {
        let row = &report.branches[b.index];
        assert_eq!((row.from, row.to), (b.from, b.to));
    }
    for g in &net.generators {
        assert_eq!(report.generators[g.index].bus, g.bus);
    }

    ws.open(&fixture("case9.m")).unwrap();
    let net = ws.current().unwrap().diagram_network().unwrap();
    assert_eq!(net.buses.len(), 9);
    // MATPOWER case9 gives every branch tap 0 and every bus 345 kV, so nothing
    // marks its generator step-ups as transformers: all branches draw as lines.
    assert!(net.branches.iter().all(|b| !b.transformer));
}
