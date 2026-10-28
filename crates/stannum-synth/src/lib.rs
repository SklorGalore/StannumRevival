//! Synthetic power system cases, built with the method of
//! A. B. Birchfield, T. Xu, K. M. Gegner, K. S. Shetye and T. J. Overbye,
//! "Grid Structural Characteristics as Validation Criteria for Synthetic
//! Networks," *IEEE Trans. Power Systems* 32(4), 2017.
//!
//! The pipeline follows the paper's Fig. 4:
//!
//! 1. [`substations`]: population points (a synthetic census) clustered
//!    into load substations; power plants assigned to substations with load
//!    or clustered into generation-only substations (Tables I and II).
//! 2. [`lines`]: transmission lines at each voltage level, chosen from
//!    Delaunay neighbours by the penalties of Table V with a dc power flow
//!    in the loop.
//! 3. [`build`]: buses, transformers, conductors sized from dc flows
//!    (eqs. 2-5), and an ac solve with shunts added where voltages leave
//!    0.97-1.05 pu.
//! 4. [`validate`]: the paper's structural statistics (Table VI), which
//!    also work on any other case with substation coordinates.
//!
//! The paper places load from census postal codes and plants from EIA-860.
//! Here both are synthesized from a seed, so a case of any size can be built
//! anywhere, including in CI, with nothing to download.

pub mod build;
pub mod cluster;
pub mod config;
pub mod dcflow;
pub mod geo;
pub mod graph;
pub mod lines;
pub mod params;
pub mod rng;
pub mod substations;
pub mod svg;
pub mod validate;

pub use config::Config;

use serde::Serialize;
use std::time::Instant;

/// A generated case and everything measured while building it.
pub struct Synthetic {
    pub config: Config,
    pub case: rs_power::Case,
    pub placement: substations::Placement,
    pub lines: Vec<lines::Line>,
    pub bus_substation: Vec<usize>,
    pub report: Report,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub name: String,
    pub seed: u64,
    pub buses: usize,
    pub generators: usize,
    pub branches: usize,
    pub lines: usize,
    /// Lines added after the main placement rounds to join split levels and
    /// remove cut substations.
    pub repair_lines: usize,
    pub transformers: usize,
    pub substations: SubstationCounts,
    pub plants: usize,
    pub capacity_mw: f64,
    pub placement: Vec<lines::PlacementSummary>,
    pub structure: validate::Stats,
    pub ac: build::AcSummary,
    /// Seconds spent in each stage.
    pub seconds: Vec<(String, f64)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SubstationCounts {
    pub total: usize,
    pub load: usize,
    pub generation: usize,
    pub both: usize,
    pub by_level: Vec<(f64, usize)>,
}

/// Build a case from `config`.
pub fn generate(config: &Config) -> Synthetic {
    let mut rng = rng::Rng::new(config.seed);
    let mut seconds = Vec::new();
    let mut clock = Instant::now();
    let mut lap = |name: &str, seconds: &mut Vec<(String, f64)>| {
        let s = clock.elapsed().as_secs_f64();
        log::info!("{name}: {s:.1} s");
        seconds.push((name.to_string(), (s * 100.0).round() / 100.0));
        clock = Instant::now();
    };

    let placement = substations::place(config, &mut rng.fork(10));
    lap("substations", &mut seconds);
    let network = lines::place(config, &placement);
    lap("lines", &mut seconds);
    let built = build::build(config, &placement, &network.lines, &mut rng.fork(20));
    lap("case and ac solve", &mut seconds);
    let spans: Vec<validate::Span> = network
        .lines
        .iter()
        .map(|l| validate::Span {
            kv: config.levels[l.level].kv,
            a: l.a,
            b: l.b,
        })
        .collect();
    let at: Vec<geo::LatLon> = placement.substations.iter().map(|s| s.at).collect();
    let levels: Vec<Vec<f64>> = placement
        .substations
        .iter()
        .map(|s| s.levels.iter().map(|&l| config.levels[l].kv).collect())
        .collect();
    let structure = validate::measure(&at, &spans, &levels);
    lap("validation", &mut seconds);

    let subs = &placement.substations;
    let count = |k: substations::Kind| subs.iter().filter(|s| s.kind == k).count();
    let role_count = |r: build::Role| built.roles.iter().filter(|&&x| x == r).count();
    let report = Report {
        name: config.name.clone(),
        seed: config.seed,
        buses: built.case.buses.len(),
        generators: built.case.generators.len(),
        branches: built.case.branches.len(),
        lines: role_count(build::Role::Line),
        repair_lines: network.repair_lines,
        transformers: role_count(build::Role::Transformer) + role_count(build::Role::StepUp),
        substations: SubstationCounts {
            total: subs.len(),
            load: count(substations::Kind::Load),
            generation: count(substations::Kind::Generation),
            both: count(substations::Kind::Both),
            by_level: config
                .levels
                .iter()
                .enumerate()
                .map(|(i, l)| (l.kv, subs.iter().filter(|s| s.levels.contains(&i)).count()))
                .collect(),
        },
        plants: placement.plants.len(),
        capacity_mw: placement.plants.iter().map(|p| p.mw).sum::<f64>().round(),
        placement: network.summary,
        structure,
        ac: built.ac,
        seconds,
    };
    Synthetic {
        config: config.clone(),
        case: built.case,
        placement,
        lines: network.lines,
        bus_substation: built.bus_substation,
        report,
    }
}

impl Synthetic {
    /// Bus, substation, latitude and longitude, one bus per row.
    pub fn coordinates_csv(&self) -> String {
        let mut s = String::from("bus,substation,lat,lon\n");
        for (b, &sub) in self.case.buses.iter().zip(&self.bus_substation) {
            let at = self.placement.substations[sub].at;
            s.push_str(&format!(
                "{},{},{:.6},{:.6}\n",
                b.id,
                sub + 1,
                at.lat,
                at.lon
            ));
        }
        s
    }
}

impl Report {
    /// A readable summary with the paper's Table VI layout.
    pub fn markdown(&self) -> String {
        let s = &self.substations;
        let ac = &self.ac;
        let mut out = format!(
            "# {name}\n\nSeed {seed}. {buses} buses, {gens} generators, {lines} line circuits, {xf} transformers. {repair} lines were added after the main rounds to join split levels and remove cut substations.\n\n\
             Substations: {total} ({load} load only, {gen} generation only, {both} both). Plants: {plants}, {cap} MW installed.\n\n",
            name = self.name,
            seed = self.seed,
            buses = self.buses,
            gens = self.generators,
            lines = self.lines,
            xf = self.transformers,
            repair = self.repair_lines,
            total = s.total,
            load = s.load,
            gen = s.generation,
            both = s.both,
            plants = self.plants,
            cap = self.capacity_mw,
        );
        out.push_str("## Structural statistics\n\n");
        out.push_str(&self.structure.table());
        out.push_str(&format!(
            "\nWhole network connected: {}. Substations whose loss splits it: {}.\n\n",
            if self.structure.connected {
                "yes"
            } else {
                "no"
            },
            self.structure.cut_substations
        ));
        out.push_str("## AC power flow\n\n");
        if ac.converged {
            out.push_str(&format!(
                "Converged in {} iterations{}. Load {:.0} MW, generation {:.0} MW.\n\n\
                 Voltages {:.3}-{:.3} pu at load buses, {} outside the band. Highest branch loading {:.0}%, {} over the limit.\n\n\
                 Added {} capacitor banks ({:.0} Mvar) and {} reactors ({:.0} Mvar); upgraded {} lines or transformers.\n\n",
                ac.iterations,
                if ac.ramp_steps.is_empty() {
                    String::new()
                } else {
                    format!(" (ramped load through {:?})", ac.ramp_steps)
                },
                ac.load_mw,
                ac.generation_mw,
                ac.vmin,
                ac.vmax,
                ac.buses_outside_band,
                100.0 * ac.max_loading,
                ac.branches_over_limit,
                ac.capacitors,
                ac.capacitor_mvar,
                ac.reactors,
                ac.reactor_mvar,
                ac.upgrades
            ));
        } else {
            out.push_str("Did not converge.\n\n");
        }
        out.push_str("## Time\n\n| Stage | Seconds |\n| --- | --- |\n");
        for (name, secs) in &self.seconds {
            out.push_str(&format!("| {name} | {secs} |\n"));
        }
        out
    }
}
