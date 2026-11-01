//! PV curves (continuation power flow): how far a transfer, or a load
//! increase, can go before the network runs out of voltage support.
//!
//! The transfer grows in steps. At each step the sink's loads (or
//! generation) and the source's generation (or loads) move by the step,
//! shared out as for transfer limits, and the case is solved again with the
//! full AC power flow, starting from the step before, with every control the
//! last solve used. When a step doesn't converge it is halved and tried
//! again, until it is smaller than the resolution: the last step that
//! converged is the nose of the curve, the most the network can carry. This
//! is how PSS/E's PV analysis finds the nose, and it keeps every control
//! and limit of a normal solve. The curve stops on its upper half: the
//! nose, not the unstable half past it, is the answer.
//!
//! Each step records the voltage at every monitored bus. The answer is the
//! transfer at the nose, the transfer at which a monitored bus first drops
//! below its low voltage limit, and the curves of the buses whose voltage
//! falls furthest.

use crate::contingency::RunHandle;
use crate::power::{self, PowerCase, SolveSettings};
use crate::transfer::{Participation, Scaling, Subsystem, participation};
use rs_power::{BusType, Case, PowerFlowResult, SolveStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Newton iterations per pass for each step, at least.
const MIN_ITERATIONS: usize = 30;

fn load_scaling() -> Scaling {
    Scaling::Load
}
fn default_curves() -> usize {
    10
}
fn default_max_points() -> usize {
    400
}

/// A PV curve study.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PvSettings {
    /// Where the extra power comes from. Every bus when left empty, so a
    /// load increase in the sink is picked up by all generation.
    #[serde(default)]
    pub source: Subsystem,
    /// How the source shares out its part: generation by default.
    #[serde(default)]
    pub source_scaling: Scaling,
    /// Where the power goes.
    #[serde(default)]
    pub sink: Subsystem,
    /// How the sink shares out its part: its loads grow, at constant power
    /// factor, by default.
    #[serde(default = "load_scaling")]
    pub sink_scaling: Scaling,
    /// The transfer added at each step, MW. 5% of what the sink starts with
    /// (its load, or generation) when absent.
    #[serde(default)]
    pub step_mw: Option<f64>,
    /// The smallest step tried before the last converged step counts as the
    /// nose, MW. A 32nd of the first step when absent.
    #[serde(default)]
    pub min_step_mw: Option<f64>,
    /// Stop at this transfer, MW, even when the network could carry more.
    #[serde(default)]
    pub max_transfer_mw: Option<f64>,
    /// Stop when the source or sink runs out of room (generators at Pmax
    /// or Pmin, loads at zero), as transfer limits count it. On by default.
    #[serde(default = "yes")]
    pub respect_generation_limits: bool,
    /// Buses whose voltages are watched: every energized bus when absent.
    #[serde(default)]
    pub monitor: Option<Subsystem>,
    /// Low voltage limit for every monitored bus, p.u. Each bus's own
    /// normal low limit from the case when absent (0.9 p.u. when it has
    /// none).
    #[serde(default)]
    pub vmin_pu: Option<f64>,
    /// Buses that always get a curve, as well as the ones whose voltage
    /// falls furthest.
    #[serde(default)]
    pub buses: Vec<u32>,
    /// How many of the buses whose voltage falls furthest get a curve.
    #[serde(default = "default_curves")]
    pub max_curves: usize,
    /// The most steps solved, failed ones included.
    #[serde(default = "default_max_points")]
    pub max_points: usize,
}

fn yes() -> bool {
    true
}

impl Default for PvSettings {
    fn default() -> Self {
        Self {
            source: Subsystem::default(),
            source_scaling: Scaling::Generation,
            sink: Subsystem::default(),
            sink_scaling: Scaling::Load,
            step_mw: None,
            min_step_mw: None,
            max_transfer_mw: None,
            respect_generation_limits: true,
            monitor: None,
            vmin_pu: None,
            buses: Vec::new(),
            max_curves: default_curves(),
            max_points: default_max_points(),
        }
    }
}

/// One converged step of the curve.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct PvPoint {
    /// Transfer added since the base case, MW.
    pub transfer_mw: f64,
    /// The sink's total load at this step, MW, at the solved voltages.
    pub sink_load_mw: f64,
    /// Total generation less total load, MW.
    pub losses_mw: f64,
    /// The lowest monitored voltage and its bus.
    pub min_vm_pu: f64,
    pub min_vm_bus: u32,
    /// Online generators held at a reactive power limit (on voltage
    /// control buses), and their reactive reserve left (room to Qmax), Mvar.
    pub q_limited: usize,
    pub q_reserve_mvar: f64,
    pub iterations: usize,
}

/// The voltage of one bus at each point of the curve.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct PvCurve {
    pub bus: u32,
    pub name: String,
    pub base_kv: f64,
    /// The bus's low voltage limit used, p.u.
    pub vmin_pu: f64,
    /// Voltage at each point of `points`, p.u.
    pub vm_pu: Vec<f64>,
    /// How far its voltage fell from the base case to the last point, p.u.
    pub drop_pu: f64,
    /// Voltage drop per 100 MW of transfer over the last step: the largest
    /// is the weakest bus at the nose.
    pub sensitivity_pu_per_100mw: f64,
    /// The transfer at which it falls below its low limit, MW, linear
    /// between points; `None` when it stays above it (or starts below).
    pub vmin_transfer_mw: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct PvSummary {
    /// The largest transfer that converged, MW: the nose when the curve
    /// stopped because the network couldn't carry more.
    pub nose_mw: f64,
    /// What stopped the curve.
    #[schemars(extend("enum" = ["collapse", "generation", "max_transfer", "max_points", "cancelled"]))]
    pub limited_by: &'static str,
    /// The nose lies between `nose_mw` and `nose_mw` plus this, MW (the
    /// last step that failed); 0 when the curve didn't stop at a nose.
    pub resolution_mw: f64,
    /// The transfer at which a monitored bus first falls below its low
    /// voltage limit, MW; `None` when none does before the curve stops.
    pub voltage_limit_mw: Option<f64>,
    /// The bus that sets `voltage_limit_mw`.
    pub voltage_limit_bus: Option<u32>,
    /// The bus whose voltage falls fastest at the last step.
    pub weakest_bus: Option<u32>,
    pub weakest_bus_name: String,
    /// What the source and sink could move, as for transfer limits.
    pub source: Participation,
    pub sink: Participation,
    pub source_scaling: Scaling,
    pub sink_scaling: Scaling,
    /// The first step, MW.
    pub step_mw: f64,
    /// Steps solved, and how many of them didn't converge.
    pub solves: usize,
    pub failed_solves: usize,
    /// Monitored buses, and how many of them start below their low limit
    /// (left out of `voltage_limit_mw`).
    pub monitored: usize,
    pub below_at_start: usize,
    pub elapsed_ms: f64,
    pub cancelled: bool,
    /// Things worth knowing about the setup, in words.
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct PvResults {
    pub summary: PvSummary,
    /// Every converged step, the base case first.
    pub points: Vec<PvPoint>,
    /// Curves of the buses asked for, then of the buses whose voltage fell
    /// furthest, furthest first.
    pub curves: Vec<PvCurve>,
}

/// Everything a PV study needs, copied out of the workspace.
pub struct PvJob {
    power: PowerCase,
    base: PowerFlowResult,
    solve: SolveSettings,
    settings: PvSettings,
    handle: RunHandle,
}

/// How one side of the transfer moves the case per MW.
struct Side {
    /// Per-MW change of each online generator's output, by row.
    generators: Vec<(usize, f64)>,
    /// Per-MW change of the scale factor on each bus's load, by row.
    loads: Vec<(usize, f64)>,
}

impl Side {
    fn new(
        case: &Case,
        solved: &Case,
        buses: &HashSet<u32>,
        scaling: Scaling,
        source: bool,
    ) -> (Self, Participation, f64) {
        let (weights, side) = participation(solved, buses, scaling, source);
        let total: f64 = weights.values().sum();
        let sign = if source { 1.0 } else { -1.0 };
        let mut generators = Vec::new();
        let mut loads = Vec::new();
        if total > 0.0 {
            match scaling {
                Scaling::Load => {
                    // Every load in the side scales by the same factor:
                    // `total` MW of load moves by one MW per MW / total.
                    for (r, b) in case.buses.iter().enumerate() {
                        if buses.contains(&b.id) && solved.buses[r].load().0 > 0.0 {
                            loads.push((r, -sign / total));
                        }
                    }
                }
                Scaling::Generation | Scaling::Reserve => {
                    for (r, g) in solved.generators.iter().enumerate() {
                        if !(g.online && buses.contains(&g.bus)) {
                            continue;
                        }
                        let room = if source { g.pmax - g.pg } else { g.pg - g.pmin }.max(0.0);
                        let w = match scaling {
                            Scaling::Reserve => room,
                            _ => g.pg.max(0.0),
                        };
                        if w > 0.0 {
                            generators.push((r, sign * w / total));
                        }
                    }
                }
            }
        }
        (Self { generators, loads }, side, total)
    }

    /// The most MW (or Mvar) one bus moves by per MW of transfer.
    fn largest_share(&self, case: &Case) -> f64 {
        let mut at: HashMap<u32, f64> = HashMap::new();
        for &(r, per) in &self.generators {
            *at.entry(case.generators[r].bus).or_default() += per.abs();
        }
        for &(r, per) in &self.loads {
            let (p, q) = case.buses[r].load_at(1.0);
            *at.entry(case.buses[r].id).or_default() += per.abs() * p.abs().max(q.abs());
        }
        at.values().fold(0.0, |a, &b| a.max(b))
    }

    /// Move `case` (as solved at the base) by `mw` of transfer.
    fn apply(&self, case: &mut Case, base: &Case, mw: f64) {
        for &(r, per) in &self.generators {
            case.generators[r].pg = base.generators[r].pg + per * mw;
        }
        for &(r, per) in &self.loads {
            let k = (1.0 + per * mw).max(0.0);
            let (b, from) = (&mut case.buses[r], &base.buses[r]);
            b.pd = from.pd * k;
            b.qd = from.qd * k;
            b.pd_i = from.pd_i * k;
            b.qd_i = from.qd_i * k;
            b.pd_z = from.pd_z * k;
            b.qd_z = from.qd_z * k;
        }
    }
}

/// One step's solve: the result when it converged, else why not (`None`
/// when cancelled).
fn solve_step(
    case: &Case,
    options: &rs_power::PowerFlowOptions,
) -> Result<PowerFlowResult, Option<String>> {
    match rs_power::solve(case, options) {
        Ok(r) if r.status == SolveStatus::Converged => Ok(r),
        Ok(r) => Err(Some(match r.status {
            SolveStatus::ReactiveLimitsInfeasible => {
                "generator reactive limits leave no bus to hold voltage".to_string()
            }
            _ => format!("no convergence in {} iterations", r.iterations),
        })),
        Err(rs_power::Error::Cancelled) => Err(None),
        Err(e) => Err(Some(e.to_string())),
    }
}

impl PvJob {
    pub fn new(
        power: &PowerCase,
        base: &PowerFlowResult,
        solve: &SolveSettings,
        settings: PvSettings,
    ) -> Result<Self, String> {
        if settings.sink.is_everything() && settings.source.is_everything() {
            return Err("Pick a sink for the PV curve (and a source, or every bus).".into());
        }
        for (name, value) in [
            ("step", settings.step_mw),
            ("smallest step", settings.min_step_mw),
            ("largest transfer", settings.max_transfer_mw),
        ] {
            if value.is_some_and(|v| !(v.is_finite() && v > 0.0)) {
                return Err(format!("The {name} must be more than 0 MW."));
            }
        }
        if settings
            .vmin_pu
            .is_some_and(|v| !(v.is_finite() && v > 0.0))
        {
            return Err("The low voltage limit must be more than 0 p.u.".into());
        }
        Ok(Self {
            power: power.clone(),
            base: base.clone(),
            solve: solve.clone(),
            settings,
            handle: RunHandle::default(),
        })
    }

    pub fn handle(&self) -> RunHandle {
        self.handle.clone()
    }

    pub fn run(self) -> Result<PvResults, String> {
        let start = Instant::now();
        let s = &self.settings;
        let case = &self.power.case;
        let solved = &self.base.case;
        let mut notes = Vec::new();

        let sink_buses = s.sink.bus_ids(&self.power);
        let source_buses = s.source.bus_ids(&self.power);
        if s.source_scaling == s.sink_scaling {
            let overlap = source_buses.intersection(&sink_buses).count();
            if overlap > 0 {
                notes.push(format!(
                    "The source and sink share {overlap} bus{}; what they shift there cancels out.",
                    if overlap == 1 { "" } else { "es" }
                ));
            }
        }
        let (source, source_part, source_total) =
            Side::new(case, solved, &source_buses, s.source_scaling, true);
        let (sink, sink_part, sink_total) =
            Side::new(case, solved, &sink_buses, s.sink_scaling, false);
        let what = |scaling: Scaling| match scaling {
            Scaling::Load => "load",
            Scaling::Reserve => "generation with room to move",
            Scaling::Generation => "online generation",
        };
        if source_total <= 0.0 {
            return Err(format!(
                "The source has no {} to shift.",
                what(s.source_scaling)
            ));
        }
        if sink_total <= 0.0 {
            return Err(format!(
                "The sink has no {} to shift.",
                what(s.sink_scaling)
            ));
        }
        let step0 = s.step_mw.unwrap_or_else(|| (sink_total * 0.05).max(1.0));
        let min_step = s.min_step_mw.unwrap_or(step0 / 32.0).min(step0);
        let generation_cap = if s.respect_generation_limits {
            match (source_part.available_mw, sink_part.available_mw) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        } else {
            None
        };
        let cap = match (generation_cap, s.max_transfer_mw) {
            (Some(g), Some(m)) => Some(g.min(m)),
            (g, m) => g.or(m),
        };
        if cap.is_some_and(|c| c <= 0.0) {
            return Err(
                "The source or sink has no room to move: its generators are at their limits."
                    .into(),
            );
        }
        if self.solve.area_interchange != power::InterchangeChoice::Off {
            notes.push("Area interchange control is on, so area slack generators move against the transfer between areas.".into());
        }

        // Watched buses: energized ones in the monitor subsystem.
        let names = power::bus_names(case);
        let monitored: Vec<usize> = (0..case.buses.len())
            .filter(|&r| {
                case.buses[r].kind != BusType::Isolated
                    && solved.buses[r].vm > 0.0
                    && s.monitor
                        .as_ref()
                        .is_none_or(|m| m.contains(&self.power, r))
            })
            .collect();
        if monitored.is_empty() {
            return Err("No energized bus is in the monitored subsystem.".into());
        }
        let vmin: Vec<f64> = monitored
            .iter()
            .map(|&r| {
                s.vmin_pu.unwrap_or_else(|| {
                    let v = case.buses[r].vmin;
                    if v > 0.0 && v < 1.0 { v } else { 0.9 }
                })
            })
            .collect();
        let row_of: HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let sink_rows: Vec<usize> = case
            .buses
            .iter()
            .enumerate()
            .filter(|(_, b)| sink_buses.contains(&b.id))
            .map(|(r, _)| r)
            .collect();

        let point = |result: &PowerFlowResult, transfer: f64| -> (PvPoint, Vec<f32>) {
            let c = &result.case;
            let vm: Vec<f32> = monitored.iter().map(|&r| c.buses[r].vm as f32).collect();
            let (k, low) = vm
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(k, v)| (k, *v as f64))
                .unwrap_or((0, 0.0));
            let gen_mw: f64 = c.generators.iter().filter(|g| g.online).map(|g| g.pg).sum();
            let load_mw: f64 = c
                .buses
                .iter()
                .filter(|b| b.kind != BusType::Isolated)
                .map(|b| b.load().0 + b.gs * b.vm * b.vm)
                .sum();
            let sink_load_mw = sink_rows.iter().map(|&r| c.buses[r].load().0).sum();
            let mut q_limited = 0;
            let mut q_reserve_mvar = 0.0;
            for g in c.generators.iter().filter(|g| g.online) {
                let span = (g.qmax - g.qmin).abs();
                if span < 1e-6 {
                    continue;
                }
                if g.qg >= g.qmax - 1e-3 * span.max(1.0) || g.qg <= g.qmin + 1e-3 * span.max(1.0) {
                    q_limited += 1;
                }
                q_reserve_mvar += (g.qmax - g.qg).max(0.0);
            }
            (
                PvPoint {
                    transfer_mw: transfer,
                    sink_load_mw,
                    losses_mw: gen_mw - load_mw,
                    min_vm_pu: low,
                    min_vm_bus: case.buses[monitored[k]].id,
                    q_limited,
                    q_reserve_mvar,
                    iterations: result.iterations,
                },
                vm,
            )
        };

        let (p0, v0) = point(&self.base, 0.0);
        let mut points = vec![p0];
        let mut voltages = vec![v0];
        let mut last = self.base.clone();
        let mut transfer = 0.0;
        let mut step = step0;
        let mut solves = 0;
        let mut failed = 0;
        let mut last_error: Option<String> = None;
        let mut resolution = 0.0;
        // A step smaller than the mismatch tolerance would "solve" without
        // moving, so the steps solve to a 20th of the smallest one at most.
        // The mismatch is checked bus by bus, so the step is measured at the
        // bus it moves most.
        let tolerance = self
            .solve
            .options(case.base_mva)
            .tolerance
            .min(power::tolerance_pu(
                min_step * source.largest_share(case).max(sink.largest_share(case)) / 20.0,
                case.base_mva,
            ));
        self.handle.set_phase(2, 0);
        let limited_by = loop {
            if self.handle.cancelled() {
                break "cancelled";
            }
            if solves >= s.max_points {
                break "max_points";
            }
            if cap.is_some_and(|c| transfer >= c - 1e-9) {
                break if generation_cap.is_some_and(|g| transfer >= g - 1e-9) {
                    "generation"
                } else {
                    "max_transfer"
                };
            }
            let next = match cap {
                Some(c) => (transfer + step).min(c),
                None => transfer + step,
            };
            let mut moved = case.clone();
            source.apply(&mut moved, case, next);
            sink.apply(&mut moved, case, next);
            let mut options = self.solve.options_for(&self.power);
            options.cancel = Some(self.handle.cancel_flag());
            options.tolerance = tolerance;
            let warm = power::warm_start(&moved, &last, &mut options);
            // Full Newton, with room to converge slowly near the nose.
            options.method = rs_power::AcMethod::Newton;
            options.max_iterations = options.max_iterations.max(MIN_ITERATIONS);
            solves += 1;
            self.handle.step();
            // From the last step's solution, generators it held at a
            // reactive limit stay held; when that fails, look at every
            // limit afresh from the same voltages before calling it a
            // failed step.
            let mut outcome = solve_step(&warm, &options);
            if matches!(outcome, Err(Some(_))) {
                options.initialization = rs_power::Initialization::Case;
                outcome = solve_step(&warm, &options);
            }
            let outcome = match outcome {
                Ok(r) => Ok(r),
                Err(None) => break "cancelled",
                Err(Some(e)) => Err(e),
            };
            match outcome {
                Ok(result) => {
                    let (p, v) = point(&result, next);
                    points.push(p);
                    voltages.push(v);
                    transfer = next;
                    last = result;
                }
                Err(e) => {
                    failed += 1;
                    last_error = Some(e);
                    if next - transfer <= min_step * (1.0 + 1e-9) {
                        resolution = next - transfer;
                        break "collapse";
                    }
                    step = ((next - transfer) / 2.0).max(min_step);
                }
            }
        };
        let cancelled = limited_by == "cancelled";
        if limited_by == "collapse"
            && let Some(e) = &last_error
        {
            notes.push(format!("The step past the nose didn't solve ({e})."));
        }
        if points.len() == 1 && limited_by == "collapse" {
            notes.push("Not even the first step solved: the case starts at or near its nose, or the step is too large.".into());
        }

        // Each monitored bus: where it crosses its limit, and how far and
        // how fast its voltage falls.
        let n = points.len();
        let transfers: Vec<f64> = points.iter().map(|p| p.transfer_mw).collect();
        let crossing = |k: usize| -> Option<f64> {
            let lim = vmin[k] as f32;
            if voltages[0][k] < lim {
                return None;
            }
            (1..n).find_map(|i| {
                let (a, b) = (voltages[i - 1][k], voltages[i][k]);
                (b < lim).then(|| {
                    let f = ((a - lim) / (a - b)) as f64;
                    transfers[i - 1] + f * (transfers[i] - transfers[i - 1])
                })
            })
        };
        let below_at_start = (0..monitored.len())
            .filter(|&k| voltages[0][k] < vmin[k] as f32)
            .count();
        if below_at_start > 0 {
            notes.push(format!(
                "{below_at_start} monitored bus{} start{} below {} low voltage limit; {} left out of the voltage limit.",
                if below_at_start == 1 { "" } else { "es" },
                if below_at_start == 1 { "s" } else { "" },
                if below_at_start == 1 { "its" } else { "their" },
                if below_at_start == 1 { "it is" } else { "they are" },
            ));
        }
        let mut voltage_limit: Option<(f64, usize)> = None;
        for k in 0..monitored.len() {
            if let Some(t) = crossing(k)
                && voltage_limit.is_none_or(|(best, _)| t < best)
            {
                voltage_limit = Some((t, k));
            }
        }
        let drop = |k: usize| (voltages[0][k] - voltages[n - 1][k]) as f64;
        let sensitivity = |k: usize| {
            if n < 2 {
                return 0.0;
            }
            let dt = transfers[n - 1] - transfers[n - 2];
            if dt <= 0.0 {
                return 0.0;
            }
            (voltages[n - 2][k] - voltages[n - 1][k]) as f64 / dt * 100.0
        };
        let weakest = (n >= 2)
            .then(|| {
                (0..monitored.len()).max_by(|&a, &b| sensitivity(a).total_cmp(&sensitivity(b)))
            })
            .flatten();

        // Curves: buses asked for, then the furthest falls.
        let index: HashMap<usize, usize> =
            monitored.iter().enumerate().map(|(k, &r)| (r, k)).collect();
        let mut chosen: Vec<usize> = Vec::new();
        for id in &s.buses {
            match row_of.get(id).and_then(|r| index.get(r)) {
                Some(&k) if !chosen.contains(&k) => chosen.push(k),
                Some(_) => {}
                None => notes.push(format!(
                    "Bus {id} isn't a monitored, energized bus, so it has no curve."
                )),
            }
        }
        let mut by_drop: Vec<usize> = (0..monitored.len()).collect();
        by_drop.sort_by(|&a, &b| drop(b).total_cmp(&drop(a)).then(a.cmp(&b)));
        for k in by_drop.into_iter().take(s.max_curves) {
            if !chosen.contains(&k) {
                chosen.push(k);
            }
        }
        let curves = chosen
            .into_iter()
            .map(|k| {
                let r = monitored[k];
                PvCurve {
                    bus: case.buses[r].id,
                    name: names[r].clone(),
                    base_kv: case.buses[r].base_kv,
                    vmin_pu: vmin[k],
                    vm_pu: voltages.iter().map(|v| v[k] as f64).collect(),
                    drop_pu: drop(k),
                    sensitivity_pu_per_100mw: sensitivity(k),
                    vmin_transfer_mw: crossing(k),
                }
            })
            .collect();

        let weakest_bus = weakest.map(|k| case.buses[monitored[k]].id);
        let summary = PvSummary {
            nose_mw: transfer,
            limited_by,
            resolution_mw: resolution,
            voltage_limit_mw: voltage_limit.map(|(t, _)| t),
            voltage_limit_bus: voltage_limit.map(|(_, k)| case.buses[monitored[k]].id),
            weakest_bus,
            weakest_bus_name: weakest
                .map(|k| names[monitored[k]].clone())
                .unwrap_or_default(),
            source: source_part,
            sink: sink_part,
            source_scaling: s.source_scaling,
            sink_scaling: s.sink_scaling,
            step_mw: step0,
            solves,
            failed_solves: failed,
            monitored: monitored.len(),
            below_at_start,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            cancelled,
            notes,
        };
        Ok(PvResults {
            summary,
            points,
            curves,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case9() -> (PowerCase, PowerFlowResult, SolveSettings) {
        let text = include_str!("../../rs-power/tests/fixtures/case9.m");
        let case = rs_power::parse_matpower(text).unwrap();
        let power = crate::power::from_case(case);
        let solve = SolveSettings::default();
        let solved = rs_power::solve(&power.case, &solve.options_for(&power)).unwrap();
        (power, solved, solve)
    }

    fn sink(bus: u32) -> PvSettings {
        PvSettings {
            sink: Subsystem {
                buses: vec![bus],
                ..Default::default()
            },
            respect_generation_limits: false,
            ..Default::default()
        }
    }

    #[test]
    fn load_grows_until_the_nose() {
        let (power, solved, solve) = case9();
        let job = PvJob::new(&power, &solved, &solve, sink(5)).unwrap();
        let r = job.run().unwrap();
        let s = &r.summary;
        assert_eq!(s.limited_by, "collapse", "{s:?}");
        assert!(s.nose_mw > 100.0, "{s:?}");
        assert!(s.resolution_mw > 0.0 && s.resolution_mw <= s.step_mw / 32.0 + 1e-9);
        // The sink's load grows by the transfer.
        let first = &r.points[0];
        let last = r.points.last().unwrap();
        assert!((first.sink_load_mw - 90.0).abs() < 1e-6);
        assert!((last.sink_load_mw - (90.0 + last.transfer_mw)).abs() < 1e-3);
        // Voltages fall along the curve and the sink bus falls furthest.
        assert_eq!(r.curves[0].bus, 5);
        let vm = &r.curves[0].vm_pu;
        assert!(vm.windows(2).all(|w| w[1] <= w[0] + 1e-6), "{vm:?}");
        assert!(vm.last().unwrap() < &0.9);
        assert_eq!(s.weakest_bus, Some(5));
        // The bus crosses its limit before the nose.
        let t = s.voltage_limit_mw.unwrap();
        assert!(t > 0.0 && t < s.nose_mw);
        assert_eq!(s.voltage_limit_bus, Some(5));
    }

    #[test]
    fn a_smaller_resolution_finds_the_same_nose() {
        let (power, solved, solve) = case9();
        let coarse = PvJob::new(&power, &solved, &solve, sink(5))
            .unwrap()
            .run()
            .unwrap();
        let fine = PvJob::new(
            &power,
            &solved,
            &solve,
            PvSettings {
                min_step_mw: Some(0.05),
                ..sink(5)
            },
        )
        .unwrap()
        .run()
        .unwrap();
        let (a, b) = (coarse.summary.nose_mw, fine.summary.nose_mw);
        assert!(b >= a - 1e-9, "{a} {b}");
        assert!(b - a <= coarse.summary.resolution_mw + 1e-9, "{a} {b}");
    }

    #[test]
    fn stops_at_the_largest_transfer_asked_for() {
        let (power, solved, solve) = case9();
        let r = PvJob::new(
            &power,
            &solved,
            &solve,
            PvSettings {
                max_transfer_mw: Some(30.0),
                step_mw: Some(7.0),
                ..sink(7)
            },
        )
        .unwrap()
        .run()
        .unwrap();
        assert_eq!(r.summary.limited_by, "max_transfer");
        assert_eq!(r.summary.nose_mw, 30.0);
        let t: Vec<f64> = r.points.iter().map(|p| p.transfer_mw).collect();
        assert_eq!(t, vec![0.0, 7.0, 14.0, 21.0, 28.0, 30.0]);
    }

    #[test]
    fn stops_when_the_source_runs_out_of_generation() {
        let (power, solved, solve) = case9();
        // Generator 3 (bus 3) has 270 MW - 85 MW of room.
        let r = PvJob::new(
            &power,
            &solved,
            &solve,
            PvSettings {
                source: Subsystem {
                    buses: vec![3],
                    ..Default::default()
                },
                source_scaling: Scaling::Reserve,
                respect_generation_limits: true,
                step_mw: Some(50.0),
                ..sink(9)
            },
        )
        .unwrap()
        .run()
        .unwrap();
        let s = &r.summary;
        assert!(
            s.limited_by == "generation" || s.limited_by == "collapse",
            "{s:?}"
        );
        assert!(s.nose_mw <= 185.0 + 1e-9);
        if s.limited_by == "generation" {
            assert!((s.nose_mw - 185.0).abs() < 1e-9);
        }
    }

    /// The nose and the sink voltages along the curve match an independent
    /// arclength continuation and PYPOWER solves
    /// (`scripts/pv_reference.py`), with Q limits off.
    #[test]
    fn matches_an_arclength_continuation() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/pv_reference.json")).unwrap();
        for study in reference["studies"].as_array().unwrap() {
            let name = study["name"].as_str().unwrap();
            let file = study["case"].as_str().unwrap();
            let path = format!(
                "{}/../rs-power/tests/fixtures/{file}",
                env!("CARGO_MANIFEST_DIR")
            );
            let text = std::fs::read_to_string(path).unwrap();
            let power = crate::power::from_case(rs_power::parse_matpower(&text).unwrap());
            let solve = SolveSettings {
                tolerance_mva: 1e-7,
                enforce_q_limits: false,
                method: crate::power::MethodChoice::Newton,
                ..Default::default()
            };
            let solved = rs_power::solve(&power.case, &solve.options_for(&power)).unwrap();
            assert_eq!(solved.status, SolveStatus::Converged, "{name}");
            let ids = |key: &str| -> Vec<u32> {
                study[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u32)
                    .collect()
            };
            let voltages = study["voltages"].as_array().unwrap();
            let sink_ids: Vec<u32> = voltages[0]["vm_pu"]
                .as_object()
                .unwrap()
                .keys()
                .map(|k| k.parse().unwrap())
                .collect();
            // Steps that land on every transfer the reference solved.
            let at: Vec<f64> = voltages
                .iter()
                .map(|v| v["transfer_mw"].as_f64().unwrap())
                .collect();
            let step = at.iter().fold(at[0], |g, &t| gcd(g, t));
            let settings = PvSettings {
                sink: Subsystem {
                    buses: ids("sink_buses"),
                    areas: ids("sink_areas"),
                    ..Default::default()
                },
                respect_generation_limits: false,
                step_mw: Some(step),
                min_step_mw: Some(0.01),
                buses: sink_ids.clone(),
                max_curves: 0,
                max_points: 2000,
                ..Default::default()
            };
            let r = PvJob::new(&power, &solved, &solve, settings)
                .unwrap()
                .run()
                .unwrap();
            assert_eq!(r.summary.limited_by, "collapse", "{name}");
            let mut nose = stannum_verify::Check::new(
                "pv",
                &format!("{name}: nose"),
                "Transfer at the nose",
                "MW",
                0.05,
            );
            nose.compare(r.summary.nose_mw, study["nose_mw"].as_f64().unwrap());
            nose.finish();
            let mut vm = stannum_verify::Check::new(
                "pv",
                &format!("{name}: sink voltages along the curve"),
                "Voltage magnitude",
                "p.u.",
                1e-6,
            );
            for v in voltages {
                let t = v["transfer_mw"].as_f64().unwrap();
                let i = r
                    .points
                    .iter()
                    .position(|p| (p.transfer_mw - t).abs() < 1e-9)
                    .unwrap_or_else(|| panic!("{name}: no point at {t} MW"));
                for (bus, expected) in v["vm_pu"].as_object().unwrap() {
                    let bus: u32 = bus.parse().unwrap();
                    let curve = r.curves.iter().find(|c| c.bus == bus).unwrap();
                    vm.compare(curve.vm_pu[i], expected.as_f64().unwrap());
                }
            }
            vm.finish();
        }
    }

    fn gcd(a: f64, b: f64) -> f64 {
        let (mut a, mut b) = (a.round() as u64, b.round() as u64);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a as f64
    }

    #[test]
    fn refuses_settings_without_a_sink() {
        let (power, solved, solve) = case9();
        assert!(PvJob::new(&power, &solved, &solve, PvSettings::default()).is_err());
        assert!(
            PvJob::new(
                &power,
                &solved,
                &solve,
                PvSettings {
                    step_mw: Some(0.0),
                    ..sink(5)
                }
            )
            .is_err()
        );
    }
}
