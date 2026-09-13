use rs_power::{BusType, Error, Model, PowerFlowOptions, SolveStatus, parse_matpower, solve};

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
