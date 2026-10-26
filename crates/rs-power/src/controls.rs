//! Voltage controls applied between Newton solves: switched shunts and
//! transformer tap changers (PSS/E switched shunt MODSW 1–2, winding COD 1).

/// Voltage controls to apply during an AC solve, usually from
/// [`crate::RawImport::voltage_controls`]. Empty by default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoltageControls {
    /// Switched shunts that hold a bus voltage.
    pub switched_shunts: Vec<SwitchedShunt>,
    /// Transformer windings whose taps hold a bus voltage.
    pub taps: Vec<TapControl>,
}

impl VoltageControls {
    /// True when there is nothing to control.
    pub fn is_empty(&self) -> bool {
        self.switched_shunts.is_empty() && self.taps.is_empty()
    }
}

/// How a switched shunt moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShuntMode {
    /// Whole blocks switch in and out to keep the voltage within
    /// `vmin..=vmax` (PSS/E MODSW 1).
    Discrete,
    /// The susceptance moves smoothly within the blocks' range to hold the
    /// voltage at the middle of the band, like a static var compensator
    /// (PSS/E MODSW 2).
    Continuous,
}

/// A switched shunt in voltage control.
///
/// Its starting susceptance `initial` is already part of the bus's `bs`
/// (as in MATPOWER); the solver moves `bs` by the change from it.
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchedShunt {
    /// External ID of the bus the shunt is connected to.
    pub bus: u32,
    /// Discrete or continuous control.
    pub mode: ShuntMode,
    /// Bus whose voltage is controlled when it is not `bus` (PSS/E SWREM).
    pub regulated_bus: Option<u32>,
    /// Upper voltage limit of the band, in per unit.
    pub vmax: f64,
    /// Lower voltage limit of the band, in per unit.
    pub vmin: f64,
    /// Starting susceptance, MVAr injected at 1 p.u. (BINIT).
    pub initial: f64,
    /// Blocks as (number of steps, MVAr per step at 1 p.u.); capacitors are
    /// positive and reactors negative.
    pub blocks: Vec<(u32, f64)>,
    /// One-based source line of the RAW record, for messages.
    pub line: usize,
}

impl SwitchedShunt {
    /// Every susceptance (MVAr at 1 p.u.) the blocks can make, in increasing
    /// order: reactor steps below zero, capacitor steps above.
    pub fn levels(&self) -> Vec<f64> {
        let mut levels = vec![0.0];
        for sign in [-1.0, 1.0] {
            let mut total = 0.0;
            for &(n, b) in &self.blocks {
                if b * sign > 0.0 {
                    for _ in 0..n {
                        total += b;
                        levels.push(total);
                    }
                }
            }
        }
        levels.sort_by(f64::total_cmp);
        levels
    }
}

/// A transformer winding whose tap holds a bus voltage within a band.
#[derive(Clone, Debug, PartialEq)]
pub struct TapControl {
    /// Index of the winding's entry in `case.branches`.
    pub branch: usize,
    /// External ID of the controlled bus (PSS/E CONT).
    pub regulated_bus: u32,
    /// Whether raising the ratio raises the controlled bus's voltage: true
    /// when that bus is on the winding's own side, false when it is on the
    /// far side.
    pub raises_voltage: bool,
    /// Upper voltage limit of the band, in per unit (VMA).
    pub vmax: f64,
    /// Lower voltage limit of the band, in per unit (VMI).
    pub vmin: f64,
    /// Starting winding ratio in the record's units (WINDV).
    pub ratio: f64,
    /// Highest winding ratio (RMA).
    pub ratio_max: f64,
    /// Lowest winding ratio (RMI).
    pub ratio_min: f64,
    /// Tap positions from `ratio_min` to `ratio_max` (NTP), at least 2.
    pub positions: u32,
    /// The branch's off-nominal tap per unit of winding ratio, so that
    /// `branch.tap = tap_per_ratio * ratio`.
    pub tap_per_ratio: f64,
    /// One-based source line of the winding record, for messages.
    pub line: usize,
}

impl TapControl {
    /// Ratio change of one tap step.
    pub fn step(&self) -> f64 {
        (self.ratio_max - self.ratio_min) / f64::from(self.positions.max(2) - 1)
    }
}

// ---------------------------------------------------------------------------
// Solver side.

use crate::{BusType, Case, Error, Generator, network::Network};
use num_complex::Complex64 as C;

/// Controls resolved against a working case before solving.
pub(crate) struct Setup<'a> {
    /// Discrete shunts, with their index in [`VoltageControls::switched_shunts`].
    pub discrete: Vec<(usize, &'a SwitchedShunt)>,
    /// Continuous shunts and the helper generator standing in for each.
    pub continuous: Vec<(usize, &'a SwitchedShunt, usize)>,
    /// Tap controls, with their index in [`VoltageControls::taps`].
    pub taps: Vec<(usize, &'a TapControl)>,
    /// Generators from this index on are continuous-shunt helpers, which
    /// always keep to their reactive limits.
    pub limited_from: usize,
    /// Number of listed shunts and taps.
    pub counts: [usize; 2],
}

impl Setup<'_> {
    pub fn none(case: &Case) -> Setup<'static> {
        Setup {
            discrete: Vec::new(),
            continuous: Vec::new(),
            taps: Vec::new(),
            limited_from: case.generators.len(),
            counts: [0, 0],
        }
    }
}

/// Validate `controls` against `case` and add a helper generator for every
/// continuous shunt, which holds its voltage within the shunt's range like a
/// static var compensator. Shunts at buses that already have an online
/// generator, or that are offline, stay at their starting value.
pub(crate) fn setup<'a>(
    case: &mut Case,
    controls: &'a VoltageControls,
) -> Result<Setup<'a>, Error> {
    let index: std::collections::HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let known = |id: u32, what: &str| {
        index
            .get(&id)
            .copied()
            .ok_or_else(|| Error::InvalidCase(format!("{what} references unknown bus {id}")))
    };
    let mut generated = vec![false; case.buses.len()];
    for g in case.generators.iter().filter(|g| g.online) {
        if let Some(&i) = index.get(&g.bus) {
            generated[i] = true;
        }
    }
    let limited_from = case.generators.len();
    let mut setup = Setup {
        discrete: Vec::new(),
        continuous: Vec::new(),
        taps: Vec::new(),
        limited_from,
        counts: [controls.switched_shunts.len(), controls.taps.len()],
    };
    for (k, shunt) in controls.switched_shunts.iter().enumerate() {
        let i = known(shunt.bus, "switched shunt")?;
        if let Some(r) = shunt.regulated_bus {
            known(r, "switched shunt")?;
        }
        if !(shunt.vmin > 0.0 && shunt.vmax >= shunt.vmin && shunt.initial.is_finite())
            || shunt.blocks.iter().any(|b| !b.1.is_finite())
        {
            return Err(Error::InvalidCase(format!(
                "switched shunt at bus {} has an invalid band or block",
                shunt.bus
            )));
        }
        match shunt.mode {
            ShuntMode::Discrete => setup.discrete.push((k, shunt)),
            ShuntMode::Continuous => {
                let bus = &mut case.buses[i];
                if generated[i] || bus.kind == BusType::Isolated {
                    continue;
                }
                let levels = shunt.levels();
                let (lo, hi) = (levels[0], levels[levels.len() - 1]);
                let vg = 0.5 * (shunt.vmax + shunt.vmin);
                if bus.kind == BusType::Pq {
                    bus.kind = BusType::Pv;
                }
                generated[i] = true;
                setup.continuous.push((k, shunt, case.generators.len()));
                case.generators.push(Generator {
                    bus: shunt.bus,
                    pg: 0.0,
                    qg: 0.0,
                    qmax: (hi - shunt.initial) * vg * vg,
                    qmin: (lo - shunt.initial) * vg * vg,
                    vg,
                    mbase: 100.0,
                    online: true,
                    pmax: 0.0,
                    pmin: 0.0,
                    extra: Vec::new(),
                    regulated_bus: shunt.regulated_bus,
                    reactive_share: 100.0,
                });
            }
        }
    }
    for (k, tap) in controls.taps.iter().enumerate() {
        known(tap.regulated_bus, "tap control")?;
        if tap.branch >= case.branches.len()
            || !(tap.vmin > 0.0 && tap.vmax >= tap.vmin)
            || !(tap.ratio_max > tap.ratio_min && tap.ratio > 0.0 && tap.tap_per_ratio > 0.0)
        {
            return Err(Error::InvalidCase(format!(
                "tap control on branch {} has an invalid branch, band or ratio range",
                tap.branch
            )));
        }
        setup.taps.push((k, tap));
    }
    Ok(setup)
}

#[derive(Clone)]
struct ShuntState<'a> {
    index: usize,
    shunt: &'a SwitchedShunt,
    /// Index in `case.buses`.
    bus: usize,
    /// Network index of the controlled bus, if it is active.
    controlled: Option<usize>,
    levels: Vec<f64>,
    value: f64,
    last: i8,
    reversals: u8,
    /// The level the shunt last moved away from and the controlled voltage
    /// it gave, so that a step back can be judged before it is taken.
    previous: Option<(f64, f64)>,
}

/// How far (p.u.) `vm` is outside `vmin..=vmax`; zero inside.
fn band_violation(vm: f64, vmin: f64, vmax: f64) -> f64 {
    (vmin - vm).max(vm - vmax).max(0.0)
}

#[derive(Clone)]
struct TapState<'a> {
    index: usize,
    tap: &'a TapControl,
    controlled: Option<usize>,
    ratio: f64,
    /// Taps holding the same bus, which share each correction.
    sharing: usize,
    last: i8,
    reversals: u8,
    /// Controlled voltage and ratio before the last move, and the solve
    /// round it was made in, to measure how strongly the ratio moves the
    /// voltage.
    before: Option<(f64, f64, usize)>,
    /// Measured voltage change per unit of ratio.
    sensitivity: Option<f64>,
    /// Held because the ratio barely moves, or wrongly moves, the voltage.
    ineffective: bool,
}

/// Steps discrete shunts and transformer taps between Newton solves.
#[derive(Clone)]
pub(crate) struct Controller<'a> {
    shunts: Vec<ShuntState<'a>>,
    taps: Vec<TapState<'a>>,
    /// Continuous shunts as (listed index, helper generator).
    helpers: Vec<(usize, usize)>,
    /// Continuous shunts fixed at an end of their range, as (listed index,
    /// susceptance moved into the bus's `bs`).
    fixed: Vec<(usize, f64)>,
    counts: [usize; 2],
}

/// Direction reversals after which a device is held, so that it cannot hunt.
const MAX_REVERSALS: u8 = 2;

/// Smallest relative voltage change per relative ratio change for a tap to
/// keep controlling its bus.
const MIN_TAP_EFFECT: f64 = 0.05;

/// Fraction of a band inside its nearer edge that a tap aims for.
const TARGET_MARGIN: f64 = 0.2;

/// Most tap steps in one pass.
const MAX_TAP_STEPS: f64 = 4.0;

/// Voltage tolerance (p.u.) outside a band before a device moves.
const BAND_TOLERANCE: f64 = 1e-5;

impl<'a> Controller<'a> {
    pub fn new(setup: &Setup<'a>, net: &Network, case: &Case) -> Self {
        let index: std::collections::HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let shunts = setup
            .discrete
            .iter()
            .map(|&(k, shunt)| ShuntState {
                index: k,
                shunt,
                bus: index[&shunt.bus],
                controlled: net
                    .by_id
                    .get(&shunt.regulated_bus.unwrap_or(shunt.bus))
                    .copied()
                    .filter(|_| net.by_id.contains_key(&shunt.bus)),
                levels: shunt.levels(),
                value: shunt.initial,
                last: 0,
                reversals: 0,
                previous: None,
            })
            .collect();
        let mut held = std::collections::HashMap::<u32, usize>::new();
        for &(_, tap) in &setup.taps {
            if case.branches[tap.branch].online {
                *held.entry(tap.regulated_bus).or_default() += 1;
            }
        }
        let taps = setup
            .taps
            .iter()
            .map(|&(k, tap)| TapState {
                index: k,
                tap,
                controlled: net
                    .by_id
                    .get(&tap.regulated_bus)
                    .copied()
                    .filter(|_| case.branches[tap.branch].online),
                ratio: tap.ratio,
                sharing: held.get(&tap.regulated_bus).copied().unwrap_or(1).max(1),
                last: 0,
                reversals: 0,
                before: None,
                sensitivity: None,
                ineffective: false,
            })
            .collect();
        Self {
            shunts,
            taps,
            helpers: setup.continuous.iter().map(|&(k, _, g)| (k, g)).collect(),
            fixed: Vec::new(),
            counts: setup.counts,
        }
    }

    /// Whether there is anything to step.
    pub fn is_empty(&self) -> bool {
        self.shunts.is_empty() && self.taps.is_empty()
    }

    /// One control pass at voltages `v`: move every device whose controlled
    /// bus is outside its band (and not held by a generator), updating the
    /// shunts' `bs` and the branches' `tap` in `case`. Returns the number of
    /// devices moved.
    ///
    /// `round` counts the solves so far; a tap's response is measured only
    /// across consecutive rounds, when nothing else changed in between.
    /// Switched shunts and tap changers under voltage control.
    pub fn devices(&self) -> (usize, usize) {
        (self.shunts.len(), self.taps.len())
    }

    pub fn pass(&mut self, case: &mut Case, v: &[C], kinds: &[BusType], round: usize) -> usize {
        let mut moved = 0;
        // The first few moves of a pass go to the normal log, the rest to the detailed one.
        let report = |n: usize, text: String| {
            if n < crate::solver::DETAIL_LINES {
                log::info!("{text}");
            } else {
                log::debug!("{text}");
            }
        };
        let free = |i: usize| kinds[i] == BusType::Pq;
        for s in &mut self.shunts {
            let Some(c) = s.controlled.filter(|&c| free(c)) else {
                continue;
            };
            if s.reversals >= MAX_REVERSALS {
                continue;
            }
            let vm = v[c].norm();
            let (next, dir) = if vm < s.shunt.vmin - BAND_TOLERANCE {
                (s.levels.iter().copied().find(|&l| l > s.value + 1e-9), 1)
            } else if vm > s.shunt.vmax + BAND_TOLERANCE {
                (
                    s.levels.iter().rev().copied().find(|&l| l < s.value - 1e-9),
                    -1,
                )
            } else {
                (None, 0)
            };
            let Some(next) = next else { continue };
            // A step back to the level just left means one step jumps right
            // over the band (it is narrower than a step, or has no width at
            // all, VSWLO = VSWHI). Both voltages are known by now, so settle
            // on whichever level lands nearer the band and hold there, like
            // a target with a deadband of one step, instead of hunting.
            if let Some((level, v_level)) = s.previous
                && s.last != 0
                && s.last != dir
                && (level - next).abs() < 1e-9
            {
                s.reversals = MAX_REVERSALS;
                let here = band_violation(vm, s.shunt.vmin, s.shunt.vmax);
                let there = band_violation(v_level, s.shunt.vmin, s.shunt.vmax);
                let keep = if there < here { level } else { s.value };
                log::info!(
                    "  switched shunt at bus {}: one step jumps over the band {:.4}-{:.4} (bus {} at {:.4} pu with {:.1} Mvar, {:.4} pu with {:.1} Mvar); holding at {keep:.1} Mvar, nearer the band",
                    s.shunt.bus,
                    s.shunt.vmin,
                    s.shunt.vmax,
                    s.shunt.regulated_bus.unwrap_or(s.shunt.bus),
                    vm,
                    s.value,
                    v_level,
                    level,
                );
                if keep != s.value {
                    case.buses[s.bus].bs += keep - s.value;
                    s.value = keep;
                    moved += 1;
                }
                continue;
            }
            if s.last != 0 && s.last != dir {
                s.reversals += 1;
            }
            s.last = dir;
            s.previous = Some((s.value, vm));
            report(
                moved,
                format!(
                    "  switched shunt at bus {}: {:.1} -> {:.1} Mvar, bus {} at {vm:.4} pu is outside {:.4}-{:.4}",
                    s.shunt.bus,
                    s.value,
                    next,
                    s.shunt.regulated_bus.unwrap_or(s.shunt.bus),
                    s.shunt.vmin,
                    s.shunt.vmax
                ),
            );
            case.buses[s.bus].bs += next - s.value;
            s.value = next;
            moved += 1;
        }
        for t in &mut self.taps {
            let Some(c) = t.controlled.filter(|&c| free(c)) else {
                continue;
            };
            let vm = v[c].norm();
            let sign = if t.tap.raises_voltage { 1.0 } else { -1.0 };
            if let Some((v0, r0, at)) = t.before.take()
                && at + 1 == round
            {
                // Per-unit voltage change per per-unit ratio change; about
                // one against a stiff source, near zero when the controlled
                // bus is the stiff side.
                let s = (vm - v0) / (t.ratio - r0);
                if s * sign * t.ratio / vm < MIN_TAP_EFFECT {
                    t.ineffective = true;
                    let branch = &case.branches[t.tap.branch];
                    log::info!(
                        "  tap on transformer {}-{} barely moves bus {}; holding it",
                        branch.from,
                        branch.to,
                        t.tap.regulated_bus
                    );
                } else {
                    t.sensitivity = Some(s);
                }
            }
            if t.ineffective || t.reversals >= MAX_REVERSALS {
                continue;
            }
            if vm >= t.tap.vmin - BAND_TOLERANCE && vm <= t.tap.vmax + BAND_TOLERANCE {
                continue;
            }
            // Aim a little inside the nearer edge of the band, as tap changers do.
            let margin = TARGET_MARGIN * (t.tap.vmax - t.tap.vmin);
            let target = if vm < t.tap.vmin {
                t.tap.vmin + margin
            } else {
                t.tap.vmax - margin
            };
            // Until a move has been measured, assume a stiff source: the
            // voltage moves by the same fraction as the ratio, shared among
            // the taps holding the bus. That undershoots, never overshoots.
            let change = match t.sensitivity {
                Some(s) => (target - vm) / s,
                None => sign * (target - vm) / vm * t.ratio / t.sharing as f64,
            };
            let step = t.tap.step();
            let steps = (change / step).round().clamp(-MAX_TAP_STEPS, MAX_TAP_STEPS);
            let steps = if steps == 0.0 { change.signum() } else { steps };
            let next = (t.ratio + steps * step).clamp(t.tap.ratio_min, t.tap.ratio_max);
            if (next - t.ratio).abs() < 1e-12 {
                continue;
            }
            let dir = if next > t.ratio { 1 } else { -1 };
            if t.last != 0 && t.last != dir {
                t.reversals += 1;
            }
            t.last = dir;
            let branch = &case.branches[t.tap.branch];
            report(
                moved,
                format!(
                    "  tap on transformer {}-{}: ratio {:.4} -> {next:.4}, bus {} at {vm:.4} pu is outside {:.4}-{:.4}",
                    branch.from, branch.to, t.ratio, t.tap.regulated_bus, t.tap.vmin, t.tap.vmax
                ),
            );
            t.before = Some((vm, t.ratio, round));
            t.ratio = next;
            case.branches[t.tap.branch].tap = t.tap.tap_per_ratio * next;
            moved += 1;
        }
        moved
    }

    /// Fix the continuous shunt whose helper generator `g` reached reactive
    /// limit `q` at an end of its range: the full susceptance moves into its
    /// bus's `bs` and the helper stops injecting. Returns false when `g` is
    /// not a helper.
    pub fn fix_continuous(&mut self, case: &mut Case, bus: usize, g: usize, q: f64) -> bool {
        let Some(&(k, _)) = self.helpers.iter().find(|h| h.1 == g) else {
            return false;
        };
        let helper = &mut case.generators[g];
        let b = q / (helper.vg * helper.vg);
        (helper.qg, helper.qmax, helper.qmin) = (0.0, 0.0, 0.0);
        case.buses[bus].bs += b;
        self.fixed.push((k, b));
        true
    }

    /// Go back to `earlier`, the state before the last pass, after the solve
    /// that pass led to failed, and hold every device that pass moved.
    /// Returns the number held.
    pub fn undo(&mut self, earlier: Self) -> usize {
        let moved_shunts: Vec<bool> = (self.shunts.iter().zip(&earlier.shunts))
            .map(|(now, then)| now.value != then.value)
            .collect();
        let moved_taps: Vec<bool> = (self.taps.iter().zip(&earlier.taps))
            .map(|(now, then)| now.ratio != then.ratio)
            .collect();
        *self = earlier;
        let mut held = 0;
        for (s, _) in self.shunts.iter_mut().zip(&moved_shunts).filter(|x| *x.1) {
            s.reversals = MAX_REVERSALS;
            held += 1;
        }
        for (t, _) in self.taps.iter_mut().zip(&moved_taps).filter(|x| *x.1) {
            t.ineffective = true;
            held += 1;
        }
        held
    }

    /// Final shunt values (MVAr at 1 p.u.) for every listed shunt, with
    /// continuous and frozen ones at their starting value, and final ratios
    /// for every listed tap control.
    pub fn values(&self, controls: Option<&VoltageControls>) -> (Vec<f64>, Vec<f64>) {
        let mut shunts = vec![f64::NAN; self.counts[0]];
        let mut taps = vec![f64::NAN; self.counts[1]];
        if let Some(c) = controls {
            for (k, s) in c.switched_shunts.iter().enumerate() {
                shunts[k] = s.initial;
            }
            for (k, t) in c.taps.iter().enumerate() {
                taps[k] = t.ratio;
            }
        }
        for s in &self.shunts {
            shunts[s.index] = s.value;
        }
        for t in &self.taps {
            taps[t.index] = t.ratio;
        }
        for &(k, b) in &self.fixed {
            shunts[k] += b;
        }
        (shunts, taps)
    }
}

/// Turn each continuous shunt's helper generator output into the shunt's
/// final susceptance, in `shunts` and in its bus's `bs`, once the helpers
/// have been removed from `case`.
pub(crate) fn finish(setup: &Setup<'_>, case: &mut Case, shunts: &mut [f64]) {
    for &(k, shunt, g) in &setup.continuous {
        let q = case.generators[g].qg;
        let Some(bus) = case.buses.iter_mut().find(|b| b.id == shunt.bus) else {
            continue;
        };
        let b = q / (bus.vm * bus.vm);
        bus.bs += b;
        // The helper is gone, so its bus goes back to a load bus.
        if bus.kind == BusType::Pv {
            bus.kind = BusType::Pq;
        }
        if let Some(value) = shunts.get_mut(k) {
            *value += b;
        }
    }
}
