//! QV curves: how much reactive power a bus can give up before its voltage
//! collapses.
//!
//! A fictitious synchronous condenser, with no active power and no reactive
//! limits, holds the study bus at one voltage after another, from the
//! voltage the last AC solve left it at up to a high voltage and down to a
//! low one. Each set point is solved with the full AC power flow, starting
//! from the one before, with every control the last solve used. The
//! condenser's reactive output at each voltage is the curve: what the bus
//! would need injected to sit there. At the solved voltage it is zero. Lower
//! voltages need the condenser to absorb, down to the bottom of the curve;
//! past it the network can't keep the bus up. The depth of the bottom is
//! the bus's reactive power margin: how much more reactive load the bus can
//! take before its voltage collapses. A bottom above zero means the bus
//! can't hold its voltage without the condenser at all.
//!
//! This is how PSS/E's QV analysis draws the curve. Controls that hold the
//! study bus itself stay where the last solve left them (the condenser
//! holds it now), generators on the bus keep their reactive output, and
//! generators elsewhere that held the bus hold their own terminal instead.

use crate::contingency::RunHandle;
use crate::power::{self, PowerCase, SolveSettings};
use crate::pv::{MIN_ITERATIONS, solve_step};
use rs_power::{BusType, Case, Generator, PowerFlowResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// The most buses one study sweeps.
pub const MAX_BUSES: usize = 100;

fn default_high() -> f64 {
    1.1
}
fn default_low() -> f64 {
    0.3
}
fn default_step() -> f64 {
    0.01
}
fn default_max_points() -> usize {
    300
}

/// A QV curve study.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct QvSettings {
    /// Buses to draw a curve for, each swept on its own.
    pub buses: Vec<u32>,
    /// The highest voltage set, p.u.
    #[serde(default = "default_high")]
    pub v_high_pu: f64,
    /// The lowest voltage tried, p.u. The sweep usually stops before it,
    /// when the voltage is too low to solve.
    #[serde(default = "default_low")]
    pub v_low_pu: f64,
    /// The voltage step, p.u. The steps land on multiples of it.
    #[serde(default = "default_step")]
    pub step_pu: f64,
    /// The smallest step tried before the sweep down stops, p.u. An 8th of
    /// the step when absent.
    #[serde(default)]
    pub min_step_pu: Option<f64>,
    /// The most solves for one bus, failed ones included.
    #[serde(default = "default_max_points")]
    pub max_points: usize,
}

impl Default for QvSettings {
    fn default() -> Self {
        Self {
            buses: Vec::new(),
            v_high_pu: default_high(),
            v_low_pu: default_low(),
            step_pu: default_step(),
            min_step_pu: None,
            max_points: default_max_points(),
        }
    }
}

/// One bus's curve: the condenser's reactive output at each voltage.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct QvCurve {
    pub bus: u32,
    pub name: String,
    pub base_kv: f64,
    /// The bus voltage in the last AC solve, p.u., where the condenser
    /// gives nothing.
    pub base_vm_pu: f64,
    /// Voltage set at each point, p.u., lowest first.
    pub v_pu: Vec<f64>,
    /// The condenser's reactive output at each point, Mvar: positive when
    /// it injects (the bus needs support to sit at that voltage), negative
    /// when it absorbs.
    pub q_mvar: Vec<f64>,
    /// Online generators elsewhere held at a reactive limit at each point.
    pub q_limited: Vec<usize>,
    /// The bottom of the curve: the least reactive output, Mvar, and the
    /// voltage there, p.u.
    pub q_min_mvar: f64,
    pub v_at_q_min_pu: f64,
    /// Reactive power margin, Mvar: how far the bottom lies below zero.
    /// Negative when the bottom is above zero, so the bus can't hold its
    /// voltage unaided.
    pub margin_mvar: f64,
    /// False when the curve was still falling at its lowest point, so the
    /// true bottom is lower and the margin at least `margin_mvar`.
    pub bottom_found: bool,
    /// What stopped the sweep down.
    #[schemars(extend("enum" = ["collapse", "v_low", "max_points", "cancelled", "failed"]))]
    pub limited_by: &'static str,
    /// Solves for this bus, and how many didn't converge.
    pub solves: usize,
    pub failed_solves: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct QvSummary {
    /// Buses with a curve.
    pub buses: usize,
    /// The bus with the smallest reactive power margin, and that margin.
    pub weakest_bus: Option<u32>,
    pub weakest_bus_name: String,
    pub weakest_margin_mvar: Option<f64>,
    pub v_high_pu: f64,
    pub v_low_pu: f64,
    pub step_pu: f64,
    pub min_step_pu: f64,
    pub solves: usize,
    pub failed_solves: usize,
    pub elapsed_ms: f64,
    pub cancelled: bool,
    /// Things worth knowing about the setup, in words.
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct QvResults {
    pub summary: QvSummary,
    /// One curve per bus, in the order asked for.
    pub curves: Vec<QvCurve>,
}

/// Everything a QV study needs, copied out of the workspace.
pub struct QvJob {
    power: PowerCase,
    base: PowerFlowResult,
    solve: SolveSettings,
    settings: QvSettings,
    handle: RunHandle,
}

/// One bus's case, ready to sweep.
struct Prepared {
    case: Case,
    /// Row of the condenser in `case.generators`.
    condenser: usize,
    /// Which switched shunts and taps keep controlling (the rest held the
    /// study bus, and stay where the last solve left them).
    shunts: Vec<bool>,
    taps: Vec<bool>,
}

fn keep<T: Clone>(items: &[T], mask: &[bool]) -> Vec<T> {
    if items.len() != mask.len() {
        return items.to_vec();
    }
    items
        .iter()
        .zip(mask)
        .filter(|(_, k)| **k)
        .map(|(x, _)| x.clone())
        .collect()
}

/// True when no generator bus held at a reactive limit sits on the side
/// of its set point that would take it off the limit: below the set point
/// at its lower limit, or above it at its upper one.
fn limits_hold(r: &PowerFlowResult) -> bool {
    let c = &r.case;
    let row: HashMap<u32, usize> = c.buses.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let mut at: HashMap<u32, (bool, bool, f64)> = HashMap::new();
    for g in c.generators.iter().filter(|g| g.online) {
        let span = (g.qmax - g.qmin).abs();
        if span < 1e-6 || !span.is_finite() || g.regulated_bus.is_some_and(|b| b != g.bus) {
            continue;
        }
        let tol = 1e-4 * span.max(1.0);
        let e = at.entry(g.bus).or_insert((true, true, g.vg));
        e.0 &= g.qg >= g.qmax - tol;
        e.1 &= g.qg <= g.qmin + tol;
    }
    at.iter().all(|(bus, &(upper, lower, vg))| {
        let Some(&i) = row.get(bus) else { return true };
        let vm = c.buses[i].vm;
        !(upper && vm > vg + 1e-4) && !(lower && vm < vg - 1e-4)
    })
}

impl QvJob {
    pub fn new(
        power: &PowerCase,
        base: &PowerFlowResult,
        solve: &SolveSettings,
        settings: QvSettings,
    ) -> Result<Self, String> {
        let s = &settings;
        if s.buses.is_empty() {
            return Err("Pick at least one bus for the QV curve.".into());
        }
        if s.buses.len() > MAX_BUSES {
            return Err(format!(
                "A QV study sweeps {MAX_BUSES} buses at most; {} were asked for.",
                s.buses.len()
            ));
        }
        for (name, value) in [
            ("high voltage", Some(s.v_high_pu)),
            ("low voltage", Some(s.v_low_pu)),
            ("voltage step", Some(s.step_pu)),
            ("smallest step", s.min_step_pu),
        ] {
            if value.is_some_and(|v| !(v.is_finite() && v > 0.0)) {
                return Err(format!("The {name} must be more than 0 p.u."));
            }
        }
        if s.v_low_pu >= s.v_high_pu {
            return Err("The low voltage must be below the high voltage.".into());
        }
        if s.max_points == 0 {
            return Err("Allow at least one solve per bus.".into());
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

    /// The case with a condenser holding bus `row`, the generators and
    /// controls that held it set aside.
    fn prepare(&self, row: usize) -> Prepared {
        let mut case = self.power.case.clone();
        let solved = &self.base.case;
        let id = case.buses[row].id;
        let index: HashMap<u32, usize> = solved
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        for (g, s) in case.generators.iter_mut().zip(&solved.generators) {
            if !g.online {
                continue;
            }
            if g.bus == id {
                // Generators on the bus keep the output they had, so the
                // condenser alone holds the voltage.
                let q = if s.qg.is_finite() { s.qg } else { g.qg };
                g.qg = q;
                g.qmax = q;
                g.qmin = q;
                g.regulated_bus = None;
            } else if g.regulated_bus == Some(id) {
                // Generators that held the bus from elsewhere hold their own
                // terminal at the voltage it had.
                g.regulated_bus = None;
                if let Some(&i) = index.get(&g.bus) {
                    g.vg = solved.buses[i].vm;
                }
            }
        }
        case.buses[row].kind = BusType::Pv;
        let vm = solved.buses[row].vm;
        case.generators.push(Generator {
            bus: id,
            pg: 0.0,
            qg: 0.0,
            qmax: f64::INFINITY,
            qmin: f64::NEG_INFINITY,
            vg: vm,
            mbase: case.base_mva,
            online: true,
            pmax: 0.0,
            pmin: 0.0,
            extra: Vec::new(),
            regulated_bus: None,
            reactive_share: 100.0,
        });
        let condenser = case.generators.len() - 1;

        // Switched shunts and taps that held the bus stay where the last
        // solve left them.
        let controls = &self.power.voltage_controls;
        let held = |bus: u32, regulated: Option<u32>| regulated.unwrap_or(bus) == id;
        let shunts: Vec<bool> = controls
            .switched_shunts
            .iter()
            .map(|c| !held(c.bus, c.regulated_bus))
            .collect();
        let taps: Vec<bool> = controls
            .taps
            .iter()
            .map(|t| t.regulated_bus != id)
            .collect();
        let shunt_rows: HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        if self.base.switched_shunts.len() == shunts.len() {
            for ((c, &value), &k) in controls
                .switched_shunts
                .iter()
                .zip(&self.base.switched_shunts)
                .zip(&shunts)
            {
                if !k
                    && value.is_finite()
                    && let Some(&i) = shunt_rows.get(&c.bus)
                {
                    case.buses[i].bs += value - c.initial;
                }
            }
        }
        if self.base.tap_ratios.len() == taps.len() {
            for ((t, &ratio), &k) in controls.taps.iter().zip(&self.base.tap_ratios).zip(&taps) {
                if !k && ratio.is_finite() && t.branch < case.branches.len() {
                    case.branches[t.branch].tap = t.tap_per_ratio * ratio;
                }
            }
        }
        Prepared {
            case,
            condenser,
            shunts,
            taps,
        }
    }

    /// The last solve, with only the controls that keep controlling.
    fn trimmed(result: &PowerFlowResult, p: &Prepared) -> PowerFlowResult {
        let mut r = result.clone();
        r.switched_shunts = keep(&r.switched_shunts, &p.shunts);
        r.tap_ratios = keep(&r.tap_ratios, &p.taps);
        r
    }

    /// Solve the prepared case with the condenser holding `v`, from `last`.
    fn solve_at(
        &self,
        p: &Prepared,
        last: &PowerFlowResult,
        v: f64,
        tolerance: f64,
    ) -> Result<PowerFlowResult, Option<String>> {
        let mut case = p.case.clone();
        case.generators[p.condenser].vg = v;
        let mut options = self.solve.options_for(&self.power);
        let controls = &mut options.voltage_controls;
        controls.switched_shunts = keep(&controls.switched_shunts, &p.shunts);
        controls.taps = keep(&controls.taps, &p.taps);
        options.cancel = Some(self.handle.cancel_flag());
        options.tolerance = tolerance;
        let warm = power::warm_start(&case, last, &mut options);
        options.method = rs_power::AcMethod::Newton;
        options.max_iterations = options.max_iterations.max(MIN_ITERATIONS);
        // Generators the last step held at a reactive limit start there,
        // which saves finding the same limits again at every step. When
        // that leaves one on the wrong side of its set point (it should
        // have come off the limit), or doesn't solve, every limit is looked
        // at afresh.
        if options.enforce_q_limits {
            options.initialization = rs_power::Initialization::Solution;
            if let Ok(r) = solve_step(&warm, &options)
                && limits_hold(&r)
            {
                return Ok(r);
            }
        }
        options.initialization = rs_power::Initialization::Case;
        solve_step(&warm, &options)
    }

    pub fn run(self) -> Result<QvResults, String> {
        let start = Instant::now();
        let s = &self.settings;
        let case = &self.power.case;
        let names = power::bus_names(case);
        let min_step = s.min_step_pu.unwrap_or(s.step_pu / 8.0).min(s.step_pu);
        // Reactive output to a tenth of an Mvar at least, so that the curve
        // is smooth near its bottom, where it is flat.
        let tolerance = self
            .solve
            .options(case.base_mva)
            .tolerance
            .min(power::tolerance_pu(0.1, case.base_mva));
        let mut notes = Vec::new();
        if self.solve.area_interchange != power::InterchangeChoice::Off {
            notes.push("Area interchange control is on, so area slack generators pick up the condenser's losses.".into());
        }
        let row_of: HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let mut rows = Vec::new();
        for &id in &s.buses {
            match row_of.get(&id) {
                None => notes.push(format!("Bus {id} isn't in the case.")),
                Some(_) if rows.iter().any(|&(b, _)| b == id) => {}
                Some(&r) => {
                    let b = &case.buses[r];
                    if b.kind == BusType::Isolated
                        || self.base.case.buses[r].vm.is_nan()
                        || self.base.case.buses[r].vm <= 0.0
                    {
                        notes.push(format!("Bus {id} is out of service, so it has no curve."));
                    } else if b.kind == BusType::Reference {
                        notes.push(format!(
                            "Bus {id} is a reference bus: its voltage is already held, so it has no curve."
                        ));
                    } else {
                        rows.push((id, r));
                    }
                }
            }
        }
        if rows.is_empty() {
            return Err(format!(
                "None of the buses can have a QV curve. {}",
                notes.join(" ")
            ));
        }

        self.handle.set_phase(2, rows.len());
        let mut curves = Vec::new();
        let mut solves = 0;
        let mut failed = 0;
        let mut cancelled = false;
        for &(id, row) in &rows {
            if self.handle.cancelled() {
                cancelled = true;
                break;
            }
            let p = self.prepare(row);
            let base_vm = self.base.case.buses[row].vm;
            let mut points: Vec<(f64, f64, usize)> = Vec::new();
            let mut bus_solves = 0;
            let mut bus_failed = 0;
            let record = |r: &PowerFlowResult, v: f64, points: &mut Vec<(f64, f64, usize)>| {
                let q = r.case.generators[p.condenser].qg;
                let limited = r
                    .case
                    .generators
                    .iter()
                    .enumerate()
                    .filter(|&(g, x)| {
                        let span = (x.qmax - x.qmin).abs();
                        g != p.condenser
                            && x.online
                            && x.bus != id
                            && span > 1e-6
                            && (x.qg >= x.qmax - 1e-3 * span.max(1.0)
                                || x.qg <= x.qmin + 1e-3 * span.max(1.0))
                    })
                    .count();
                points.push((v, q, limited));
            };

            // The base voltage first: the condenser gives nothing there.
            let first = Self::trimmed(&self.base, &p);
            bus_solves += 1;
            let at_base = match self.solve_at(&p, &first, base_vm, tolerance) {
                Ok(r) => r,
                Err(None) => {
                    cancelled = true;
                    break;
                }
                Err(Some(e)) => {
                    bus_failed += 1;
                    notes.push(format!(
                        "Bus {id}: the case didn't solve with the condenser at the bus's own voltage ({e})."
                    ));
                    solves += bus_solves;
                    failed += bus_failed;
                    curves.push(QvCurve {
                        bus: id,
                        name: names[row].clone(),
                        base_kv: case.buses[row].base_kv,
                        base_vm_pu: base_vm,
                        v_pu: Vec::new(),
                        q_mvar: Vec::new(),
                        q_limited: Vec::new(),
                        q_min_mvar: 0.0,
                        v_at_q_min_pu: base_vm,
                        margin_mvar: 0.0,
                        bottom_found: false,
                        limited_by: "failed",
                        solves: bus_solves,
                        failed_solves: bus_failed,
                    });
                    self.handle.step();
                    continue;
                }
            };
            record(&at_base, base_vm, &mut points);

            // Up, then down, each from the base voltage. Steps land on
            // multiples of the step, the first one at least a quarter step
            // from the base voltage.
            let mut down_reason = "v_low";
            for up in [true, false] {
                let limit = if up { s.v_high_pu } else { s.v_low_pu };
                let mut last = at_base.clone();
                let mut v = base_vm;
                let mut step = s.step_pu;
                let reason = loop {
                    if self.handle.cancelled() {
                        break "cancelled";
                    }
                    if (up && v >= limit - 1e-9) || (!up && v <= limit + 1e-9) {
                        break "v_low";
                    }
                    if bus_solves >= s.max_points {
                        break "max_points";
                    }
                    let next = if step == s.step_pu {
                        let k = v / step;
                        let grid = if up {
                            (k + 0.25).floor() + 1.0
                        } else {
                            (k - 0.25).ceil() - 1.0
                        };
                        grid * step
                    } else if up {
                        v + step
                    } else {
                        v - step
                    };
                    let next = if up { next.min(limit) } else { next.max(limit) };
                    bus_solves += 1;
                    match self.solve_at(&p, &last, next, tolerance) {
                        Ok(r) => {
                            record(&r, next, &mut points);
                            v = next;
                            last = r;
                        }
                        Err(None) => break "cancelled",
                        Err(Some(_)) => {
                            bus_failed += 1;
                            if (next - v).abs() <= min_step * (1.0 + 1e-9) {
                                break "collapse";
                            }
                            step = ((next - v).abs() / 2.0).max(min_step);
                        }
                    }
                };
                if reason == "cancelled" {
                    cancelled = true;
                }
                if !up {
                    down_reason = reason;
                } else if reason == "collapse" {
                    notes.push(format!(
                        "Bus {id}: the sweep up stopped at {:.3} p.u.; higher voltages didn't solve.",
                        v
                    ));
                }
                if cancelled {
                    break;
                }
            }
            solves += bus_solves;
            failed += bus_failed;
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (k, &(v_min, q_min, _)) = points
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
                .expect("the base point");
            curves.push(QvCurve {
                bus: id,
                name: names[row].clone(),
                base_kv: case.buses[row].base_kv,
                base_vm_pu: base_vm,
                v_pu: points.iter().map(|x| x.0).collect(),
                q_mvar: points.iter().map(|x| x.1).collect(),
                q_limited: points.iter().map(|x| x.2).collect(),
                q_min_mvar: q_min,
                v_at_q_min_pu: v_min,
                margin_mvar: -q_min,
                bottom_found: k > 0,
                limited_by: if cancelled { "cancelled" } else { down_reason },
                solves: bus_solves,
                failed_solves: bus_failed,
            });
            self.handle.step();
            if cancelled {
                break;
            }
        }
        for c in &curves {
            if c.limited_by != "failed" && !c.bottom_found {
                notes.push(format!(
                    "Bus {}: the curve was still falling at {:.3} p.u., so its margin is at least {:.1} Mvar; set a lower low voltage to find the bottom.",
                    c.bus, c.v_at_q_min_pu, c.margin_mvar
                ));
            }
        }
        let weakest = curves
            .iter()
            .filter(|c| c.limited_by != "failed")
            .min_by(|a, b| a.margin_mvar.total_cmp(&b.margin_mvar));
        let summary = QvSummary {
            buses: curves.len(),
            weakest_bus: weakest.map(|c| c.bus),
            weakest_bus_name: weakest.map(|c| c.name.clone()).unwrap_or_default(),
            weakest_margin_mvar: weakest.map(|c| c.margin_mvar),
            v_high_pu: s.v_high_pu,
            v_low_pu: s.v_low_pu,
            step_pu: s.step_pu,
            min_step_pu: min_step,
            solves,
            failed_solves: failed,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            cancelled,
            notes,
        };
        Ok(QvResults { summary, curves })
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

    fn buses(ids: &[u32]) -> QvSettings {
        QvSettings {
            buses: ids.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn a_load_bus_has_a_bottom_below_zero() {
        let (power, solved, solve) = case9();
        let r = QvJob::new(&power, &solved, &solve, buses(&[5]))
            .unwrap()
            .run()
            .unwrap();
        let c = &r.curves[0];
        assert_eq!(c.bus, 5);
        // Nothing needed at the bus's own voltage.
        let i = c
            .v_pu
            .iter()
            .position(|&v| (v - c.base_vm_pu).abs() < 1e-12)
            .unwrap();
        assert!(c.q_mvar[i].abs() < 0.5, "{c:?}");
        // Higher voltages need injection; lower ones absorb, to a bottom.
        assert!(c.q_mvar.last().unwrap() > &0.0);
        assert!(c.bottom_found, "{c:?}");
        assert!(c.margin_mvar > 50.0, "{c:?}");
        assert!(c.v_at_q_min_pu < c.base_vm_pu);
        assert!(c.v_pu.windows(2).all(|w| w[1] > w[0]));
        // Steps land on the grid until they halve near the end.
        if c.failed_solves == 0 {
            for &v in &c.v_pu {
                let on_grid = ((v / 0.01).round() * 0.01 - v).abs() < 1e-9;
                assert!(on_grid || (v - c.base_vm_pu).abs() < 1e-12, "{v}");
            }
        }
        assert_eq!(r.summary.weakest_bus, Some(5));
        assert!(r.summary.solves >= c.v_pu.len());
    }

    /// The curve matches PYPOWER solves with a generator added at the bus
    /// (`scripts/qv_reference.py`), with Q limits off.
    #[test]
    fn matches_pypower_with_a_condenser_added() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/qv_reference.json")).unwrap();
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
            let bus = study["bus"].as_u64().unwrap() as u32;
            let r = QvJob::new(&power, &solved, &solve, buses(&[bus]))
                .unwrap()
                .run()
                .unwrap();
            let c = &r.curves[0];
            let mut q = stannum_verify::Check::new(
                "qv",
                &format!("{name}: condenser output along the curve"),
                "Reactive power",
                "Mvar",
                1e-4,
            );
            let points = study["points"].as_array().unwrap();
            for p in points {
                let v = p["v_pu"].as_f64().unwrap();
                let i = c
                    .v_pu
                    .iter()
                    .position(|x| (x - v).abs() < 1e-9)
                    .unwrap_or_else(|| panic!("{name}: no point at {v} p.u."));
                q.compare(c.q_mvar[i], p["q_mvar"].as_f64().unwrap());
            }
            q.finish();
            assert_eq!(c.v_pu.len(), points.len() + 1, "{name}");
            let mut margin = stannum_verify::Check::new(
                "qv",
                &format!("{name}: reactive power margin"),
                "Reactive power",
                "Mvar",
                1e-4,
            );
            margin.compare(c.margin_mvar, -study["q_min_mvar"].as_f64().unwrap());
            margin.finish();
            assert!((c.v_at_q_min_pu - study["v_at_q_min_pu"].as_f64().unwrap()).abs() < 1e-9);
        }
    }

    #[test]
    fn a_generator_bus_keeps_its_output() {
        let (power, solved, solve) = case9();
        let r = QvJob::new(&power, &solved, &solve, buses(&[2]))
            .unwrap()
            .run()
            .unwrap();
        let c = &r.curves[0];
        assert!(!c.v_pu.is_empty());
        // At its own voltage the condenser gives nothing: the generator
        // keeps its output.
        let i = c
            .v_pu
            .iter()
            .position(|&v| (v - c.base_vm_pu).abs() < 1e-12)
            .unwrap();
        assert!(c.q_mvar[i].abs() < 0.5, "{c:?}");
    }

    #[test]
    fn the_weakest_bus_has_the_smallest_margin() {
        let (power, solved, solve) = case9();
        let r = QvJob::new(&power, &solved, &solve, buses(&[5, 7, 9]))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(r.curves.len(), 3);
        let least = r
            .curves
            .iter()
            .map(|c| c.margin_mvar)
            .fold(f64::INFINITY, f64::min);
        assert_eq!(r.summary.weakest_margin_mvar, Some(least));
    }

    #[test]
    fn leaves_out_the_reference_bus_and_unknown_buses() {
        let (power, solved, solve) = case9();
        let r = QvJob::new(&power, &solved, &solve, buses(&[1, 99, 5]))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(r.curves.len(), 1);
        assert_eq!(r.summary.notes.len(), 2, "{:?}", r.summary.notes);
        assert!(
            QvJob::new(&power, &solved, &solve, buses(&[1]))
                .unwrap()
                .run()
                .is_err()
        );
    }

    #[test]
    fn refuses_bad_settings() {
        let (power, solved, solve) = case9();
        assert!(QvJob::new(&power, &solved, &solve, QvSettings::default()).is_err());
        for bad in [
            QvSettings {
                step_pu: 0.0,
                ..buses(&[5])
            },
            QvSettings {
                v_low_pu: 1.2,
                ..buses(&[5])
            },
        ] {
            assert!(QvJob::new(&power, &solved, &solve, bad).is_err());
        }
    }
}
