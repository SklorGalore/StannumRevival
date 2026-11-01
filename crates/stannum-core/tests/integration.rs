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
    // nodal can't read MATPOWER, so topology comes from the power flow
    // model. It's built the first time it's asked for.
    assert!(summary.topology.available && summary.topology.info.is_none());
    assert!(!ws.current().unwrap().topology_built());
    ws.current().unwrap().topology_snapshot().unwrap();
    let summary = ws.current().unwrap().summary();
    let topo = summary
        .topology
        .info
        .as_ref()
        .expect("topology from the power model");
    assert!(topo.derived);
    assert_eq!((topo.buses, topo.equipment), (9, 9));
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
    // Generation balances load plus losses, to within the bus mismatches
    // the default 0.1 MVA tolerance leaves.
    let balance = report.total_generation_mw - report.total_load_mw - report.total_losses_mw;
    let allowed = report.buses.len() as f64 * report.residual_mva + 1e-6;
    assert!(
        balance.abs() < allowed,
        "power balance off by {balance}, allowed {allowed}"
    );

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
    assert_eq!(own.element, "bus:5");
    let flows = case.responses(column, "flow0", false).unwrap();
    assert_eq!(flows.rows.len(), 9);
    // Each flow row names its branch, for drawing on the diagram.
    assert!(
        flows
            .rows
            .iter()
            .all(|r| r.element.starts_with("line:") || r.element.starts_with("transformer:"))
    );
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

#[test]
fn case_diff_names_dc_lines_like_the_diagram() {
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus_dc.raw")).unwrap();
    let net = ws.current().unwrap().diagram_network().unwrap();
    let drawn: Vec<&str> = net.dc_lines.iter().map(|d| d.element.as_str()).collect();
    assert_eq!(drawn, ["dcline:1:3:East link"]);

    let summary = ws
        .compare(
            &fixture("five_bus_dc.raw"),
            &fixture("five_bus_dc_modified.raw"),
        )
        .unwrap();
    // The layout is decoded, so DC records are no longer left unsupported.
    assert!(summary.complete, "{:?}", summary.diagnostics);
    let dc = summary
        .sections
        .iter()
        .find(|s| s.name == "TWO-TERMINAL DC")
        .unwrap();
    assert_eq!((dc.counts.added, dc.counts.changed), (1, 1));

    let comparison = ws.comparison().unwrap();
    let rows = comparison.section_records("TWO-TERMINAL DC");
    let changed = rows.iter().find(|r| r.status == Status::Changed).unwrap();
    // The same id the diagram gives the line, so a pick there finds this row.
    assert_eq!(changed.element.as_deref(), Some(drawn[0]));
    let added = rows.iter().find(|r| r.status == Status::Added).unwrap();
    assert_eq!(added.element.as_deref(), Some("dcline:2:5:Spur link"));

    let detail = comparison
        .record_detail("TWO-TERMINAL DC", changed.index)
        .unwrap();
    let setvl = detail.fields.iter().find(|f| f.name == "SETVL").unwrap();
    assert_eq!(
        (setvl.before.as_deref(), setvl.after.as_deref()),
        (Some("40.00"), Some("55.00"))
    );
    let meter = detail.fields.iter().find(|f| f.name == "METER").unwrap();
    assert_eq!(meter.before.as_deref(), Some("'I'"));
}

#[test]
fn derived_topology_matches_nodal_on_a_v35_case() {
    // Build the five-bus case both ways: nodal from the RAW records, and the
    // fallback from rs-power's model. Ids and the analysis must agree.
    let text = std::fs::read_to_string(fixture("five_bus.raw")).unwrap();
    let power = stannum_core::power::load(&text, false).unwrap();
    let derived = stannum_core::derived_topology::from_power(&power);
    let nodal = nodal::parse_raw(text.as_bytes()).unwrap();
    let ids = |c: &nodal::model::Case| {
        let mut v: Vec<_> = c.equipment.iter().map(|e| e.id.clone()).collect();
        v.sort();
        v
    };
    assert_eq!(ids(&derived), ids(&nodal));
    let mut buses: Vec<_> = derived.buses.iter().map(|b| b.number).collect();
    buses.sort();
    assert_eq!(buses, [1, 2, 3, 4, 5]);
    let a = nodal::topology::analyze(&derived);
    let b = nodal::topology::analyze(&nodal);
    assert_eq!(a.islands.len(), b.islands.len());
    assert_eq!(a.bridges.len(), b.bridges.len());
    let (mut x, mut y) = (a.articulation_buses.clone(), b.articulation_buses.clone());
    x.sort();
    y.sort();
    assert_eq!(x, y);
}

#[test]
fn derived_topology_keeps_three_winding_transformers_whole() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs-power/tests/fixtures/raw/transformers.raw");
    let mut ws = Workspace::default();
    let summary = ws.open(&path).unwrap();
    assert_eq!(summary.power.info.as_ref().unwrap().revision, Some(33));
    ws.current().unwrap().topology_snapshot().unwrap();
    let summary = ws.current().unwrap().summary();
    let topo = summary
        .topology
        .info
        .as_ref()
        .expect("revision 33 gets a derived topology");
    assert!(topo.derived);
    let snapshot = ws.current().unwrap().topology_snapshot().unwrap();
    // No internal star buses, and each three-winding transformer is one
    // piece of equipment with three terminals.
    assert!(
        snapshot
            .case
            .buses
            .iter()
            .all(|b| !b.name.starts_with("STR_PT_XF_"))
    );
    let three: Vec<_> = snapshot
        .case
        .equipment
        .iter()
        .filter(|e| e.id.starts_with("transformer:") && e.buses.len() == 3)
        .collect();
    assert!(
        !three.is_empty(),
        "the fixture has three-winding transformers"
    );
    for e in three {
        assert_eq!(e.active_terminals.len(), 3);
    }
}

#[test]
fn study_edits_change_what_is_solved() {
    use stannum_core::study::Edit;
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let case = ws.current_mut().unwrap();
    // Results are compared to 1e-6, so solve tighter than the 0.1 MVA default.
    let tight = || SolveSettings {
        tolerance_mva: 1e-6,
        ..SolveSettings::default()
    };
    let base = case.solve(tight()).unwrap();
    // A line on the ring, so taking it out leaves every bus connected.
    let line = base
        .branches
        .iter()
        .filter(|b| b.from >= 4 && b.to >= 4)
        .max_by(|a, b| a.loading_pct.partial_cmp(&b.loading_pct).unwrap())
        .unwrap();
    let element = line.element.clone();

    case.set_edits(vec![
        Edit::Status {
            element: element.clone(),
            online: false,
        },
        Edit::Load {
            bus: 5,
            mw: 125.0,
            mvar: 60.0,
        },
    ])
    .unwrap();
    let out = case.solve(tight()).unwrap();
    assert!(out.converged);
    let row = out.branches.iter().find(|b| b.element == element).unwrap();
    assert!(
        !row.online && row.pf.abs() < 1e-9,
        "the outaged line carries nothing"
    );
    assert!(
        (out.total_load_mw - base.total_load_mw - 35.0).abs() < 1e-6,
        "bus 5 load went from 90 to 125 MW"
    );

    // A bad edit is refused and the study stays as it was.
    let err = case
        .set_edits(vec![Edit::Status {
            element: "line:1:99:1".into(),
            online: false,
        }])
        .unwrap_err();
    assert!(err.message.contains("line:1:99:1"));
    assert_eq!(case.edits.len(), 2);

    let unit = out.generators[1].element.clone();
    case.set_edits(vec![Edit::Generation {
        element: unit,
        mw: 100.0,
    }])
    .unwrap();
    let redispatched = case.solve(tight()).unwrap();
    assert!((redispatched.generators[1].pg - 100.0).abs() < 1e-6);

    // Cutting generator 2's only line leaves an island the solver can't take.
    case.set_edits(vec![Edit::Status {
        element: "line:2:8:1".into(),
        online: false,
    }])
    .unwrap();
    let err = case.solve(tight()).unwrap_err();
    assert!(
        err.message
            .starts_with("The study's outages split the network"),
        "{err}"
    );

    // No edits: back to the file. This solve starts from the redispatched
    // one, so it agrees with the first to the solver's tolerance.
    case.set_edits(vec![]).unwrap();
    let again = case.solve(tight()).unwrap();
    assert!((again.total_losses_mw - base.total_losses_mw).abs() < 1e-6);
}

#[test]
fn last_solution_start_resumes_the_previous_solve() {
    use stannum_core::power::StartChoice;
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus.raw")).unwrap();
    let case = ws.current_mut().unwrap();
    let last = SolveSettings {
        start: StartChoice::Last,
        enforce_q_limits: true,
        // Tight enough that a 1 MW load edit needs Newton steps.
        tolerance_mva: 1e-6,
        ..SolveSettings::default()
    };
    // Nothing solved yet: the case voltages, like "From case".
    let first = case.solve(last.clone()).unwrap();
    let cold = case
        .solve(SolveSettings {
            start: StartChoice::Case,
            ..last.clone()
        })
        .unwrap();
    assert!(first.converged && first.iterations > 0);
    assert_eq!(first.iterations, cold.iterations);
    // Again from that solution: nothing left to do, same answer.
    let again = case.solve(last.clone()).unwrap();
    assert!(again.converged);
    assert_eq!(again.iterations, 0);
    assert!((again.total_losses_mw - cold.total_losses_mw).abs() < 1e-6);
    for (a, b) in again.buses.iter().zip(&cold.buses) {
        assert!((a.vm - b.vm).abs() < 1e-8);
    }
    assert_eq!(
        (again.shunts_switched, again.taps_moved),
        (cold.shunts_switched, cold.taps_moved)
    );
    // A DC solve leaves no AC solution to start from.
    case.solve(SolveSettings {
        model: ModelChoice::Dc,
        ..last.clone()
    })
    .unwrap();
    assert_eq!(
        case.solve(last.clone()).unwrap().iterations,
        cold.iterations
    );
    // After a study edit the last solve is still the starting point.
    case.solve(last.clone()).unwrap();
    case.set_edits(vec![stannum_core::study::Edit::Load {
        bus: first.buses[1].id,
        mw: first.buses[1].pd + 1.0,
        mvar: first.buses[1].qd,
    }])
    .unwrap();
    let edited = case.solve(last.clone()).unwrap();
    let edited_cold = case
        .solve(SolveSettings {
            start: StartChoice::Case,
            ..last.clone()
        })
        .unwrap();
    assert!(edited.converged && edited.iterations > 0);
    assert!(edited.iterations < edited_cold.iterations);
    for (a, b) in edited.buses.iter().zip(&edited_cold.buses) {
        assert!((a.vm - b.vm).abs() < 1e-6);
    }
    // Clearing the study goes back to the base case, again from the last solve.
    case.set_edits(vec![]).unwrap();
    assert!(case.solve(last).unwrap().converged);
}

#[test]
fn every_solve_starts_from_the_last_one_by_default() {
    use stannum_core::power::StartChoice;
    assert_eq!(SolveSettings::default().start, StartChoice::Last);
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let case = ws.current_mut().unwrap();
    assert!(case.solve(SolveSettings::default()).unwrap().iterations > 0);
    assert_eq!(case.solve(SolveSettings::default()).unwrap().iterations, 0);
    let flat = SolveSettings {
        start: StartChoice::Flat,
        ..SolveSettings::default()
    };
    assert!(case.solve(flat).unwrap().iterations > 0);
}

#[test]
fn the_default_tolerance_is_0_1_mva_on_any_base() {
    let settings = SolveSettings::default();
    assert_eq!(settings.tolerance_mva, 0.1);
    // Per unit on each case's own base, so the MW and Mvar stay the same.
    for (base, pu) in [(100.0, 0.001), (1000.0, 0.0001), (10.0, 0.01)] {
        let tol = settings.options(base).tolerance;
        assert!((tol - pu).abs() < 1e-15, "{base} MVA base: {tol} p.u.");
    }
    // A case with no base falls back to 100 MVA.
    assert_eq!(settings.options(0.0).tolerance, 0.001);
}

#[test]
fn dc_grid_file_attaches_to_a_case_and_solves() {
    let mut ws = Workspace::default();
    ws.open(&fixture("vsc_grids/two_area.m")).unwrap();
    let case = ws.current_mut().unwrap();
    let summary = case
        .load_dc_grids(&fixture("vsc_grids/two_area.dcgrid.json"))
        .unwrap();
    assert_eq!(summary.grids.len(), 2);
    assert_eq!(summary.grids[1].converters, 4);
    assert!(case.summary().dc_grids.is_some());

    let report = case.solve(SolveSettings::default()).unwrap();
    assert!(report.converged);
    let vsc = report.vsc_grids.as_ref().expect("VSC grid results");
    assert!(vsc.converged && vsc.iterations > 1);
    let [three, four] = &vsc.grids[..] else {
        panic!()
    };
    assert_eq!(three.converters[0].dc_control, "slack");
    assert_eq!(three.converters[1].ac_control, "voltage");
    assert!((three.converters[2].ps_mw + 150.0).abs() < 1e-6);
    assert!(four.converters.iter().all(|c| c.q_max_mvar.is_some()));
    // Losses: generation covers load, AC losses and the DC grids' losses,
    // to the solve's mismatch at each bus.
    let gap = report.total_generation_mw - report.total_load_mw - report.total_losses_mw;
    let allowed = report.buses.len() as f64 * report.residual_mva;
    assert!(gap.abs() <= allowed, "{gap} MW, allowed {allowed}");

    // A DC solve leaves the grids out; detaching them returns to plain AC.
    let dc = SolveSettings {
        model: ModelChoice::Dc,
        ..SolveSettings::default()
    };
    assert!(case.solve(dc).unwrap().vsc_grids.is_none());
    case.clear_dc_grids();
    assert!(
        case.solve(SolveSettings::default())
            .unwrap()
            .vsc_grids
            .is_none()
    );
}

#[test]
fn a_dc_grid_file_that_does_not_fit_the_case_is_refused() {
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let case = ws.current_mut().unwrap();
    let error = case
        .load_dc_grids(&fixture("vsc_grids/two_area.dcgrid.json"))
        .unwrap_err();
    assert!(
        error.message.contains("AC bus 105 is not in the case"),
        "{error}"
    );
    assert!(case.dc_grids.is_none());
}

#[test]
fn a_cancelled_solve_keeps_the_previous_results() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let case = ws.current_mut().unwrap();
    case.solve(SolveSettings::default()).unwrap();
    let err = case
        .solve_cancellable(
            SolveSettings::default(),
            Some(Arc::new(AtomicBool::new(true))),
        )
        .unwrap_err();
    assert_eq!(err, stannum_core::workspace::SOLVE_CANCELLED);
    assert_eq!(err.kind, stannum_core::ErrorKind::Cancelled);
    assert!(case.solved.is_some());
}

#[test]
fn the_report_sends_its_tables_as_columns() {
    let mut ws = Workspace::default();
    ws.open(&fixture("case9.m")).unwrap();
    let report = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings::default())
        .unwrap();
    let json = serde_json::to_value(&report).unwrap();
    let buses = &json["buses"];
    assert_eq!(buses["count"], 9);
    assert_eq!(buses["id"].as_array().unwrap().len(), 9);
    let branches = &json["branches"];
    assert_eq!(branches["count"], report.branches.len());
    for (i, row) in report.branches.iter().enumerate() {
        assert_eq!(branches["element"][i], row.element.as_str());
        let pf = branches["pf"][i].as_f64().unwrap();
        assert!((pf - row.pf).abs() <= 5e-5, "rounded to 0.1 kW");
        assert_eq!(
            branches["loading_pct"][i].is_null(),
            row.loading_pct.is_none()
        );
    }
    let vm = buses["vm"][0].as_f64().unwrap();
    assert!((vm - report.buses[0].vm).abs() <= 5e-7);
    assert_eq!(json["generators"]["online"][0], true);
}

#[test]
fn every_row_field_has_a_column() {
    // Columns come from the row's own fields (`stannum_core::table`), so a
    // field added to a row is always sent.
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus_zip.raw")).unwrap();
    let report = ws
        .current_mut()
        .unwrap()
        .solve(SolveSettings::default())
        .unwrap();
    let json = serde_json::to_value(&report).unwrap();
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.retain(|k| k != "count");
        k.sort();
        k
    };
    let rows = [
        ("buses", serde_json::to_value(&report.buses[0]).unwrap()),
        (
            "generators",
            serde_json::to_value(&report.generators[0]).unwrap(),
        ),
        (
            "branches",
            serde_json::to_value(&report.branches[0]).unwrap(),
        ),
    ];
    for (table, row) in rows {
        assert_eq!(keys(&json[table]), keys(&row), "{table}");
    }
    let parts = json["buses"]["load_parts"].as_array().unwrap();
    assert!(
        parts.iter().any(|p| !p.is_null()),
        "ZIP load parts are sent"
    );
}

#[test]
fn zip_loads_vary_with_voltage_and_show_their_parts() {
    let mut ws = Workspace::default();
    ws.open(&fixture("five_bus_zip.raw")).unwrap();
    let case = ws.current_mut().unwrap();
    let bus = |id: u32| {
        let power = case.power.as_ref().unwrap();
        power
            .case
            .buses
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .clone()
    };
    // Bus 2: 30 + j10 of each part; YQ -10 is inductive, so it consumes.
    let b = bus(2);
    assert_eq!((b.pd, b.pd_i, b.pd_z), (30.0, 30.0, 30.0));
    assert_eq!((b.qd, b.qd_i, b.qd_z), (10.0, 10.0, 10.0));
    assert!(
        !case
            .power
            .as_ref()
            .unwrap()
            .warnings
            .iter()
            .any(|w| w.message.contains("ZIP")),
        "ZIP loads are modelled, not approximated"
    );

    let tight = || SolveSettings {
        tolerance_mva: 1e-6,
        ..SolveSettings::default()
    };
    let report = case.solve(tight()).unwrap();
    assert!(report.converged);
    let row = report.buses.iter().find(|r| r.id == 2).unwrap();
    let parts = row.load_parts.as_ref().expect("bus 2 has ZIP parts");
    let vm = row.vm;
    assert!((vm - 1.0).abs() > 1e-3, "the bus sits off 1 p.u.");
    assert_eq!(parts.p[0], 30.0);
    assert!((parts.p[1] - 30.0 * vm).abs() < 1e-9);
    assert!((parts.p[2] - 30.0 * vm * vm).abs() < 1e-9);
    assert!((row.pd - parts.p.iter().sum::<f64>()).abs() < 1e-9);
    assert!((row.qd - parts.q.iter().sum::<f64>()).abs() < 1e-9);
    // A bus with only constant power has no parts to show.
    assert!(
        report
            .buses
            .iter()
            .find(|r| r.id == 5)
            .unwrap()
            .load_parts
            .is_none()
    );
    // Generation balances the load drawn at the solved voltages plus losses.
    let balance = report.total_generation_mw - report.total_load_mw - report.total_losses_mw;
    assert!(balance.abs() < 1e-4, "{balance}");

    // The inspector shows the parts as filed, for the load and its bus.
    let detail = case.element_detail("load:2:1").unwrap();
    let field = |d: &stannum_core::inspect::ElementDetail, label: &str, unit: &str| {
        d.data[0]
            .fields
            .iter()
            .find(|f| f.label == label && f.unit == unit)
            .map(|f| f.value.clone())
    };
    use stannum_core::inspect::Value::Number;
    assert_eq!(field(&detail, "Constant power", "MW"), Some(Number(30.0)));
    assert_eq!(
        field(&detail, "Constant current at 1 p.u.", "Mvar"),
        Some(Number(10.0))
    );
    assert_eq!(
        field(&detail, "Constant impedance at 1 p.u.", "Mvar"),
        Some(Number(10.0))
    );
    let detail = case.element_detail("bus:4").unwrap();
    assert_eq!(
        field(&detail, "Constant impedance at 1 p.u.", "MW"),
        Some(Number(20.0))
    );

    // A study load edit keeps each part's share.
    case.set_edits(vec![stannum_core::study::Edit::Load {
        bus: 2,
        mw: 45.0,
        mvar: 15.0,
    }])
    .unwrap();
    let edited = case.solve(tight()).unwrap();
    let parts = edited
        .buses
        .iter()
        .find(|r| r.id == 2)
        .unwrap()
        .load_parts
        .clone()
        .unwrap();
    assert!(parts.p[0] > 0.0 && parts.p[1] > 0.0 && parts.p[2] > 0.0);
}
