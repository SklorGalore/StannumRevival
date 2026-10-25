use rs_power::{
    BusType, Error, Generator, Initialization, Model, PowerFlowOptions, SolveStatus,
    parse_matpower, solve,
};

fn case() -> rs_power::Case {
    parse_matpower(include_str!("fixtures/case9.m")).unwrap()
}

#[test]
fn preserves_input_and_row_order() {
    let mut c = case();
    c.buses.reverse();
    c.generators.reverse();
    c.branches.reverse();
    let before = format!("{c:?}");
    let result = solve(&c, &PowerFlowOptions::default()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(before, format!("{c:?}"));
    assert_eq!(
        c.buses.iter().map(|b| b.id).collect::<Vec<_>>(),
        result.case.buses.iter().map(|b| b.id).collect::<Vec<_>>()
    );
}

#[test]
fn dc_lines_match_paired_terminal_generators() {
    for model in [Model::Ac, Model::Dc] {
        let mut with_line = case();
        let line = vec![
            5.0, 7.0, 1.0, 10.0, 99.0, 2.0, 3.0, 1.0, 1.0, -100.0, 100.0, -100.0, 100.0, -100.0,
            100.0, 1.0, 0.0,
        ];
        with_line.dcline = Some(vec![line.clone()]);
        let mut equivalent = with_line.clone();
        equivalent.dcline = None;
        for bus in &mut equivalent.buses {
            if bus.id == 5 || bus.id == 7 {
                bus.kind = BusType::Pv;
            }
        }
        equivalent.generators.extend([
            Generator {
                bus: 5,
                pg: -10.0,
                qg: 2.0,
                qmax: 100.0,
                qmin: -100.0,
                vg: 1.0,
                mbase: equivalent.base_mva,
                online: true,
                pmax: 100.0,
                pmin: -100.0,
                extra: Vec::new(),
            },
            Generator {
                bus: 7,
                pg: 9.0,
                qg: 3.0,
                qmax: 100.0,
                qmin: -100.0,
                vg: 1.0,
                mbase: equivalent.base_mva,
                online: true,
                pmax: 100.0,
                pmin: -100.0,
                extra: Vec::new(),
            },
        ]);
        let options = PowerFlowOptions {
            model,
            ..Default::default()
        };
        let actual = solve(&with_line, &options).unwrap();
        let expected = solve(&equivalent, &options).unwrap();
        assert_eq!(actual.status, SolveStatus::Converged);
        assert_eq!(actual.case.generators.len(), with_line.generators.len());
        for (actual, expected) in actual.case.buses.iter().zip(&expected.case.buses) {
            assert!((actual.vm - expected.vm).abs() < 1e-10);
            assert!((actual.va - expected.va).abs() < 1e-10);
        }
        assert_eq!(actual.branch_flows, expected.branch_flows);
        let solved = &actual.case.dcline.as_ref().unwrap()[0];
        assert_eq!(&solved[3..5], &[10.0, 9.0]);
        assert_eq!(solved[5], expected.case.generators[3].qg);
        assert_eq!(solved[6], expected.case.generators[4].qg);
        assert_eq!(&solved[7..9], &[1.0, 1.0]);
    }
}

#[test]
fn dc_line_end_with_zero_width_q_range_injects_that_q() {
    // The sending end has QMIN = QMAX = 12 MVAr (a VSC in power factor
    // control); it should inject exactly that, with or without Q limits,
    // leaving bus 5 a PQ bus. The receiving end still holds 1.0 p.u.
    for enforce_q_limits in [false, true] {
        let mut c = case();
        c.dcline = Some(vec![vec![
            5.0, 7.0, 1.0, 10.0, 10.0, 0.0, 0.0, 1.0, 1.0, 0.0, 20.0, 12.0, 12.0, -100.0, 100.0,
            0.0, 0.0,
        ]]);
        let options = PowerFlowOptions {
            enforce_q_limits,
            ..Default::default()
        };
        let result = solve(&c, &options).unwrap();
        assert_eq!(result.status, SolveStatus::Converged);
        let solved = &result.case.dcline.as_ref().unwrap()[0];
        assert_eq!(solved[5], 12.0);
        let bus = |id| result.case.buses.iter().find(|b| b.id == id).unwrap();
        assert_eq!(bus(5).kind, BusType::Pq);
        assert!((bus(7).vm - 1.0).abs() < 1e-9);
        assert!((bus(5).vm - 1.0).abs() > 1e-6, "bus 5 is not held at VF");
    }
}

#[test]
fn offline_dc_lines_are_preserved_and_invalid_lines_are_rejected() {
    let mut c = case();
    let line = vec![
        5.0, 7.0, 0.0, 10.0, 9.0, 2.0, 3.0, 1.0, 1.0, -100.0, 100.0, -100.0, 100.0, -100.0, 100.0,
        1.0, 0.0,
    ];
    c.dcline = Some(vec![line.clone()]);
    let result = solve(&c, &PowerFlowOptions::default()).unwrap();
    assert_eq!(result.case.dcline.unwrap()[0], line);

    for invalid in [
        vec![0.0; 16],
        vec![
            5.0, 999.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
    ] {
        let mut c = case();
        c.dcline = Some(vec![invalid]);
        assert!(matches!(
            solve(&c, &PowerFlowOptions::default()),
            Err(Error::InvalidCase(_))
        ));
    }
}

#[test]
fn nonconvergence_returns_last_state() {
    let result = solve(
        &case(),
        &PowerFlowOptions {
            max_iterations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::IterationLimit);
    assert_eq!(result.iterations, 0);
    assert!(result.residual > 1e-8);
    assert!(result.case.buses.iter().all(|b| b.vm.is_finite()));
}

#[test]
fn flat_start_ignores_supplied_iterate_and_preserves_reference_angle() {
    let expected = solve(&case(), &PowerFlowOptions::default()).unwrap();
    let mut perturbed = case();
    for (i, bus) in perturbed.buses.iter_mut().enumerate() {
        bus.vm = 0.7 + i as f64 * 0.02;
        bus.va = 40.0 - i as f64 * 9.0;
    }
    perturbed.buses[0].va = 7.0;
    let actual = solve(
        &perturbed,
        &PowerFlowOptions {
            initialization: Initialization::Flat,
            max_iterations: 30,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(actual.status, SolveStatus::Converged);
    for (actual, expected) in actual.case.buses.iter().zip(&expected.case.buses) {
        assert!((actual.vm - expected.vm).abs() < 1e-8);
        assert!((actual.va - expected.va - 7.0).abs() < 1e-7);
    }
}

#[test]
fn rejects_invalid_input_and_topology() {
    let check = |c| assert!(solve(&c, &PowerFlowOptions::default()).is_err());
    let mut c = case();
    c.base_mva = 0.0;
    check(c);
    let mut c = case();
    c.buses[1].id = 1;
    check(c);
    let mut c = case();
    c.generators[0].bus = 999;
    check(c);
    let mut c = case();
    c.branches[0].to = 999;
    check(c);
    let mut c = case();
    c.buses[0].vm = f64::NAN;
    check(c);
    let mut c = case();
    c.generators[0].online = false;
    check(c);
    let mut c = case();
    c.buses[1].kind = BusType::Reference;
    check(c);
    let mut c = case();
    c.branches[0].online = false;
    check(c);
    let mut c = case();
    c.branches[0].r = 0.0;
    c.branches[0].x = 0.0;
    check(c);
    let mut c = case();
    c.generators[0].qmin = 301.0;
    check(c);
    let mut c = case();
    c.buses.clear();
    check(c);
}

#[test]
fn explicitly_isolated_equipment_is_excluded() {
    let mut c = case();
    let mut b = c.buses[0].clone();
    b.id = 100;
    b.kind = BusType::Isolated;
    c.buses.push(b);
    let mut g = c.generators[0].clone();
    g.bus = 100;
    c.generators.push(g);
    let mut br = c.branches[0].clone();
    br.to = 100;
    c.branches.push(br);
    let result = solve(&c, &PowerFlowOptions::default()).unwrap();
    assert_eq!(result.status, SolveStatus::Converged);
    assert_eq!(result.case.buses.last().unwrap().kind, BusType::Isolated);
    assert_eq!(result.case.generators.last().unwrap().pg, 0.0);
    assert_eq!(*result.branch_flows.last().unwrap(), [0.0; 4]);
}

#[test]
fn nodal_power_balance_includes_shunts_and_losses() {
    let c = case();
    let r = solve(&c, &PowerFlowOptions::default()).unwrap();
    for b in &r.case.buses {
        let pg: f64 = r
            .case
            .generators
            .iter()
            .filter(|g| g.bus == b.id)
            .map(|g| g.pg)
            .sum();
        let qg: f64 = r
            .case
            .generators
            .iter()
            .filter(|g| g.bus == b.id)
            .map(|g| g.qg)
            .sum();
        let (mut p, mut q) = (0.0, 0.0);
        for (br, f) in r.case.branches.iter().zip(&r.branch_flows) {
            if br.from == b.id {
                p += f[0];
                q += f[1];
            }
            if br.to == b.id {
                p += f[2];
                q += f[3];
            }
        }
        assert!((pg - b.pd - b.gs * b.vm * b.vm - p).abs() < 1e-5);
        assert!((qg - b.qd + b.bs * b.vm * b.vm - q).abs() < 1e-5);
    }
}

#[test]
fn parser_rejects_execution_and_bad_syntax() {
    let original = include_str!("fixtures/case9.m");
    for bad in ["system('whoami');", "mpc.bus(1,1) = 5;", "mpc.foo = [1];"] {
        assert!(matches!(
            parse_matpower(&format!("{original}\n{bad}")),
            Err(Error::Parse { .. })
        ));
    }
    for (from, to) in [
        ("100;", "50+50;"),
        ("mpc.bus = [", "mpc.bus = [1+2;"),
        ("mpc.bus = [", "mpc.bus = [sqrt(4);"),
        ("'2'", "'1'"),
    ] {
        assert!(
            parse_matpower(&original.replacen(from, to, 1)).is_err(),
            "accepted {to}"
        );
    }
    assert!(parse_matpower("mpc.version = '2';\nmpc.bus = [1 2").is_err());
    assert!(parse_matpower("mpc.version = '2';\nmpc.bus = [1 2; 3];").is_err());
}

#[test]
fn parser_handles_comments_continuations_and_exponents() {
    let original = include_str!("fixtures/case9.m");
    let text = original
        .replace("function mpc = case9", "function mpc = case9()")
        .replace("100;", "1e2; % base")
        .replace("1\t3\t0", "1, ... continued\n3, +0");
    let c = parse_matpower(&format!("{text}\nend\n")).unwrap();
    assert_eq!(c.base_mva, 100.0);
    assert_eq!(c.buses[0].kind, BusType::Reference);
}

#[test]
fn dc_rejects_zero_reactance_and_reactive_options() {
    let mut c = case();
    c.branches[0].r = 0.1;
    c.branches[0].x = 0.0;
    assert!(
        solve(
            &c,
            &PowerFlowOptions {
                model: Model::Dc,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        solve(
            &case(),
            &PowerFlowOptions {
                model: Model::Dc,
                enforce_q_limits: true,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn mixed_limits_preserve_last_solved_state_when_no_slack_remains() {
    let mut c = case();
    c.generators[0].qmax = -100.0;
    c.generators[1].qmax = -100.0;
    c.generators[2].qmin = 100.0;
    let baseline = solve(&c, &PowerFlowOptions::default()).unwrap();
    let result = solve(
        &c,
        &PowerFlowOptions {
            enforce_q_limits: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, SolveStatus::ReactiveLimitsInfeasible);
    assert_eq!(result.q_limit_passes, 0);
    assert_eq!(result.case.buses[0].kind, BusType::Reference);
    for (a, b) in result.case.generators.iter().zip(&baseline.case.generators) {
        assert_eq!(a.qg, b.qg);
    }
}

#[test]
fn one_bus_network_needs_no_factorization() {
    let mut c = case();
    c.buses.truncate(1);
    c.generators.truncate(1);
    c.branches.clear();
    c.buses[0].pd = 10.0;
    c.buses[0].qd = 5.0;
    for model in [Model::Ac, Model::Dc] {
        let r = solve(
            &c,
            &PowerFlowOptions {
                model,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.status, SolveStatus::Converged);
        assert!((r.case.generators[0].pg - 10.0).abs() < 1e-10);
    }
}

fn two_island_case() -> rs_power::Case {
    let mut combined = case();
    let mut second = case();
    for bus in &mut second.buses {
        bus.id += 100;
    }
    for generator in &mut second.generators {
        generator.bus += 100;
    }
    for branch in &mut second.branches {
        branch.from += 100;
        branch.to += 100;
    }
    combined.buses.extend(second.buses);
    combined.generators.extend(second.generators);
    combined.branches.extend(second.branches);
    combined
}

#[test]
fn solves_multiple_islands_and_preserves_combined_order() {
    let single = case();
    let combined = two_island_case();
    for model in [Model::Ac, Model::Dc] {
        let options = PowerFlowOptions {
            model,
            ..Default::default()
        };
        let expected = solve(&single, &options).unwrap();
        let actual = solve(&combined, &options).unwrap();
        assert_eq!(actual.status, SolveStatus::Converged);
        assert_eq!(actual.iterations, expected.iterations);

        for island in 0..2 {
            for (i, bus) in expected.case.buses.iter().enumerate() {
                let actual_bus = &actual.case.buses[island * single.buses.len() + i];
                assert!((actual_bus.vm - bus.vm).abs() < 1e-10);
                assert!((actual_bus.va - bus.va).abs() < 1e-10);
            }
            for (i, generator) in expected.case.generators.iter().enumerate() {
                let actual_generator =
                    &actual.case.generators[island * single.generators.len() + i];
                assert!((actual_generator.pg - generator.pg).abs() < 1e-8);
                assert!((actual_generator.qg - generator.qg).abs() < 1e-8);
            }
            for (i, flows) in expected.branch_flows.iter().enumerate() {
                let actual_flows = &actual.branch_flows[island * single.branches.len() + i];
                for j in 0..4 {
                    assert!((actual_flows[j] - flows[j]).abs() < 1e-8);
                }
            }
        }
    }
}

#[test]
fn requires_one_reference_generator_per_active_island() {
    let mut missing = two_island_case();
    missing.buses[9].kind = BusType::Pq;
    assert!(matches!(
        solve(&missing, &PowerFlowOptions::default()),
        Err(Error::Unsupported(message)) if message.contains("each active island")
    ));

    let mut duplicate = two_island_case();
    duplicate.buses[10].kind = BusType::Reference;
    assert!(matches!(
        solve(&duplicate, &PowerFlowOptions::default()),
        Err(Error::Unsupported(message)) if message.contains("each active island")
    ));
}
