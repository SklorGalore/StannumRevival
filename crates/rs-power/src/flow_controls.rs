//! Power-flow controls applied between Newton solves: phase shifters that
//! hold the active power through a transformer winding within a band (PSS/E
//! winding COD 3, symmetric, and COD 5, asymmetric), and tap changers that
//! hold its reactive power within a band (COD 2).

/// Flow controls to apply during an AC solve, usually from
/// [`crate::RawImport::flow_controls`]. Empty by default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlowControls {
    /// Transformer windings whose phase shift angle holds their active
    /// power flow within a band.
    pub phase_shifters: Vec<PhaseShifter>,
    /// Transformer windings whose tap ratio holds their reactive power flow
    /// within a band.
    pub reactive_taps: Vec<ReactiveTap>,
}

impl FlowControls {
    /// True when there is nothing to control.
    pub fn is_empty(&self) -> bool {
        self.phase_shifters.is_empty() && self.reactive_taps.is_empty()
    }
}

/// A transformer winding whose tap steps to hold the reactive power flowing
/// into it at its own bus (the branch's `from` end) within a band (PSS/E
/// COD 2).
#[derive(Clone, Debug, PartialEq)]
pub struct ReactiveTap {
    /// Index of the winding's entry in `case.branches`.
    pub branch: usize,
    /// Upper limit of the flow band in Mvar (VMA).
    pub mvar_max: f64,
    /// Lower limit of the flow band in Mvar (VMI).
    pub mvar_min: f64,
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

impl ReactiveTap {
    /// The band as (low, high) Mvar, whichever way round the record gives it.
    pub fn band(&self) -> (f64, f64) {
        (
            self.mvar_min.min(self.mvar_max),
            self.mvar_min.max(self.mvar_max),
        )
    }

    /// Ratio change of one tap step.
    pub fn step(&self) -> f64 {
        (self.ratio_max - self.ratio_min) / f64::from(self.positions.max(2) - 1)
    }
}

/// A transformer winding whose phase shift angle holds the active power
/// flowing into it at its own bus (the branch's `from` end) within a band.
///
/// The angle moves continuously within `angle_min..=angle_max`.
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseShifter {
    /// Index of the winding's entry in `case.branches`.
    pub branch: usize,
    /// Upper limit of the flow band in MW (VMA).
    pub mw_max: f64,
    /// Lower limit of the flow band in MW (VMI).
    pub mw_min: f64,
    /// Largest phase shift angle in degrees (RMA).
    pub angle_max: f64,
    /// Smallest phase shift angle in degrees (RMI).
    pub angle_min: f64,
    /// Starting phase shift angle in degrees (ANG), the branch's `shift`.
    pub angle: f64,
    /// For an asymmetric phase shifter (COD 5), the winding connection
    /// angle in degrees (CNXA) and the branch tap at the starting angle.
    /// None for a symmetric one (COD 3).
    pub asymmetric: Option<Asymmetric>,
    /// One-based source line of the winding record, for messages.
    pub line: usize,
}

/// How an asymmetric phase shifter's ratio follows its angle.
///
/// Its regulating voltage is added at the connection angle `α` to the
/// winding voltage, so the complex ratio is `1 + k·e^{jα}` for some
/// regulating amount `k`. For a phase shift `φ` that gives a magnitude of
/// `sin α / sin(α − φ)`: at `α = 90°` (a quadrature booster) `1 / cos φ`.
/// The branch tap is scaled by that magnitude relative to the starting
/// angle, so the case's own tap holds at its own angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Asymmetric {
    /// Winding connection angle in degrees (CNXA), within -90..=90 and not
    /// zero.
    pub connection_angle: f64,
    /// The branch's off-nominal tap at the starting angle.
    pub tap: f64,
}

impl Asymmetric {
    /// Ratio magnitude at phase shift `angle` (degrees), relative to no
    /// regulating voltage.
    pub fn magnitude(&self, angle: f64) -> f64 {
        let a = self.connection_angle.to_radians();
        a.sin() / (a - angle.to_radians()).sin()
    }

    /// Angles (degrees) the connection angle can reach, kept 5° clear of
    /// where the magnitude runs off to infinity.
    pub fn reach(&self) -> (f64, f64) {
        let a = self.connection_angle;
        if a > 0.0 {
            (a - 175.0, a - 5.0)
        } else {
            (a + 5.0, a + 175.0)
        }
    }
}

impl PhaseShifter {
    /// The band as (low, high) MW, whichever way round the record gives it.
    pub fn band(&self) -> (f64, f64) {
        (self.mw_min.min(self.mw_max), self.mw_min.max(self.mw_max))
    }

    /// The angle range as (low, high) degrees, narrowed for an asymmetric
    /// shifter to angles its connection can reach.
    pub fn range(&self) -> (f64, f64) {
        let (mut lo, mut hi) = (
            self.angle_min.min(self.angle_max),
            self.angle_min.max(self.angle_max),
        );
        if let Some(a) = &self.asymmetric {
            let (rlo, rhi) = a.reach();
            (lo, hi) = (lo.max(rlo), hi.min(rhi));
        }
        (lo, hi)
    }

    /// The branch tap at phase shift `angle`: unchanged for a symmetric
    /// shifter, scaled with the ratio magnitude for an asymmetric one.
    pub fn tap_at(&self, angle: f64, tap: f64) -> f64 {
        match &self.asymmetric {
            None => tap,
            Some(a) => a.tap * a.magnitude(angle) / a.magnitude(self.angle),
        }
    }
}

// ---------------------------------------------------------------------------
// Solver side.

use crate::{BusType, Case, Error, network::Network};
use faer::{Mat, prelude::Solve, sparse::SparseColMat};
use num_complex::Complex64 as C;

/// The flow controls that move in a solve, with their listed indices.
pub(crate) struct Moving<'a> {
    pub shifters: Vec<(usize, &'a PhaseShifter)>,
    pub taps: Vec<(usize, &'a ReactiveTap)>,
}

impl Moving<'_> {
    pub fn none() -> Moving<'static> {
        Moving {
            shifters: Vec::new(),
            taps: Vec::new(),
        }
    }
}

/// Check `controls` against `case` and list the controls that move: no
/// phase shifter when `lock_shifters` says so, no tap when `lock_taps`
/// does, and none on an offline branch.
pub(crate) fn setup<'a>(
    case: &Case,
    controls: &'a FlowControls,
    [lock_shifters, lock_taps]: [bool; 2],
) -> Result<Moving<'a>, Error> {
    let mut moving = Moving::none();
    for (k, p) in controls.phase_shifters.iter().enumerate() {
        let valid = [p.mw_max, p.mw_min, p.angle_max, p.angle_min, p.angle]
            .iter()
            .all(|x| x.is_finite())
            && p.asymmetric.is_none_or(|a| {
                a.tap > 0.0
                    && a.tap.is_finite()
                    && a.connection_angle.abs() >= 1.0
                    && a.connection_angle.abs() <= 90.0
            });
        if p.branch >= case.branches.len() || !valid {
            return Err(Error::InvalidCase(format!(
                "phase shifter on branch {} has an invalid branch, band, angle range or connection angle",
                p.branch
            )));
        }
        let (lo, hi) = p.range();
        if !lock_shifters && case.branches[p.branch].online && lo < hi {
            moving.shifters.push((k, p));
        }
    }
    for (k, t) in controls.reactive_taps.iter().enumerate() {
        let valid = [t.mvar_max, t.mvar_min, t.ratio, t.tap_per_ratio]
            .iter()
            .all(|x| x.is_finite())
            && t.ratio_max > t.ratio_min
            && t.ratio > 0.0
            && t.tap_per_ratio > 0.0;
        if t.branch >= case.branches.len() || !valid {
            return Err(Error::InvalidCase(format!(
                "reactive flow tap on branch {} has an invalid branch, band or ratio range",
                t.branch
            )));
        }
        if !lock_taps && case.branches[t.branch].online {
            moving.taps.push((k, t));
        }
    }
    Ok(moving)
}

/// What a control moves and what it holds.
#[derive(Clone, Copy)]
enum Device<'a> {
    Shifter(&'a PhaseShifter),
    Tap(&'a ReactiveTap),
}

impl Device<'_> {
    fn branch(&self) -> usize {
        match self {
            Device::Shifter(p) => p.branch,
            Device::Tap(t) => t.branch,
        }
    }

    fn band(&self) -> (f64, f64) {
        match self {
            Device::Shifter(p) => p.band(),
            Device::Tap(t) => t.band(),
        }
    }

    /// Whether the device holds reactive rather than active power.
    fn reactive(&self) -> bool {
        matches!(self, Device::Tap(_))
    }
}

#[derive(Clone)]
struct State<'a> {
    /// Index in the device's list in [`FlowControls`].
    index: usize,
    device: Device<'a>,
    /// Phase shift angle in degrees, or winding ratio.
    value: f64,
    last: i8,
    reversals: u8,
    /// Held because it barely moves its flow, or a solve after its move
    /// failed.
    held: bool,
}

/// Direction reversals after which a device is held, so that it cannot hunt.
const MAX_REVERSALS: u8 = 2;

/// Fraction of a band inside its nearer edge that a device aims for.
const TARGET_MARGIN: f64 = 0.2;

/// Flow tolerance (MW or Mvar) outside a band before a device moves.
const BAND_TOLERANCE: f64 = 0.1;

/// Largest angle change (degrees) in one pass.
const MAX_STEP: f64 = 10.0;

/// Most tap steps in one pass.
const MAX_TAP_STEPS: f64 = 4.0;

/// Smallest flow change for a device to keep controlling: MW per degree
/// for a phase shifter, Mvar per tap step for a tap.
const MIN_EFFECT: f64 = 0.01;

/// Moves phase shifters and reactive flow taps between Newton solves.
#[derive(Clone)]
pub(crate) struct FlowController<'a> {
    devices: Vec<State<'a>>,
    counts: [usize; 2],
}

/// A device's branch at the present voltages.
struct Terminal {
    f: usize,
    t: usize,
    /// Power into the branch at its `from` end, per unit.
    s: C,
    /// `Vf · conj(yft · Vt)` and `Vt · conj(ytf · Vf)`.
    a: C,
    b: C,
    /// `Vf · conj(yff · Vf)`.
    own: C,
    /// Off-nominal tap magnitude of the branch.
    tap: f64,
}

impl Terminal {
    /// Change of the power into the branch at each end per unit of the
    /// device's control, with every voltage held: per radian of angle,
    /// or per unit of winding ratio.
    fn ends(&self, device: &Device<'_>) -> [C; 2] {
        match device {
            // yft and ytf turn with the angle: d/dphi = -j a at the from
            // end, j b at the to end.
            Device::Shifter(_) => [C::new(0.0, -1.0) * self.a, C::new(0.0, 1.0) * self.b],
            // yff goes as 1/t^2 and yft, ytf as 1/t.
            Device::Tap(t) => {
                let per_ratio = t.tap_per_ratio / self.tap;
                [-(2.0 * self.own + self.a) * per_ratio, -self.b * per_ratio]
            }
        }
    }
}

impl<'a> FlowController<'a> {
    pub fn new(moving: &Moving<'a>, controls: &FlowControls) -> Self {
        let state = |index, device, value| State {
            index,
            device,
            value,
            last: 0,
            reversals: 0,
            held: false,
        };
        let devices = (moving.shifters.iter())
            .map(|&(k, p)| state(k, Device::Shifter(p), p.angle))
            .chain((moving.taps.iter()).map(|&(k, t)| state(k, Device::Tap(t), t.ratio)))
            .collect();
        Self {
            devices,
            counts: [controls.phase_shifters.len(), controls.reactive_taps.len()],
        }
    }

    /// Whether there is anything to move.
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Phase shifters and reactive flow taps in control.
    pub fn devices(&self) -> (usize, usize) {
        let taps = self.devices.iter().filter(|d| d.device.reactive()).count();
        (self.devices.len() - taps, taps)
    }

    /// One control pass at voltages `v` with bus kinds `kinds`: move every
    /// device whose flow is outside its band, updating the branches'
    /// `shift` and `tap` in `case`. Returns the number moved.
    ///
    /// The moves come from the flows' sensitivities to the controls through
    /// the whole network (the power-flow Jacobian), solved together, so
    /// that devices in parallel or in series share the correction instead
    /// of fighting over it. Taps then round their moves to whole steps.
    pub fn pass(
        &mut self,
        case: &mut Case,
        net: &Network,
        v: &[C],
        kinds: &[BusType],
    ) -> Result<usize, Error> {
        let mut edge_of = std::collections::HashMap::new();
        for d in &self.devices {
            edge_of.insert(d.device.branch(), None);
        }
        for (k, e) in net.edges.iter().enumerate() {
            if let Some(slot) = edge_of.get_mut(&e.original) {
                *slot = Some(k);
            }
        }
        let terminal = |branch: usize| {
            edge_of[&branch].map(|k: usize| {
                let e = &net.edges[k];
                let own = v[e.f] * (e.yff * v[e.f]).conj();
                let a = v[e.f] * (e.yft * v[e.t]).conj();
                let tap = case.branches[branch].tap;
                Terminal {
                    f: e.f,
                    t: e.t,
                    s: own + a,
                    a,
                    b: v[e.t] * (e.ytf * v[e.f]).conj(),
                    own,
                    tap: if tap == 0.0 { 1.0 } else { tap },
                }
            })
        };
        let base = case.base_mva;
        let measure = |d: &Device<'_>, s: C| if d.reactive() { s.im } else { s.re };
        // Devices outside their bands, with their terminals, flows and
        // targets.
        let mut out = Vec::new();
        for (k, d) in self.devices.iter().enumerate() {
            if d.held || d.reversals >= MAX_REVERSALS {
                continue;
            }
            let Some(term) = terminal(d.device.branch()) else {
                continue;
            };
            let flow = measure(&d.device, term.s) * base;
            let (lo, hi) = d.device.band();
            if flow >= lo - BAND_TOLERANCE && flow <= hi + BAND_TOLERANCE {
                continue;
            }
            let margin = TARGET_MARGIN * (hi - lo);
            let target = if flow < lo { lo + margin } else { hi - margin };
            out.push((k, term, flow, target));
        }
        if out.is_empty() {
            return Ok(0);
        }
        let parts: Vec<_> = out
            .iter()
            .map(|(k, term, _, _)| (self.devices[*k].device, term))
            .collect();
        let slopes = sensitivities(net, v, kinds, &parts)?;
        // Per degree for shifters, per unit of ratio for taps; MW or Mvar.
        let m = out.len();
        let unit = |d: &Device<'_>| match d {
            Device::Shifter(_) => base * std::f64::consts::PI / 180.0,
            Device::Tap(_) => base,
        };
        let mut matrix: Vec<Vec<f64>> = (0..m)
            .map(|i| (0..m).map(|j| slopes[i][j] * unit(&parts[j].0)).collect())
            .collect();
        // A device that barely moves its own flow is held, and left out.
        let mut keep = vec![true; m];
        for i in 0..m {
            let d = &mut self.devices[out[i].0];
            let effect = match d.device {
                Device::Shifter(_) => matrix[i][i],
                Device::Tap(t) => matrix[i][i] * t.step(),
            };
            if effect.abs() < MIN_EFFECT {
                keep[i] = false;
                d.held = true;
                let branch = &case.branches[d.device.branch()];
                let (what, per) = match d.device {
                    Device::Shifter(_) => ("phase shifter", "MW per degree"),
                    Device::Tap(_) => ("reactive flow tap", "Mvar per step"),
                };
                log::info!(
                    "  {what} {}-{} barely moves its flow ({effect:.3} {per}); holding it",
                    branch.from,
                    branch.to,
                );
            }
        }
        for i in 0..m {
            for j in 0..m {
                if !(keep[i] && keep[j]) {
                    matrix[i][j] = if i == j { 1.0 } else { 0.0 };
                }
            }
        }
        let rhs: Vec<f64> = (0..m)
            .map(|i| if keep[i] { out[i].3 - out[i].2 } else { 0.0 })
            .collect();
        let changes = dense_solve(matrix.clone(), rhs.clone()).unwrap_or_else(|| {
            // Devices that cannot be moved independently (in series on
            // one path, say): each takes its own share instead.
            (0..m).map(|i| rhs[i] / matrix[i][i] / m as f64).collect()
        });
        // Scale every angle move down together when one would exceed the
        // per-pass limit, so the shifters keep their proportions.
        let largest = (0..m)
            .filter(|&i| !parts[i].0.reactive())
            .fold(0.0_f64, |a, i| a.max(changes[i].abs()));
        let factor = if largest > MAX_STEP {
            MAX_STEP / largest
        } else {
            1.0
        };
        let mut moved = 0;
        for (i, (k, _, flow, _)) in out.iter().enumerate() {
            if !keep[i] {
                continue;
            }
            let d = &mut self.devices[*k];
            let next = match d.device {
                Device::Shifter(p) => {
                    let (amin, amax) = p.range();
                    (d.value + changes[i] * factor).clamp(amin, amax)
                }
                // Whole steps, at least one, as tap changers move.
                Device::Tap(t) => {
                    let step = t.step();
                    let steps = (changes[i] / step)
                        .round()
                        .clamp(-MAX_TAP_STEPS, MAX_TAP_STEPS);
                    let steps = if steps == 0.0 {
                        changes[i].signum()
                    } else {
                        steps
                    };
                    (d.value + steps * step).clamp(t.ratio_min, t.ratio_max)
                }
            };
            if (next - d.value).abs() < 1e-9 {
                continue;
            }
            let dir = if next > d.value { 1 } else { -1 };
            if d.last != 0 && d.last != dir {
                d.reversals += 1;
            }
            d.last = dir;
            let (lo, hi) = d.device.band();
            let branch = &mut case.branches[d.device.branch()];
            let text = match d.device {
                Device::Shifter(_) => format!(
                    "  phase shifter {}-{}: angle {:.3} -> {next:.3} deg, flow {flow:.1} MW is outside {lo:.1}-{hi:.1}",
                    branch.from, branch.to, d.value
                ),
                Device::Tap(_) => format!(
                    "  reactive flow tap {}-{}: ratio {:.4} -> {next:.4}, flow {flow:.1} Mvar is outside {lo:.1}-{hi:.1}",
                    branch.from, branch.to, d.value
                ),
            };
            if moved < crate::solver::DETAIL_LINES {
                log::info!("{text}");
            } else {
                log::debug!("{text}");
            }
            d.value = next;
            match d.device {
                Device::Shifter(p) => {
                    branch.shift = next;
                    branch.tap = p.tap_at(next, branch.tap);
                }
                Device::Tap(t) => branch.tap = t.tap_per_ratio * next,
            }
            moved += 1;
        }
        Ok(moved)
    }

    /// Go back to `earlier` after the solve that the last pass led to
    /// failed, holding every device that pass moved. Returns the number held.
    pub fn undo(&mut self, earlier: Self) -> usize {
        let moved: Vec<bool> = (self.devices.iter().zip(&earlier.devices))
            .map(|(now, then)| now.value != then.value)
            .collect();
        *self = earlier;
        let mut held = 0;
        for (d, _) in self.devices.iter_mut().zip(&moved).filter(|x| *x.1) {
            d.held = true;
            held += 1;
        }
        held
    }

    /// Final angles for every listed phase shifter and final ratios for
    /// every listed reactive flow tap, with locked and offline ones at their
    /// starting values.
    pub fn values(&self, controls: &FlowControls) -> (Vec<f64>, Vec<f64>) {
        let mut angles: Vec<f64> = controls.phase_shifters.iter().map(|p| p.angle).collect();
        let mut ratios: Vec<f64> = controls.reactive_taps.iter().map(|t| t.ratio).collect();
        angles.resize(self.counts[0], f64::NAN);
        ratios.resize(self.counts[1], f64::NAN);
        for d in &self.devices {
            match d.device {
                Device::Shifter(_) => angles[d.index] = d.value,
                Device::Tap(_) => ratios[d.index] = d.value,
            }
        }
        (angles, ratios)
    }
}

/// `slopes[i][j]`: change of device `i`'s flow (per unit; active power for
/// a shifter, reactive for a tap) per unit of device `j`'s control (radian
/// of angle, or unit of winding ratio), with the rest of the network
/// responding as the Newton equations say: generators hold their buses'
/// voltages and the reference buses take up the change in losses.
fn sensitivities(
    net: &Network,
    v: &[C],
    kinds: &[BusType],
    parts: &[(Device<'_>, &Terminal)],
) -> Result<Vec<Vec<f64>>, Error> {
    let (angles, pq) = crate::solver::variables(kinds);
    let n = angles.len() + pq.len();
    let mut angle_of = vec![None; v.len()];
    let mut magnitude_of = vec![None; v.len()];
    for (k, &i) in angles.iter().enumerate() {
        angle_of[i] = Some(k);
    }
    for (k, &i) in pq.iter().enumerate() {
        magnitude_of[i] = Some(angles.len() + k);
    }
    let m = parts.len();
    let part = |d: &Device<'_>, s: C| if d.reactive() { s.im } else { s.re };
    let ends: Vec<[C; 2]> = parts.iter().map(|(d, t)| t.ends(d)).collect();
    let mut slopes = vec![vec![0.0; m]; m];
    // Direct effect of each control on its own branch's flow, with every
    // voltage held (and on any other device on the same branch).
    for i in 0..m {
        for j in 0..m {
            if parts[i].1.f == parts[j].1.f
                && parts[i].1.t == parts[j].1.t
                && parts[i].0.branch() == parts[j].0.branch()
            {
                slopes[i][j] = part(&parts[i].0, ends[j][0]);
            }
        }
    }
    if n == 0 {
        return Ok(slopes);
    }
    let entries = crate::solver::jacobian(net, v, &angles, &pq);
    let a = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &entries)
        .map_err(|e| Error::Numerical(format!("sparse assembly: {e:?}")))?;
    let lu = a
        .sp_lu()
        .map_err(|e| Error::Numerical(format!("singular Jacobian for flow controls: {e:?}")))?;
    // Right-hand sides: the change in the power each terminal bus sends
    // into the branch per unit of control. The Newton step dx = -J^-1 dF
    // moves the state to absorb it.
    let rhs = Mat::from_fn(n, m, |row, j| {
        let t = parts[j].1;
        let mut x = 0.0;
        for (bus, ds) in [(t.f, ends[j][0]), (t.t, ends[j][1])] {
            if angle_of[bus] == Some(row) {
                x -= ds.re;
            }
            if magnitude_of[bus] == Some(row) {
                x -= ds.im;
            }
        }
        x
    });
    let dx = lu.solve(rhs);
    // Each flow's change with the state: S = own_f + a with
    // dS/dθf = j a, dS/dθt = -j a, dS/d|Vf| = (2 own_f + a)/|Vf|,
    // dS/d|Vt| = a/|Vt|.
    for (i, (d, t)) in parts.iter().enumerate() {
        let vf = v[t.f].norm();
        let vt = v[t.t].norm();
        let j = C::new(0.0, 1.0);
        let grads = [
            (angle_of[t.f], part(d, j * t.a)),
            (angle_of[t.t], part(d, -j * t.a)),
            (magnitude_of[t.f], part(d, (2.0 * t.own + t.a) / vf)),
            (magnitude_of[t.t], part(d, t.a / vt)),
        ];
        for k in 0..m {
            for &(col, g) in &grads {
                if let Some(col) = col {
                    slopes[i][k] += g * dx[(col, k)];
                }
            }
        }
    }
    if slopes.iter().flatten().any(|x| !x.is_finite()) {
        return Err(Error::Numerical(
            "non-finite flow control sensitivities".into(),
        ));
    }
    Ok(slopes)
}

/// Solve the small dense system `a x = b` by Gaussian elimination with
/// partial pivoting; None when it is (nearly) singular.
fn dense_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    let scale = a.iter().flatten().fold(0.0_f64, |m, x| m.max(x.abs()));
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() <= 1e-9 * scale {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let (upper, lower) = a.split_at_mut(col + 1);
        let pivot_row = &upper[col];
        for (offset, row) in lower.iter_mut().enumerate() {
            let f = row[col] / pivot_row[col];
            for (x, p) in row[col..].iter_mut().zip(&pivot_row[col..]) {
                *x -= f * p;
            }
            b[col + 1 + offset] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}
