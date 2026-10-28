//! Area interchange control: area slack generation holds each area's net
//! export at its desired value.
use rs_power::{
    Area, AreaInterchange, AreaLoad, AreaSolution, Case, InterchangeMode, PowerFlowOptions,
    RawOptions, SolveStatus, parse_matpower, parse_raw_with_options, solve,
};

/// IEEE 30-bus case: area 1 holds the reference bus 1, area 2 has
/// generators at buses 13 and 23, area 3 at buses 22 and 27.
fn case30() -> Case {
    parse_matpower(include_str!("fixtures/case30.m")).unwrap()
}

fn area(number: u32, slack_bus: u32, desired_mw: f64) -> Area {
    Area {
        number,
        slack_bus: Some(slack_bus),
        desired_mw,
        tolerance_mw: 0.1,
        name: format!("A{number}"),
    }
}

fn options(mode: InterchangeMode, areas: Vec<Area>, loads: Vec<AreaLoad>) -> PowerFlowOptions {
    PowerFlowOptions {
        interchange: AreaInterchange { mode, areas, loads },
        ..Default::default()
    }
}

fn find(areas: &[AreaSolution], number: u32) -> &AreaSolution {
    areas.iter().find(|a| a.number == number).unwrap()
}

fn pg(case: &Case, bus: u32) -> f64 {
    case.generators
        .iter()
        .filter(|g| g.bus == bus)
        .map(|g| g.pg)
        .sum()
}

#[test]
fn every_area_reports_its_tie_line_interchange_with_control_off() {
    let case = case30();
    let free = solve(&case, &PowerFlowOptions::default()).unwrap();
    assert_eq!(free.status, SolveStatus::Converged);
    let numbers: Vec<u32> = free.areas.iter().map(|a| a.number).collect();
    assert_eq!(numbers, vec![1, 2, 3]);
    // Tie lines are metered at one end, so what one area exports another
    // imports.
    let total: f64 = free.areas.iter().map(|a| a.interchange_mw).sum();
    assert!(total.abs() < 1e-9, "{total}");
    assert!(free.areas.iter().all(|a| !a.controlled));
    assert_eq!(free.interchange_passes, 0);
    // Listing areas with control off changes nothing but the names.
    let listed = solve(
        &case,
        &options(
            InterchangeMode::Off,
            vec![area(2, 13, 0.0), area(3, 27, 0.0)],
            vec![],
        ),
    )
    .unwrap();
    assert_eq!(pg(&listed.case, 13), pg(&free.case, 13));
    let numbers: Vec<u32> = listed.areas.iter().map(|a| a.number).collect();
    assert_eq!(numbers, vec![2, 3, 1]);
    assert_eq!(find(&listed.areas, 2).name, "A2");
    assert!(
        (find(&listed.areas, 2).interchange_mw - find(&free.areas, 2).interchange_mw).abs() < 1e-9
    );
}

#[test]
fn area_slack_generation_moves_until_interchange_is_within_tolerance() {
    let case = case30();
    let free = solve(&case, &PowerFlowOptions::default()).unwrap();
    let start2 = find(&free.areas, 2).interchange_mw;
    let start3 = find(&free.areas, 3).interchange_mw;
    let (want2, want3) = (start2 + 10.0, start3 - 5.0);
    let result = solve(
        &case,
        &options(
            InterchangeMode::TieLines,
            vec![area(2, 13, want2), area(3, 27, want3)],
            vec![],
        ),
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!(result.interchange_passes >= 1);
    let a2 = find(&result.areas, 2);
    let a3 = find(&result.areas, 3);
    assert!(a2.controlled && a3.controlled);
    assert!((a2.interchange_mw - want2).abs() <= 0.1, "{a2:?}");
    assert!((a3.interchange_mw - want3).abs() <= 0.1, "{a3:?}");
    // The slack units supplied the change, about 10 and -5 MW give or
    // take the change in each area's losses.
    let moved2 = pg(&result.case, 13) - pg(&free.case, 13);
    let moved3 = pg(&result.case, 27) - pg(&free.case, 27);
    assert!((moved2 - 10.0).abs() < 1.0, "{moved2}");
    assert!((moved3 + 5.0).abs() < 1.0, "{moved3}");
    assert!((a2.slack_change_mw - moved2).abs() < 1e-9);
    // Other generators in the areas stay put; the reference area balances.
    assert_eq!(pg(&result.case, 23), pg(&free.case, 23));
    assert_eq!(pg(&result.case, 22), pg(&free.case, 22));
    let a1 = find(&result.areas, 1);
    assert!(!a1.controlled);
    let total: f64 = result.areas.iter().map(|a| a.interchange_mw).sum();
    assert!(total.abs() < 1e-9, "{total}");
}

#[test]
fn generators_at_the_area_slack_bus_share_the_change_by_machine_base() {
    let mut case = case30();
    let g = case.generators.iter().position(|g| g.bus == 13).unwrap();
    let mut twin = case.generators[g].clone();
    twin.mbase = 3.0 * case.generators[g].mbase;
    case.generators.push(twin);
    let free = solve(&case, &PowerFlowOptions::default()).unwrap();
    let want = find(&free.areas, 2).interchange_mw + 20.0;
    let result = solve(
        &case,
        &options(InterchangeMode::TieLines, vec![area(2, 13, want)], vec![]),
    )
    .unwrap();
    let last = case.generators.len() - 1;
    let d0 = result.case.generators[g].pg - free.case.generators[g].pg;
    let d1 = result.case.generators[last].pg - free.case.generators[last].pg;
    assert!((d1 - 3.0 * d0).abs() < 1e-6, "{d0} {d1}");
}

#[test]
fn loads_owned_by_another_area_count_only_in_tie_lines_and_loads_mode() {
    let case = case30();
    // Bus 3 (area 1) carries 2.4 MW; say that load belongs to area 2.
    let load = AreaLoad {
        bus: 3,
        area: 2,
        p: 2.4,
        p_i: 0.0,
        p_z: 0.0,
    };
    let free = solve(&case, &PowerFlowOptions::default()).unwrap();
    let ties = |r: &rs_power::PowerFlowResult, a| find(&r.areas, a).interchange_mw;
    let with_loads = solve(
        &case,
        &options(
            InterchangeMode::TieLinesAndLoads,
            vec![],
            vec![load.clone()],
        ),
    )
    .unwrap();
    // No areas listed: nothing moves, but area 2 now imports the load too.
    assert!((ties(&with_loads, 2) - (ties(&free, 2) - 2.4)).abs() < 1e-9);
    assert!((ties(&with_loads, 1) - (ties(&free, 1) + 2.4)).abs() < 1e-9);
    // Holding area 2 at its tie-line export plus 5 MW: by tie lines alone
    // the slack moves 5 MW, counting the load it moves 7.4 MW.
    let want = ties(&free, 2) + 5.0;
    let tie_only = solve(
        &case,
        &options(
            InterchangeMode::TieLines,
            vec![area(2, 13, want)],
            vec![load.clone()],
        ),
    )
    .unwrap();
    let both = solve(
        &case,
        &options(
            InterchangeMode::TieLinesAndLoads,
            vec![area(2, 13, want)],
            vec![load],
        ),
    )
    .unwrap();
    let moved = |r: &rs_power::PowerFlowResult| pg(&r.case, 13) - pg(&free.case, 13);
    assert!((moved(&tie_only) - 5.0).abs() < 0.5, "{}", moved(&tie_only));
    assert!((moved(&both) - 7.4).abs() < 0.5, "{}", moved(&both));
    assert!((ties(&both, 2) - want).abs() <= 0.1);
}

#[test]
fn an_area_holding_the_reference_bus_is_not_controlled() {
    let case = case30();
    let free = solve(&case, &PowerFlowOptions::default()).unwrap();
    // Area 1's slack is bus 2 (a PV bus), but the reference bus 1 is in
    // area 1 too, so its generation balances the island whatever bus 2 does.
    let result = solve(
        &case,
        &options(
            InterchangeMode::TieLines,
            vec![area(1, 2, find(&free.areas, 1).interchange_mw + 30.0)],
            vec![],
        ),
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(result.interchange_passes, 0);
    assert!(!find(&result.areas, 1).controlled);
    assert_eq!(pg(&result.case, 2), pg(&free.case, 2));
}

#[test]
fn interchange_control_works_with_reactive_limits_and_newton() {
    let case = case30();
    let free = solve(&case, &PowerFlowOptions::default()).unwrap();
    let want = find(&free.areas, 3).interchange_mw + 15.0;
    let mut opt = options(InterchangeMode::TieLines, vec![area(3, 22, want)], vec![]);
    opt.enforce_q_limits = true;
    let result = solve(&case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert!((find(&result.areas, 3).interchange_mw - want).abs() <= 0.1);
}

#[test]
fn dc_solves_ignore_interchange_control() {
    let case = case30();
    let mut opt = options(InterchangeMode::TieLines, vec![area(2, 13, 50.0)], vec![]);
    opt.model = rs_power::Model::Dc;
    let result = solve(&case, &opt).unwrap();
    assert!(result.areas.is_empty());
    assert_eq!(pg(&result.case, 13), pg(&case, 13));
}

fn psse_case3(zip_loads: bool) -> rs_power::RawImport {
    parse_raw_with_options(
        include_str!("fixtures/raw/t_psse_case3.raw"),
        &RawOptions {
            all_dc_lines: true,
            zip_loads,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn raw_import_reads_area_records() {
    let import = psse_case3(true);
    let areas: Vec<_> = import
        .areas
        .iter()
        .map(|a| {
            (
                a.number,
                a.slack_bus,
                a.desired_mw,
                a.tolerance_mw,
                a.name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        areas,
        vec![
            (1, Some(101), -2800.0, 10.0, "CENTRAL"),
            (2, Some(206), -1600.0, 10.0, "EAST"),
            (3, Some(301), 2900.0, 55.0, "CENTRAL_DC"),
            (4, Some(401), 300.0, 15.0, "EAST_COGEN1"),
            (5, Some(3011), 900.0, 10.0, "WEST"),
            (6, Some(402), 300.0, 20.0, "EAST_COGEN2"),
        ]
    );
    // Every load there sits in its bus's area.
    assert!(import.area_loads.is_empty());
}

#[test]
fn raw_import_lists_loads_assigned_to_another_area() {
    let text = include_str!("fixtures/raw/t_psse_case3.raw");
    // Move the first load record (bus 152, area 1) to area 2.
    let start = text.find("BEGIN LOAD DATA").unwrap();
    let line_start = start + text[start..].find('\n').unwrap() + 1;
    let line_end = line_start + text[line_start..].find('\n').unwrap();
    let record = &text[line_start..line_end];
    let fields: Vec<&str> = record.split(',').collect();
    let mut moved: Vec<String> = fields.iter().map(|s| s.to_string()).collect();
    moved[3] = "   2".into();
    let edited = format!(
        "{}{}{}",
        &text[..line_start],
        moved.join(","),
        &text[line_end..]
    );
    let bus: u32 = fields[0].trim().parse().unwrap();
    let pl: f64 = fields[5].trim().parse().unwrap();
    for zip in [true, false] {
        let import = parse_raw_with_options(
            &edited,
            &RawOptions {
                zip_loads: zip,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(import.area_loads.len(), 1);
        let l = &import.area_loads[0];
        assert_eq!((l.bus, l.area), (bus, 2));
        let vm = import.case.buses.iter().find(|b| b.id == bus).unwrap().vm;
        assert!(l.at(vm) >= pl - 1e-9, "{l:?}");
    }
}

#[test]
fn psse_case3_holds_its_controllable_areas_with_detailed_dc_lines() {
    let import = psse_case3(true);
    let opt = PowerFlowOptions {
        enforce_q_limits: true,
        tolerance: 1e-6,
        dc_model: rs_power::DcModel::Detailed {
            lines: import.lcc_lines.clone(),
            vsc_lines: import.vsc_lines.clone(),
            mtdc: import.mtdc.clone(),
            adjust_taps: true,
        },
        interchange: AreaInterchange {
            mode: InterchangeMode::TieLines,
            areas: import.areas.clone(),
            loads: import.area_loads.clone(),
        },
        ..Default::default()
    };
    let result = solve(&import.case, &opt).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    // Areas 3 to 6 each hold an island's reference bus; 1 and 2 are held.
    for a in &result.areas {
        let want = a.desired_mw.unwrap();
        if a.number <= 2 {
            assert!(a.controlled, "{a:?}");
            assert!((a.interchange_mw - want).abs() <= a.tolerance_mw, "{a:?}");
        } else {
            assert!(!a.controlled, "{a:?}");
        }
    }
    let total: f64 = result.areas.iter().map(|a| a.interchange_mw).sum();
    assert!(total.abs() < 1e-6, "{total}");
}
