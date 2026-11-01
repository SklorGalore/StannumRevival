//! Meshed and multi-terminal VSC DC grids solved in sequence with the AC
//! network (Beerten, Cole and Belmans, 2012).
use num_complex::Complex64 as C;
use rs_power::{
    AcControl, AcState, Branch, Bus, BusType, Case, ConverterLimits, ConverterLosses, DcControl,
    DcGrid, DcGridBus, DcGridLine, DcGridSettings, Generator, GridConverter, PowerFlowOptions,
    SolveStatus, load_matpower, solve_dc_grids,
};

/// Two asynchronous copies of case9 (buses 101–109 and 201–209) and an
/// offshore island: a 160 MW reference plant at 301 behind a line to 302.
fn three_ac_grids() -> Case {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/case9.m");
    let nine = load_matpower(path).unwrap();
    let mut case = Case {
        base_mva: 100.0,
        buses: Vec::new(),
        generators: Vec::new(),
        branches: Vec::new(),
        dcline: None,
        gencost: None,
        labels: Default::default(),
    };
    for area in [1u32, 2] {
        let shift = area * 100;
        for b in &nine.buses {
            let mut b = b.clone();
            b.id += shift;
            b.area = area;
            if area == 2 {
                b.pd *= 1.1;
                b.qd *= 1.1;
            }
            case.buses.push(b);
        }
        for g in &nine.generators {
            let mut g = g.clone();
            g.bus += shift;
            case.generators.push(g);
        }
        for br in &nine.branches {
            let mut br = br.clone();
            br.from += shift;
            br.to += shift;
            case.branches.push(br);
        }
    }
    let template = nine.buses[3].clone();
    for (id, kind) in [(301, BusType::Reference), (302, BusType::Pq)] {
        case.buses.push(Bus {
            id,
            kind,
            area: 3,
            base_kv: 220.0,
            ..template.clone()
        });
    }
    case.generators.push(Generator {
        bus: 301,
        pg: 160.0,
        vg: 1.02,
        qmax: 80.0,
        qmin: -80.0,
        ..nine.generators[0].clone()
    });
    case.branches.push(Branch {
        from: 301,
        to: 302,
        r: 0.002,
        x: 0.04,
        b: 0.1,
        ..nine.branches[1].clone()
    });
    case
}

fn station(name: &str, ac_bus: u32, dc_bus: u32, dc: DcControl, ac: AcControl) -> GridConverter {
    GridConverter {
        name: name.into(),
        ac_bus,
        dc_bus,
        in_service: true,
        dc_control: dc,
        ac_control: ac,
        transformer: C::new(0.0015, 0.1121),
        filter_b: 0.0,
        reactor: C::new(0.0001, 0.16428),
        losses: ConverterLosses {
            a_mw: 1.103,
            b_kv: 0.887,
            c_rectifier_ohm: 4.371,
            c_inverter_ohm: 2.885,
        },
        limits: ConverterLimits::default(),
    }
}

fn bus(number: u32) -> DcGridBus {
    DcGridBus {
        number,
        name: format!("DC {number}"),
    }
}

fn line(from: u32, to: u32, ohm: f64) -> DcGridLine {
    DcGridLine {
        from,
        to,
        name: format!("{from}-{to}"),
        resistance_ohm: ohm,
        in_service: true,
    }
}

/// A meshed three-terminal grid between both areas and the offshore
/// island, and a meshed four-terminal grid with droop control.
fn grids() -> Vec<DcGrid> {
    let q0 = AcControl::Reactive { q_mvar: 0.0 };
    let mut b = vec![
        station(
            "A1",
            107,
            4,
            DcControl::Droop {
                p_mw: 40.0,
                v_dc_pu: 1.0,
                mw_per_kv: 8.0,
            },
            q0,
        ),
        station(
            "A2",
            109,
            5,
            DcControl::Power { p_mw: -60.0 },
            AcControl::Reactive { q_mvar: 10.0 },
        ),
        station(
            "B1",
            207,
            6,
            DcControl::Droop {
                p_mw: 20.0,
                v_dc_pu: 1.0,
                mw_per_kv: 4.0,
            },
            q0,
        ),
        station("B2", 209, 7, DcControl::Power { p_mw: 30.0 }, q0),
    ];
    for c in &mut b {
        c.filter_b = 0.0887;
    }
    vec![
        DcGrid {
            name: "Three-terminal".into(),
            base_kv: 150.0,
            poles: 2,
            buses: vec![bus(1), bus(2), bus(3)],
            lines: vec![line(1, 2, 12.0), line(2, 3, 9.0), line(1, 3, 15.0)],
            converters: vec![
                station("North", 105, 1, DcControl::Slack { v_dc_pu: 1.0 }, q0),
                station(
                    "South",
                    205,
                    2,
                    DcControl::Power { p_mw: 75.0 },
                    AcControl::Voltage { v_pu: 1.0 },
                ),
                station("Offshore", 302, 3, DcControl::Power { p_mw: -150.0 }, q0),
            ],
        },
        DcGrid {
            name: "Four-terminal".into(),
            base_kv: 300.0,
            poles: 2,
            buses: vec![bus(4), bus(5), bus(6), bus(7)],
            lines: vec![
                line(4, 5, 20.0),
                line(5, 6, 25.0),
                line(6, 7, 20.0),
                line(7, 4, 30.0),
                line(4, 6, 40.0),
            ],
            converters: b,
        },
    ]
}

fn tight() -> PowerFlowOptions {
    PowerFlowOptions {
        tolerance: 1e-9,
        max_iterations: 20,
        ..Default::default()
    }
}

#[test]
fn several_meshed_grids_converge_and_balance() {
    let case = three_ac_grids();
    let grids = grids();
    let r = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    assert_eq!(r.status, SolveStatus::Converged);
    assert!(r.iterations <= 10, "{} iterations", r.iterations);
    // The solved case is the input again, without converter helpers.
    assert_eq!(r.ac.case.generators.len(), case.generators.len());
    assert_eq!(
        r.ac.case.buses[case.buses.iter().position(|b| b.id == 205).unwrap()].kind,
        BusType::Pq
    );
    let vm = |id: u32| r.ac.case.buses.iter().find(|b| b.id == id).unwrap().vm;

    for (grid, solved) in grids.iter().zip(&r.grids) {
        // DC power balance: what converters put in is the line losses.
        let into_dc: f64 = solved.converters.iter().map(|c| c.pdc_mw).sum();
        assert!(
            (into_dc - solved.line_loss_mw).abs() < 1e-5,
            "{}: {into_dc} vs {}",
            grid.name,
            solved.line_loss_mw
        );
        assert!(solved.line_loss_mw > 0.0 && solved.converter_loss_mw > 0.0);
        // Line flows by hand from the bus voltages.
        for (l, s) in grid.lines.iter().zip(&solved.lines) {
            let v = |n: u32| solved.buses.iter().find(|b| b.number == n).unwrap().kv;
            let current = (v(l.from) - v(l.to)) / l.resistance_ohm;
            assert!((s.current_ka - current).abs() < 1e-9);
            assert!((s.loss_mw - 2.0 * current * current * l.resistance_ohm).abs() < 1e-6);
        }
        for (c, s) in grid.converters.iter().zip(&solved.converters) {
            // Station balance: AC injection, losses and DC power.
            assert!((s.pc_mw + s.loss_mw + s.pdc_mw).abs() < 1e-9);
            match c.dc_control {
                DcControl::Power { p_mw } => assert!((s.ps_mw - p_mw).abs() < 1e-9),
                DcControl::Slack { v_dc_pu } => assert!((s.udc_pu - v_dc_pu).abs() < 1e-12),
                DcControl::Droop {
                    p_mw,
                    v_dc_pu,
                    mw_per_kv,
                } => {
                    let law = p_mw + mw_per_kv * grid.base_kv * (s.udc_pu - v_dc_pu);
                    assert!(
                        (-s.pdc_mw - law).abs() < 1e-5,
                        "{}: {} vs {law}",
                        c.name,
                        -s.pdc_mw
                    );
                }
            }
            match c.ac_control {
                AcControl::Reactive { q_mvar } => {
                    assert_eq!(s.ac_state, AcState::Reactive);
                    assert!((s.qs_mvar - q_mvar).abs() < 1e-9);
                }
                AcControl::Voltage { v_pu } => {
                    assert_eq!(s.ac_state, AcState::Voltage);
                    assert!((vm(c.ac_bus) - v_pu).abs() < 1e-9);
                    assert!((s.us_pu - v_pu).abs() < 1e-9);
                }
            }
        }
    }
    // The offshore plant, the island's reference, supplies what the
    // converter takes (150 MW) plus the island's line losses.
    let offshore = r.ac.case.generators.iter().find(|g| g.bus == 301).unwrap();
    assert!(
        offshore.pg > 150.0 && offshore.pg < 151.0,
        "{}",
        offshore.pg
    );
    // The slack converter sends the three-terminal grid's surplus to area 1.
    let north = &r.grids[0].converters[0];
    let rest: f64 = r.grids[0].converters[1..].iter().map(|c| c.pdc_mw).sum();
    assert!(north.pdc_mw < 0.0 && (north.pdc_mw + rest - r.grids[0].line_loss_mw).abs() < 1e-5);
    assert!(north.ps_mw > 60.0 && north.ps_mw < 75.0, "{}", north.ps_mw);
}

#[test]
fn a_voltage_controlling_converter_stops_at_its_current_limit() {
    let case = three_ac_grids();
    let mut grids = grids();
    let free = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    let south = &free.grids[0].converters[1];
    // A current limit just above what P alone needs, below what holding
    // 1.0 p.u. needs.
    let p_only = (south.ps_mw / 100.0).abs() / south.us_pu;
    assert!(
        south.current_pu > p_only * 1.01,
        "{} {p_only}",
        south.current_pu
    );
    let limit = 0.5 * (south.current_pu + p_only);
    grids[0].converters[1].limits.current_max_pu = limit;
    let r = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    assert_eq!(r.status, SolveStatus::Converged);
    let s = &r.grids[0].converters[1];
    assert_eq!(s.ac_state, AcState::ReactiveLimit);
    assert!(
        (s.current_pu - limit).abs() < 1e-6,
        "{} vs {limit}",
        s.current_pu
    );
    assert!(!s.outside_limits);
    // It no longer holds its bus at 1.0 p.u.
    assert!((s.us_pu - 1.0).abs() > 1e-4);
    // Unenforced, the same limit is only reported.
    let r = solve_dc_grids(
        &case,
        &tight(),
        &grids,
        &DcGridSettings {
            enforce_limits: false,
            ..Default::default()
        },
    )
    .unwrap();
    let s = &r.grids[0].converters[1];
    assert_eq!(s.ac_state, AcState::Voltage);
    assert!(s.outside_limits);
}

#[test]
fn a_power_order_beyond_the_current_limit_is_reduced() {
    let case = three_ac_grids();
    let mut grids = grids();
    grids[0].converters[2].limits.current_max_pu = 1.2;
    let r = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    assert_eq!(r.status, SolveStatus::Converged);
    let s = &r.grids[0].converters[2];
    assert!(s.p_limited);
    assert!(s.ps_mw > -150.0 && s.ps_mw < -115.0, "{}", s.ps_mw);
    assert!((s.current_pu - 1.2).abs() < 1e-6, "{}", s.current_pu);
}

#[test]
fn reactive_set_points_stay_within_the_range() {
    let case = three_ac_grids();
    let mut grids = grids();
    let c = &mut grids[1].converters[1];
    c.ac_control = AcControl::Reactive { q_mvar: 200.0 };
    c.limits.uc_max_pu = 1.1;
    let r = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    assert_eq!(r.status, SolveStatus::Converged);
    let s = &r.grids[1].converters[1];
    assert_eq!(s.ac_state, AcState::ReactiveLimit);
    assert!((s.uc_pu - 1.1).abs() < 1e-6, "{}", s.uc_pu);
    assert!((s.qs_mvar - s.q_max_mvar).abs() < 1e-6);
}

#[test]
fn out_of_service_parts_and_bad_data() {
    let case = three_ac_grids();
    let mut grids = grids();
    // An out-of-service converter is reported idle; the rest still solve.
    grids[1].converters[3].in_service = false;
    grids[1].lines[4].in_service = false;
    let r = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    assert_eq!(r.status, SolveStatus::Converged);
    assert!(!r.grids[1].converters[3].in_service && r.grids[1].converters[3].ps_mw == 0.0);
    assert!(!r.grids[1].lines[4].in_service && r.grids[1].lines[4].loss_mw == 0.0);

    let fails = |edit: &dyn Fn(&mut Vec<DcGrid>)| {
        let mut g = self::grids();
        edit(&mut g);
        solve_dc_grids(&case, &tight(), &g, &DcGridSettings::default())
            .unwrap_err()
            .to_string()
    };
    // No slack or droop converter.
    let e = fails(&|g| g[0].converters[0].dc_control = DcControl::Power { p_mw: 0.0 });
    assert!(e.contains("no slack or droop"), "{e}");
    // Voltage control at a generator bus.
    let e = fails(&|g| {
        g[0].converters[0].ac_bus = 103;
        g[0].converters[0].ac_control = AcControl::Voltage { v_pu: 1.0 };
    });
    assert!(e.contains("AC voltage control"), "{e}");
    let e = fails(&|g| g[0].converters[0].ac_bus = 999);
    assert!(e.contains("not in the case"), "{e}");
    let e = fails(&|g| g[0].lines[0].resistance_ohm = 0.0);
    assert!(e.contains("positive resistance"), "{e}");
    let e = fails(&|g| g[1].poles = 3);
    assert!(e.contains("poles"), "{e}");
}

#[test]
fn the_linear_model_is_refused() {
    let opt = PowerFlowOptions {
        model: rs_power::Model::Dc,
        ..Default::default()
    };
    assert!(
        solve_dc_grids(
            &three_ac_grids(),
            &opt,
            &grids(),
            &DcGridSettings::default()
        )
        .is_err()
    );
}

#[test]
fn works_with_loose_tolerances_and_generator_limits() {
    let case = three_ac_grids();
    let grids = grids();
    let tight = solve_dc_grids(&case, &tight(), &grids, &DcGridSettings::default()).unwrap();
    for enforce_q_limits in [false, true] {
        // A loose 2 MVA on a 100 MVA base (Stannum's default before 1.1).
        let opt = PowerFlowOptions {
            tolerance: 0.02,
            enforce_q_limits,
            ..Default::default()
        };
        let r = solve_dc_grids(&case, &opt, &grids, &DcGridSettings::default()).unwrap();
        assert_eq!(r.status, SolveStatus::Converged);
        for (a, b) in r.grids.iter().zip(&tight.grids) {
            for (x, y) in a.converters.iter().zip(&b.converters) {
                assert!((x.ps_mw - y.ps_mw).abs() < 2.0, "{} {}", x.ps_mw, y.ps_mw);
            }
        }
    }
}
