//! Turn placed substations and lines into a solvable case: buses at each
//! level, step-up and tie transformers, conductors sized from dc flows,
//! then an ac solve with shunts added where voltages leave the target band
//! (Sections III-A, IV and V of the paper).

use crate::config::{Config, Fuel};
use crate::geo::LatLon;
use crate::lines::Line;
use crate::params::{Conductor, Design};
use crate::rng::Rng;
use crate::substations::Placement;
use rs_power::{
    Branch, Bus, BusType, Case, Generator, Initialization, Model, PowerFlowOptions, SolveStatus,
};
use serde::Serialize;

/// What a branch row is, for the report and for resizing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Line,
    Transformer,
    StepUp,
}

#[derive(Clone, Debug)]
struct LineDesign {
    line: usize,
    conductor: Conductor,
    circuits: u32,
    gmd_scale: f64,
}

#[derive(Clone, Debug)]
struct Tie {
    sub: usize,
    lo: usize,
    hi: usize,
    mva: f64,
    units: u32,
}

const TRANSFORMER_SIZES: [f64; 10] = [
    100.0, 150.0, 200.0, 300.0, 400.0, 600.0, 800.0, 1000.0, 1200.0, 1500.0,
];

fn transformer_size(mva: f64) -> (f64, u32) {
    match TRANSFORMER_SIZES.iter().find(|&&s| s >= mva) {
        Some(&s) => (s, 1),
        None => {
            let units = (mva / 1500.0).ceil().clamp(1.0, 6.0) as u32;
            (1500.0, units)
        }
    }
}

/// How the ac solve went.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AcSummary {
    pub converged: bool,
    pub iterations: usize,
    /// Load levels solved on the way when a flat start didn't converge.
    pub ramp_steps: Vec<f64>,
    pub capacitors: usize,
    pub capacitor_mvar: f64,
    pub reactors: usize,
    pub reactor_mvar: f64,
    pub vmin: f64,
    pub vmax: f64,
    pub buses_outside_band: usize,
    pub max_loading: f64,
    pub branches_over_limit: usize,
    pub upgrades: usize,
    pub load_mw: f64,
    pub generation_mw: f64,
}

pub struct Built {
    pub case: Case,
    /// Substation of each bus, in bus order.
    pub bus_substation: Vec<usize>,
    pub roles: Vec<Role>,
    pub ac: AcSummary,
}

struct Layout {
    /// Bus id of each (substation, level index into the config).
    level_bus: Vec<Vec<(usize, u32)>>,
    /// Generator bus id of each plant.
    plant_bus: Vec<u32>,
    buses: Vec<Bus>,
    bus_substation: Vec<usize>,
    names: Vec<String>,
}

fn bus(id: u32, kv: f64, area: u32) -> Bus {
    Bus {
        id,
        kind: BusType::Pq,
        pd: 0.0,
        qd: 0.0,
        pd_i: 0.0,
        qd_i: 0.0,
        pd_z: 0.0,
        qd_z: 0.0,
        gs: 0.0,
        bs: 0.0,
        area,
        vm: 1.0,
        va: 0.0,
        base_kv: kv,
        zone: area,
        vmax: 1.1,
        vmin: 0.9,
        extra: vec![],
    }
}

fn generator_kv(mw: f64) -> f64 {
    if mw < 150.0 {
        13.8
    } else if mw < 500.0 {
        18.0
    } else {
        22.0
    }
}

fn layout(cfg: &Config, p: &Placement) -> Layout {
    let subs = &p.substations;
    let mut order: Vec<usize> = (0..subs.len()).collect();
    order.sort_by_key(|&i| (subs[i].area, i));
    let mut level_bus = vec![Vec::new(); subs.len()];
    let mut plant_bus = vec![0u32; p.plants.len()];
    let mut buses = Vec::new();
    let mut bus_substation = Vec::new();
    let mut names = Vec::new();
    let tan = (1.0 / cfg.load.power_factor.powi(2) - 1.0).max(0.0).sqrt();
    for (k, &si) in order.iter().enumerate() {
        let s = &subs[si];
        for (j, &li) in s.levels.iter().enumerate() {
            let id = buses.len() as u32 + 1;
            let kv = cfg.levels[li].kv;
            let mut b = bus(id, kv, s.area);
            if j == 0 {
                b.pd = s.load_mw;
                b.qd = (s.load_mw * tan * 100.0).round() / 100.0;
            }
            buses.push(b);
            bus_substation.push(si);
            names.push(format!("S{} {}", k + 1, kv as i64));
            level_bus[si].push((li, id));
        }
        for (u, &pi) in s.plants.iter().enumerate() {
            let id = buses.len() as u32 + 1;
            buses.push(bus(id, generator_kv(p.plants[pi].mw), s.area));
            bus_substation.push(si);
            names.push(format!("S{} G{}", k + 1, u + 1));
            plant_bus[pi] = id;
        }
    }
    Layout {
        level_bus,
        plant_bus,
        buses,
        bus_substation,
        names,
    }
}

fn line_branch(from: u32, to: u32, p: crate::params::LineParams) -> Branch {
    Branch {
        from,
        to,
        r: p.r,
        x: p.x,
        b: p.b,
        rate_a: p.mva.round(),
        rate_b: (1.1 * p.mva).round(),
        rate_c: (1.2 * p.mva).round(),
        tap: 0.0,
        shift: 0.0,
        online: true,
        angmin: -360.0,
        angmax: 360.0,
        extra: vec![],
    }
}

fn transformer(from: u32, to: u32, mva: f64, x_own: f64, base: f64) -> Branch {
    let x = x_own * base / mva;
    Branch {
        from,
        to,
        r: x / 40.0,
        x,
        b: 0.0,
        rate_a: mva,
        rate_b: mva * 1.1,
        rate_c: mva * 1.2,
        tap: 1.0,
        shift: 0.0,
        online: true,
        angmin: -360.0,
        angmax: 360.0,
        extra: vec![],
    }
}

struct Assembly<'a> {
    cfg: &'a Config,
    placement: &'a Placement,
    lines: &'a [Line],
    designs: Vec<Design>,
    layout: Layout,
    line_designs: Vec<LineDesign>,
    ties: Vec<Tie>,
    /// Logical item behind each branch row: line design or tie index.
    rows: Vec<(Role, usize)>,
    /// Output of each plant, MW.
    dispatch: Vec<f64>,
}

impl Assembly<'_> {
    fn bus_at(&self, sub: usize, level: usize) -> u32 {
        self.layout.level_bus[sub]
            .iter()
            .find(|(l, _)| *l == level)
            .expect("substation has the level")
            .1
    }

    fn case(&mut self) -> Case {
        let cfg = self.cfg;
        let base = cfg.base_mva;
        let mut branches = Vec::new();
        self.rows.clear();
        for (k, d) in self.line_designs.iter().enumerate() {
            let line = &self.lines[d.line];
            let design = &self.designs[line.level];
            let params = design.line(&d.conductor, line.miles.max(0.5), d.gmd_scale, base);
            let (from, to) = (
                self.bus_at(line.a, line.level),
                self.bus_at(line.b, line.level),
            );
            for _ in 0..d.circuits {
                branches.push(line_branch(from, to, params));
                self.rows.push((Role::Line, k));
            }
        }
        for (k, t) in self.ties.iter().enumerate() {
            let (from, to) = (self.bus_at(t.sub, t.hi), self.bus_at(t.sub, t.lo));
            for _ in 0..t.units {
                branches.push(transformer(from, to, t.mva, 0.10, base));
                self.rows.push((Role::Transformer, k));
            }
        }
        let mut generators = Vec::new();
        let mut gen_names = Vec::new();
        let mut biggest = (0.0, 0u32);
        for (si, s) in self.placement.substations.iter().enumerate() {
            let top = *s.levels.iter().max().expect("a substation has a level");
            let hv = self.bus_at(si, top);
            for &pi in &s.plants {
                let plant = &self.placement.plants[pi];
                let gb = self.layout.plant_bus[pi];
                // Room for full output with reactive power at its limit.
                let (mva, units) = transformer_size(1.25 * plant.mw);
                for _ in 0..units {
                    branches.push(transformer(gb, hv, mva, 0.12, base));
                }
                for _ in 0..units {
                    self.rows.push((Role::StepUp, pi));
                }
                let renewable = matches!(plant.fuel, Fuel::Wind | Fuel::Solar);
                let (qmax, qmin) = if renewable {
                    (0.33 * plant.mw, -0.33 * plant.mw)
                } else {
                    (0.6 * plant.mw, -0.3 * plant.mw)
                };
                generators.push(Generator {
                    bus: gb,
                    pg: (self.dispatch[pi] * 100.0).round() / 100.0,
                    qg: 0.0,
                    qmax: qmax.round(),
                    qmin: qmin.round(),
                    // Hold the substation's high side, as plant controls do.
                    vg: 1.03,
                    mbase: (plant.mw / 0.9).round(),
                    online: true,
                    pmax: plant.mw,
                    pmin: 0.0,
                    extra: vec![],
                    regulated_bus: Some(hv),
                    // Units holding one bus share its Mvar by their reactive
                    // range, so they reach their limits together. Equal
                    // shares asked a 10 Mvar unit for as much as a 1,000 MW
                    // one beside it.
                    reactive_share: (qmax.round() - qmin.round()).max(1.0),
                });
                gen_names.push(plant.fuel.matpower().to_string());
                if plant.mw > biggest.0 {
                    biggest = (plant.mw, gb);
                }
            }
        }
        let mut buses = self.layout.buses.clone();
        for g in &generators {
            buses[g.bus as usize - 1].kind = BusType::Pv;
        }
        if biggest.1 > 0 {
            buses[biggest.1 as usize - 1].kind = BusType::Reference;
        }
        let mut labels = std::collections::BTreeMap::new();
        labels.insert("bus_name".to_string(), self.layout.names.clone());
        labels.insert("genfuel".to_string(), gen_names);
        Case {
            base_mva: base,
            buses,
            generators,
            branches,
            dcline: None,
            gencost: None,
            labels,
        }
    }

    /// Resize lines and transformers to carry `flows` (MW per row) at
    /// `loading` of their rating. Returns how many items changed.
    fn resize(&mut self, flows: &[f64], loading: f64, only_grow: bool) -> usize {
        let mut line_flow = vec![0.0; self.line_designs.len()];
        let mut tie_flow = vec![0.0; self.ties.len()];
        for (&(role, k), f) in self.rows.iter().zip(flows) {
            match role {
                Role::Line => line_flow[k] += f.abs(),
                Role::Transformer => tie_flow[k] += f.abs(),
                Role::StepUp => {}
            }
        }
        let mut changed = 0;
        for (d, &f) in self.line_designs.iter_mut().zip(&line_flow) {
            let design = &self.designs[self.lines[d.line].level];
            let (c, n) = design.size(f, loading);
            let bigger =
                design.rating(&c) * n as f64 > design.rating(&d.conductor) * d.circuits as f64;
            if (c.name, n) != (d.conductor.name, d.circuits) && (!only_grow || bigger) {
                d.conductor = c;
                d.circuits = n;
                changed += 1;
            }
        }
        for (t, &f) in self.ties.iter_mut().zip(&tie_flow) {
            let (mva, units) = transformer_size((f / loading).max(100.0));
            let bigger = mva * units as f64 > t.mva * t.units as f64;
            if (mva, units) != (t.mva, t.units) && (!only_grow || bigger) {
                t.mva = mva;
                t.units = units;
                changed += 1;
            }
        }
        changed
    }
}

/// Peak dispatch: wind, solar and hydro at typical peak output, nuclear and
/// coal near full, and gas filling the rest so generation covers `need`
/// (load plus losses). Gas sits near load, so this keeps long transfers
/// realistic. Returns the share of capacity each fuel runs at.
fn merit_dispatch(
    plants: &[crate::substations::Plant],
    need: f64,
) -> std::collections::HashMap<Fuel, f64> {
    use std::collections::HashMap;
    let mut cap: HashMap<Fuel, f64> = HashMap::new();
    for p in plants {
        *cap.entry(p.fuel).or_default() += p.mw;
    }
    let c = |f: Fuel| cap.get(&f).copied().unwrap_or(0.0);
    let mut share: HashMap<Fuel, f64> = HashMap::from([
        (Fuel::Wind, 0.3),
        (Fuel::Solar, 0.55),
        (Fuel::Hydro, 0.5),
        (Fuel::Nuclear, 0.95),
        (Fuel::Coal, 0.8),
        (Fuel::Gas, 0.0),
    ]);
    let fixed: f64 = [
        Fuel::Wind,
        Fuel::Solar,
        Fuel::Hydro,
        Fuel::Nuclear,
        Fuel::Coal,
    ]
    .iter()
    .map(|&f| share[&f] * c(f))
    .sum();
    let gas = (need - fixed) / c(Fuel::Gas).max(1e-9);
    if fixed > need {
        // Too much: turn everything down evenly.
        for v in share.values_mut() {
            *v *= need / fixed;
        }
    } else if gas <= 0.95 {
        share.insert(Fuel::Gas, gas);
    } else {
        // Not enough gas: run gas at 95% and the rest up toward 95% evenly.
        share.insert(Fuel::Gas, 0.95);
        let short = need - fixed - 0.95 * c(Fuel::Gas);
        let room: f64 = [
            Fuel::Wind,
            Fuel::Solar,
            Fuel::Hydro,
            Fuel::Nuclear,
            Fuel::Coal,
        ]
        .iter()
        .map(|&f| (0.95 - share[&f]).max(0.0) * c(f))
        .sum();
        let k = (short / room.max(1e-9)).min(1.0);
        for f in [
            Fuel::Wind,
            Fuel::Solar,
            Fuel::Hydro,
            Fuel::Nuclear,
            Fuel::Coal,
        ] {
            let v = share[&f];
            share.insert(f, v + k * (0.95 - v).max(0.0));
        }
    }
    share
}

/// Each plant's output: the fuel shares of [`merit_dispatch`], with gas and
/// coal moved between areas so that each area covers its own load where it
/// can, as balancing authorities schedule small interchanges. Without this,
/// a continent-sized case ships power across its whole width.
fn balanced_dispatch(placement: &Placement, need: f64) -> Vec<f64> {
    let plants = &placement.plants;
    let subs = &placement.substations;
    let share = merit_dispatch(plants, need);
    let flexible = |f: Fuel| matches!(f, Fuel::Gas | Fuel::Coal);
    let mut area_of = vec![0usize; plants.len()];
    let areas = subs.iter().map(|s| s.area as usize + 1).max().unwrap_or(1);
    let mut load = vec![0.0; areas];
    for s in subs {
        load[s.area as usize] += s.load_mw;
        for &p in &s.plants {
            area_of[p] = s.area as usize;
        }
    }
    let total_load: f64 = load.iter().sum::<f64>().max(1e-9);
    let mut out: Vec<f64> = plants.iter().map(|p| p.mw * share[&p.fuel]).collect();
    let flex_total: f64 = (0..plants.len())
        .filter(|&i| flexible(plants[i].fuel))
        .map(|i| out[i])
        .sum();
    if flex_total <= 0.0 {
        return out;
    }
    // Flexible output and room in each area.
    let (mut fixed, mut cap) = (vec![0.0; areas], vec![0.0; areas]);
    for (i, p) in plants.iter().enumerate() {
        if flexible(p.fuel) {
            cap[area_of[i]] += 0.95 * p.mw;
        } else {
            fixed[area_of[i]] += out[i];
        }
    }
    let mut flex: Vec<f64> = (0..areas)
        .map(|a| (load[a] / total_load * need - fixed[a]).clamp(0.0, cap[a]))
        .collect();
    // Spread what's left over (or short) across areas with room, a few
    // times since some areas fill up.
    for _ in 0..8 {
        let short = flex_total - flex.iter().sum::<f64>();
        if short.abs() < 1e-6 * need {
            break;
        }
        let room: Vec<f64> = (0..areas)
            .map(|a| {
                if short > 0.0 {
                    cap[a] - flex[a]
                } else {
                    flex[a]
                }
            })
            .collect();
        let sum: f64 = room.iter().sum();
        if sum <= 0.0 {
            break;
        }
        for a in 0..areas {
            flex[a] += short * room[a] / sum;
        }
    }
    for (i, p) in plants.iter().enumerate() {
        if flexible(p.fuel) && cap[area_of[i]] > 0.0 {
            out[i] = 0.95 * p.mw * flex[area_of[i]] / cap[area_of[i]];
        }
    }
    out
}

fn options(model: Model) -> PowerFlowOptions {
    PowerFlowOptions {
        model,
        initialization: Initialization::Case,
        tolerance: 1e-5,
        max_iterations: 30,
        // Generators keep to their reactive limits and hold their
        // substation's high-side bus.
        enforce_q_limits: model == Model::Ac,
        ..PowerFlowOptions::default()
    }
}

/// The case with load and generation at `step` of `full`'s.
fn at_step(case: &Case, full: &Case, step: f64) -> Case {
    let mut c = case.clone();
    for (b, f) in c.buses.iter_mut().zip(&full.buses) {
        b.pd = f.pd * step;
        b.qd = f.qd * step;
    }
    for (g, f) in c.generators.iter_mut().zip(&full.generators) {
        g.pg = f.pg * step;
    }
    c
}

/// A converged ac solution, or None. A solution with any bus under 0.6 pu
/// is a collapsed one, not an operating point, and counts as a failure.
fn solve_ac(case: &Case) -> Option<rs_power::PowerFlowResult> {
    rs_power::solve(case, &options(Model::Ac))
        .ok()
        .filter(|r| r.status == SolveStatus::Converged && r.case.buses.iter().all(|b| b.vm > 0.6))
}

/// Solve `step` without reactive limits and put a shunt at each generator's
/// high-side bus for the reactive power it would need beyond its limits:
/// reactors where line charging is more than the plants can absorb,
/// capacitors where they can't supply enough. Returns how many were added.
fn compensate_generators(case: &mut Case, step: &Case) -> usize {
    let mut free = options(Model::Ac);
    free.enforce_q_limits = false;
    let Some(r) = rs_power::solve(step, &free)
        .ok()
        .filter(|r| r.status == SolveStatus::Converged)
    else {
        return 0;
    };
    let mut need = vec![0.0; case.buses.len()];
    for g in &r.case.generators {
        let bus = g.regulated_bus.unwrap_or(g.bus) as usize - 1;
        if g.qg > g.qmax {
            need[bus] += g.qg - g.qmax;
        } else if g.qg < g.qmin {
            need[bus] -= g.qmin - g.qg;
        }
    }
    let mut added = 0;
    for (b, &q) in case.buses.iter_mut().zip(&need) {
        if q.abs() < 5.0 {
            continue;
        }
        let bank = if b.base_kv >= 200.0 { 25.0 } else { 5.0 };
        b.bs += (q / bank).round() * bank;
        added += 1;
    }
    added
}

/// Add a capacitor bank at load buses below `lo` and, when `reactors`, a
/// reactor at those above `hi` (taking capacitors out first). Only a bus
/// that is the lowest (or highest) among its neighbours gets one in a pass,
/// so a sagging area isn't over-compensated all at once. Banks are sized
/// from the ratings of the bus's branches. Returns how many changed.
fn add_shunts(
    case: &mut Case,
    solved: &Case,
    lo: f64,
    hi: f64,
    reactors: bool,
    gain: f64,
) -> usize {
    let n = case.buses.len();
    let mut strength = vec![0.0; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for br in &case.branches {
        let (f, t) = (br.from as usize - 1, br.to as usize - 1);
        strength[f] += br.rate_a;
        strength[t] += br.rate_a;
        adj[f].push(t);
        adj[t].push(f);
    }
    let v: Vec<f64> = solved.buses.iter().map(|b| b.vm).collect();
    let mut added = 0;
    for (i, b) in solved.buses.iter().enumerate() {
        if b.kind != BusType::Pq {
            continue;
        }
        let target = if b.vm < lo && adj[i].iter().all(|&j| v[j] >= v[i]) {
            lo + 0.015
        } else if reactors && b.vm > hi && adj[i].iter().all(|&j| v[j] <= v[i]) {
            hi - 0.015
        } else {
            continue;
        };
        let (step, most) = if b.base_kv >= 200.0 {
            (25.0, 100.0)
        } else {
            (5.0, 30.0)
        };
        let mvar = (target - b.vm) * 0.5 * gain * strength[i];
        let size =
            ((mvar.abs() / step).ceil().max(1.0) * step).min(most * gain.max(0.25)) * mvar.signum();
        case.buses[i].bs += size;
        added += 1;
    }
    added
}

/// Count the shunts in `case` for the report.
fn tally_shunts(case: &Case, ac: &mut AcSummary) {
    ac.capacitors = case.buses.iter().filter(|b| b.bs > 0.0).count();
    ac.capacitor_mvar = case.buses.iter().filter(|b| b.bs > 0.0).map(|b| b.bs).sum();
    ac.reactors = case.buses.iter().filter(|b| b.bs < 0.0).count();
    ac.reactor_mvar = -case
        .buses
        .iter()
        .filter(|b| b.bs < 0.0)
        .map(|b| b.bs)
        .sum::<f64>();
}

fn keep_voltages(case: &mut Case, solved: &Case) {
    for (b, s) in case.buses.iter_mut().zip(&solved.buses) {
        b.vm = s.vm;
        b.va = s.va;
    }
}

/// The best full-load pass so far, with what's needed to go back to it.
struct Best {
    score: usize,
    case: Case,
    result: rs_power::PowerFlowResult,
    rows: Vec<(Role, usize)>,
    line_designs: Vec<LineDesign>,
    ties: Vec<Tie>,
    upgrades: usize,
}

pub fn build(cfg: &Config, placement: &Placement, lines: &[Line], rng: &mut Rng) -> Built {
    let subs = &placement.substations;
    let designs: Vec<Design> = cfg.levels.iter().map(|l| Design::for_kv(l.kv)).collect();
    let layout = layout(cfg, placement);
    let line_designs = lines
        .iter()
        .enumerate()
        .map(|(i, l)| LineDesign {
            line: i,
            conductor: designs[l.level].conductors[0],
            circuits: 1,
            // One tower style per line: spacing within 10% of typical.
            gmd_scale: rng.range(0.9, 1.1),
        })
        .collect();
    let mut ties = Vec::new();
    for (si, s) in subs.iter().enumerate() {
        let mut levels = s.levels.clone();
        levels.sort_unstable();
        for w in levels.windows(2) {
            ties.push(Tie {
                sub: si,
                lo: w[0],
                hi: w[1],
                mva: 300.0,
                units: 1,
            });
        }
    }
    let total_load: f64 = subs.iter().map(|s| s.load_mw).sum();
    let mut a = Assembly {
        cfg,
        placement,
        lines,
        designs,
        layout,
        line_designs,
        ties,
        rows: vec![],
        dispatch: balanced_dispatch(placement, total_load * 1.02),
    };

    // Size conductors and transformers from dc flows (a few passes, since
    // flows move as impedances change).
    for pass in 0..4 {
        let case = a.case();
        let Ok(dc) = rs_power::solve(&case, &options(Model::Dc)) else {
            break;
        };
        let flows: Vec<f64> = dc.branch_flows.iter().map(|f| f[0]).collect();
        let changed = a.resize(&flows, 0.6, pass > 1);
        log::info!(
            "dc sizing pass {}: {changed} lines and transformers resized",
            pass + 1
        );
        if changed == 0 {
            break;
        }
    }

    let mut ac = AcSummary::default();
    let mut case = a.case();
    let full = case.clone();
    let (lo, hi) = (cfg.voltage.low, cfg.voltage.high);
    if let Ok(dc) = rs_power::solve(&case, &options(Model::Dc)) {
        for (b, d) in case.buses.iter_mut().zip(&dc.case.buses) {
            b.va = d.va;
        }
    }

    // Bring load up in steps, adding capacitor banks wherever voltages sag
    // below the band before going on, as a planner would compensate a
    // growing system (Section IV). A step that won't converge is split.
    let mut solved = solve_ac(&case);
    if solved.is_none() {
        let mut done = 0.0;
        let mut step = 0.3;
        let mut splits = 0;
        let mut last_solution = None;
        while done < 1.0 && splits < 6 {
            let target: f64 = if done == 0.0 {
                step
            } else {
                (done + step).min(1.0)
            };
            let mut ok = None;
            let mut last_good = case.clone();
            for _ in 0..12 {
                let mut r = solve_ac(&at_step(&case, &full, target));
                for _ in 0..3 {
                    if r.is_some() {
                        break;
                    }
                    let now = at_step(&case, &full, target);
                    let added = compensate_generators(&mut case, &now);
                    log::info!(
                        "ac at {:.0}% load: {added} shunts added for generators past their reactive limits",
                        100.0 * target
                    );
                    if added == 0 {
                        break;
                    }
                    r = solve_ac(&at_step(&case, &full, target));
                }
                let Some(r) = r else {
                    log::info!("ac at {:.0}% load: no convergence", 100.0 * target);
                    break;
                };
                keep_voltages(&mut case, &r.case);
                last_good = case.clone();
                let added = add_shunts(&mut case, &r.case, lo, hi, false, 1.0);
                let vmin = r
                    .case
                    .buses
                    .iter()
                    .map(|b| b.vm)
                    .fold(f64::INFINITY, f64::min);
                let total: f64 = case.buses.iter().map(|b| b.bs).sum();
                let big = case.buses.iter().map(|b| b.bs).fold(0.0, f64::max);
                let low = r.case.buses.iter().filter(|b| b.vm < 0.9).count();
                log::info!(
                    "ac at {:.0}% load: lowest voltage {vmin:.3} pu ({low} under 0.9), {added} capacitor banks added, total {total:.0} biggest {big:.0}",
                    100.0 * target
                );
                ok = Some(r);
                if added == 0 {
                    break;
                }
            }
            // Keep the case that matches the step's solution, undoing any
            // banks after it (or, on failure, the ones tried for it).
            case = last_good;
            if let Some(r) = ok {
                ac.ramp_steps.push((target * 100.0).round() / 100.0);
                done = target;
                // The solution of the case as it now stands (the last
                // banks that broke convergence were taken back out).
                last_solution = Some(r);
            } else {
                step /= 2.0;
                splits += 1;
            }
        }
        solved = if done >= 1.0 { last_solution } else { None };
    }

    // At full load: capacitors and reactors for the band, and bigger
    // conductors, more circuits or transformers where branches run over
    // their limit, re-solving from the last answer each time. Banks get
    // smaller as passes go on so neighbouring fixes settle instead of
    // swinging, and the best pass is kept.
    let flow_mva = |f: &[f64; 4]| {
        (f[0] * f[0] + f[1] * f[1])
            .sqrt()
            .max((f[2] * f[2] + f[3] * f[3]).sqrt())
    };
    let problems = |case: &Case, r: &rs_power::PowerFlowResult| {
        let out = r
            .case
            .buses
            .iter()
            .filter(|b| b.kind == BusType::Pq && (b.vm < lo || b.vm > hi))
            .count();
        let over = case
            .branches
            .iter()
            .zip(&r.branch_flows)
            .filter(|(b, f)| b.rate_a > 0.0 && flow_mva(f) > cfg.max_loading * b.rate_a)
            .count();
        out + over
    };
    let mut best: Option<Best> = None;
    let mut scale = 1.0;
    let mut failures = 0;
    for pass in 0..40 {
        let r = match solved.take() {
            Some(r) => r,
            None => {
                // The last changes broke convergence: go back to the best
                // pass and make smaller ones.
                let Some(b) = &best else { break };
                failures += 1;
                if failures > 2 {
                    break;
                }
                log::info!("full load pass {pass}: no convergence, back to the best pass");
                case = b.case.clone();
                a.rows = b.rows.clone();
                a.line_designs = b.line_designs.clone();
                a.ties = b.ties.clone();
                ac.upgrades = b.upgrades;
                scale *= 0.5;
                b.result.clone()
            }
        };
        let score = problems(&case, &r);
        log::info!(
            "full load pass {pass}: {score} buses outside the band or branches over the limit"
        );
        keep_voltages(&mut case, &r.case);
        if best.as_ref().is_none_or(|b| score < b.score) {
            best = Some(Best {
                score,
                case: case.clone(),
                result: r.clone(),
                rows: a.rows.clone(),
                line_designs: a.line_designs.clone(),
                ties: a.ties.clone(),
                upgrades: ac.upgrades,
            });
        }
        if score == 0 {
            break;
        }
        let gain = scale
            * if pass < 10 {
                1.0
            } else if pass < 25 {
                0.5
            } else {
                0.25
            };
        let mut changed = add_shunts(&mut case, &r.case, lo, hi, true, gain);
        let flows: Vec<f64> = r.branch_flows.iter().map(flow_mva).collect();
        let over = case
            .branches
            .iter()
            .zip(&flows)
            .any(|(b, f)| b.rate_a > 0.0 && *f > cfg.max_loading * b.rate_a);
        // After a failure, shunts only: upgrades move impedances most.
        if over && failures == 0 {
            let ups = a.resize(&flows, cfg.max_loading * 0.9, true);
            ac.upgrades += ups;
            if ups > 0 {
                let mut next = a.case();
                for (b, s) in next.buses.iter_mut().zip(&case.buses) {
                    b.bs = s.bs;
                    b.vm = s.vm;
                    b.va = s.va;
                }
                case = next;
                changed += ups;
            }
        }
        if changed == 0 {
            break;
        }
        solved = solve_ac(&case);
    }
    if let Some(b) = best {
        case = b.case;
        solved = Some(b.result);
        a.rows = b.rows;
        a.line_designs = b.line_designs;
        a.ties = b.ties;
        ac.upgrades = b.upgrades;
    }
    tally_shunts(&case, &mut ac);

    if let Some(r) = &solved {
        ac.converged = true;
        ac.iterations = r.iterations;
        let pq = r.case.buses.iter().filter(|b| b.kind == BusType::Pq);
        ac.vmin = pq.clone().map(|b| b.vm).fold(f64::INFINITY, f64::min);
        ac.vmax = pq.clone().map(|b| b.vm).fold(f64::NEG_INFINITY, f64::max);
        ac.buses_outside_band = pq.filter(|b| b.vm < lo - 1e-6 || b.vm > hi + 1e-6).count();
        for (b, f) in case.branches.iter().zip(&r.branch_flows) {
            let s = (f[0] * f[0] + f[1] * f[1])
                .sqrt()
                .max((f[2] * f[2] + f[3] * f[3]).sqrt());
            if b.rate_a > 0.0 {
                let l = s / b.rate_a;
                ac.max_loading = ac.max_loading.max(l);
                if l > cfg.max_loading {
                    ac.branches_over_limit += 1;
                }
            }
        }
        ac.load_mw = r.case.buses.iter().map(|b| b.pd).sum();
        ac.generation_mw = r.case.generators.iter().map(|g| g.pg).sum();
        // Keep the solved voltages and dispatch as the case's starting point.
        for (b, s) in case.buses.iter_mut().zip(&r.case.buses) {
            b.vm = (s.vm * 1e6).round() / 1e6;
            b.va = (s.va * 1e4).round() / 1e4;
        }
        for (g, s) in case.generators.iter_mut().zip(&r.case.generators) {
            g.pg = (s.pg * 100.0).round() / 100.0;
            g.qg = (s.qg * 100.0).round() / 100.0;
        }
    }
    let roles = a.rows.iter().map(|r| r.0).collect();
    Built {
        case,
        bus_substation: a.layout.bus_substation,
        roles,
        ac,
    }
}

/// Coordinates of each bus (its substation's).
pub fn bus_coordinates(built: &Built, placement: &Placement) -> Vec<LatLon> {
    built
        .bus_substation
        .iter()
        .map(|&s| placement.substations[s].at)
        .collect()
}
