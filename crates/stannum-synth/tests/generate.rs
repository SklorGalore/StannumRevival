//! End-to-end checks: a generated case meets the paper's validation
//! criteria, solves, and is the same for the same seed.

use stannum_synth::{Config, generate};

fn small(seed: u64) -> Config {
    let mut c = Config::for_buses(300);
    c.seed = seed;
    c
}

#[test]
fn a_small_case_meets_the_criteria_and_solves() {
    let s = generate(&small(7));
    let r = &s.report;
    assert!((250..=350).contains(&r.buses), "{} buses", r.buses);

    // Structure (paper Section III, Table VI).
    assert!(r.structure.connected);
    assert_eq!(
        r.structure.cut_substations, 0,
        "survives any one substation outage"
    );
    for l in &r.structure.levels {
        assert!(l.connected, "{} kV is connected", l.kv);
        assert!(
            (1.15..=1.45).contains(&l.m_per_n),
            "{} kV m/n {}",
            l.kv,
            l.m_per_n
        );
        assert!(
            (l.mst - 0.50).abs() < 0.08,
            "{} kV MST share {}",
            l.kv,
            l.mst
        );
        assert!(
            (l.delaunay - 0.70).abs() < 0.08,
            "{} kV Delaunay share {}",
            l.kv,
            l.delaunay
        );
        assert!(
            l.four + l.five_plus < 0.02,
            "{} kV lines beyond 3 Delaunay steps",
            l.kv
        );
        assert!(l.crossings < 0.20, "{} kV crossings {}", l.kv, l.crossings);
    }

    // AC power flow (Section V): converged, voltages in band, loading near
    // the 85% limit at most.
    let ac = &r.ac;
    assert!(ac.converged);
    assert!(
        ac.vmin >= 0.96 && ac.vmax <= 1.07,
        "{}-{}",
        ac.vmin,
        ac.vmax
    );
    assert!(
        ac.buses_outside_band * 50 <= r.buses,
        "{} buses outside the band",
        ac.buses_outside_band
    );
    assert!(ac.max_loading < 1.05, "max loading {}", ac.max_loading);
    assert!(
        (ac.generation_mw - ac.load_mw) / ac.load_mw < 0.06,
        "losses look wrong"
    );

    // The written case reads back and solves from its stored solution.
    let text = rs_power::format_matpower(&s.case).unwrap();
    let back = rs_power::parse_matpower(&text).unwrap();
    let solved = rs_power::solve(
        &back,
        &rs_power::PowerFlowOptions {
            enforce_q_limits: true,
            tolerance: 1e-5,
            max_iterations: 20,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(solved.status, rs_power::SolveStatus::Converged);

    // Every bus has coordinates.
    assert_eq!(s.coordinates_csv().lines().count(), r.buses + 1);
}

#[test]
fn the_same_seed_builds_the_same_case() {
    let a = rs_power::format_matpower(&generate(&small(3)).case).unwrap();
    let b = rs_power::format_matpower(&generate(&small(3)).case).unwrap();
    let c = rs_power::format_matpower(&generate(&small(4)).case).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// About 80,000 buses: `cargo test --release -p stannum-synth -- --ignored`.
#[test]
#[ignore = "large: several minutes in release"]
fn an_80k_bus_case() {
    let s = generate(&Config::for_buses(80_000));
    let r = &s.report;
    eprintln!("{}", r.markdown());
    assert!((72_000..=88_000).contains(&r.buses));
    assert!(r.structure.connected);
    assert!(r.ac.converged);
}
