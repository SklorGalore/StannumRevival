//! Power-flow controls applied between Newton solves: phase shifters that
//! hold the active power through a transformer winding within a band (PSS/E
//! winding COD 3, symmetric, and COD 5, asymmetric).

/// Flow controls to apply during an AC solve, usually from
/// [`crate::RawImport::flow_controls`]. Empty by default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlowControls {
    /// Transformer windings whose phase shift angle holds their active
    /// power flow within a band.
    pub phase_shifters: Vec<PhaseShifter>,
}

impl FlowControls {
    /// True when there is nothing to control.
    pub fn is_empty(&self) -> bool {
        self.phase_shifters.is_empty()
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

/// Check `controls` against `case` and list the phase shifters that move:
/// none when `lock` says so, and none on an offline branch.
pub(crate) fn setup<'a>(
    case: &Case,
    controls: &'a FlowControls,
    lock: bool,
) -> Result<Vec<(usize, &'a PhaseShifter)>, Error> {
    let mut moving = Vec::new();
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
        if !lock && case.branches[p.branch].online && lo < hi {
            moving.push((k, p));
        }
    }
    Ok(moving)
}

#[derive(Clone)]
struct ShifterState<'a> {
    index: usize,
    shifter: &'a PhaseShifter,
    angle: f64,
    last: i8,
    reversals: u8,
    /// Held because the angle barely moves the flow, or a solve after its
    /// move failed.
    held: bool,
}

/// Direction reversals after which a shifter is held, so that it cannot hunt.
const MAX_REVERSALS: u8 = 2;

/// Fraction of a band inside its nearer edge that a shifter aims for.
const TARGET_MARGIN: f64 = 0.2;

/// Flow tolerance (MW) outside a band before a shifter moves.
const BAND_TOLERANCE: f64 = 0.1;

/// Largest angle change (degrees) in one pass.
const MAX_STEP: f64 = 10.0;

/// Smallest flow change (MW per degree) for a shifter to keep controlling.
const MIN_EFFECT: f64 = 0.01;

/// Moves phase shifters between Newton solves.
#[derive(Clone)]
pub(crate) struct FlowController<'a> {
    shifters: Vec<ShifterState<'a>>,
    count: usize,
}

/// A shifter's branch at the present voltages.
struct Terminal {
    f: usize,
    t: usize,
    /// Flow into the branch at its `from` end, per unit.
    flow: f64,
    /// `Vf · conj(yft · Vt)` and `Vt · conj(ytf · Vf)`.
    a: C,
    b: C,
    /// `Vf · conj(yff · Vf)` and `Vt · conj(ytt · Vt)`.
    own: [C; 2],
}

impl<'a> FlowController<'a> {
    pub fn new(moving: &[(usize, &'a PhaseShifter)], controls: &FlowControls) -> Self {
        Self {
            shifters: moving
                .iter()
                .map(|&(index, shifter)| ShifterState {
                    index,
                    shifter,
                    angle: shifter.angle,
                    last: 0,
                    reversals: 0,
                    held: false,
                })
                .collect(),
            count: controls.phase_shifters.len(),
        }
    }

    /// Whether there is anything to move.
    pub fn is_empty(&self) -> bool {
        self.shifters.is_empty()
    }

    /// Phase shifters in flow control.
    pub fn devices(&self) -> usize {
        self.shifters.len()
    }

    /// One control pass at voltages `v` with bus kinds `kinds`: move every
    /// shifter whose flow is outside its band, updating the branches'
    /// `shift` (and `tap`, for asymmetric shifters) in `case`. Returns the
    /// number moved.
    ///
    /// The moves come from the flows' sensitivities to the angles through
    /// the whole network (the power-flow Jacobian), solved together, so
    /// that shifters in parallel or in series share the correction instead
    /// of fighting over it.
    pub fn pass(
        &mut self,
        case: &mut Case,
        net: &Network,
        v: &[C],
        kinds: &[BusType],
    ) -> Result<usize, Error> {
        let mut edge_of = std::collections::HashMap::new();
        for s in &self.shifters {
            edge_of.insert(s.shifter.branch, None);
        }
        for (k, e) in net.edges.iter().enumerate() {
            if let Some(slot) = edge_of.get_mut(&e.original) {
                *slot = Some(k);
            }
        }
        let terminal = |branch: usize| {
            edge_of[&branch].map(|k: usize| {
                let e = &net.edges[k];
                let own = [
                    v[e.f] * (e.yff * v[e.f]).conj(),
                    v[e.t] * (e.ytt * v[e.t]).conj(),
                ];
                let a = v[e.f] * (e.yft * v[e.t]).conj();
                Terminal {
                    f: e.f,
                    t: e.t,
                    flow: (own[0] + a).re,
                    a,
                    b: v[e.t] * (e.ytf * v[e.f]).conj(),
                    own,
                }
            })
        };
        // Shifters outside their bands, with their terminals and targets.
        let base = case.base_mva;
        let mut out = Vec::new();
        for (k, s) in self.shifters.iter().enumerate() {
            if s.held || s.reversals >= MAX_REVERSALS {
                continue;
            }
            let Some(term) = terminal(s.shifter.branch) else {
                continue;
            };
            let flow = term.flow * base;
            let (lo, hi) = s.shifter.band();
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
        let slopes = sensitivities(net, v, kinds, &out.iter().map(|o| &o.1).collect::<Vec<_>>())?;
        // MW per degree.
        let scale = base * std::f64::consts::PI / 180.0;
        let m = out.len();
        let mut matrix: Vec<Vec<f64>> = slopes
            .iter()
            .map(|row| row.iter().map(|x| x * scale).collect())
            .collect();
        // A shifter that barely moves its own flow is held, and left out.
        let mut keep = vec![true; m];
        for i in 0..m {
            if matrix[i][i].abs() < MIN_EFFECT {
                keep[i] = false;
                let s = &mut self.shifters[out[i].0];
                s.held = true;
                let branch = &case.branches[s.shifter.branch];
                log::info!(
                    "  phase shifter {}-{} barely moves its flow ({:.3} MW per degree); holding it",
                    branch.from,
                    branch.to,
                    matrix[i][i]
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
            // Shifters that cannot be moved independently (in series on
            // one path, say): each takes its own share instead.
            (0..m).map(|i| rhs[i] / matrix[i][i] / m as f64).collect()
        });
        // Scale the whole move down together when one would exceed the
        // per-pass limit, so the shifters keep their proportions.
        let largest = changes.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
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
            let s = &mut self.shifters[*k];
            let (amin, amax) = s.shifter.range();
            let next = (s.angle + changes[i] * factor).clamp(amin, amax);
            if (next - s.angle).abs() < 1e-9 {
                continue;
            }
            let dir = if next > s.angle { 1 } else { -1 };
            if s.last != 0 && s.last != dir {
                s.reversals += 1;
            }
            s.last = dir;
            let (lo, hi) = s.shifter.band();
            let branch = &mut case.branches[s.shifter.branch];
            let text = format!(
                "  phase shifter {}-{}: angle {:.3} -> {next:.3} deg, flow {flow:.1} MW is outside {lo:.1}-{hi:.1}",
                branch.from, branch.to, s.angle
            );
            if moved < crate::solver::DETAIL_LINES {
                log::info!("{text}");
            } else {
                log::debug!("{text}");
            }
            s.angle = next;
            branch.shift = next;
            branch.tap = s.shifter.tap_at(next, branch.tap);
            moved += 1;
        }
        Ok(moved)
    }

    /// Go back to `earlier` after the solve that the last pass led to
    /// failed, holding every shifter that pass moved. Returns the number held.
    pub fn undo(&mut self, earlier: Self) -> usize {
        let moved: Vec<bool> = (self.shifters.iter().zip(&earlier.shifters))
            .map(|(now, then)| now.angle != then.angle)
            .collect();
        *self = earlier;
        let mut held = 0;
        for (s, _) in self.shifters.iter_mut().zip(&moved).filter(|x| *x.1) {
            s.held = true;
            held += 1;
        }
        held
    }

    /// Final angles for every listed phase shifter, with locked and offline
    /// ones at their starting angle.
    pub fn values(&self, controls: &FlowControls) -> Vec<f64> {
        let mut angles: Vec<f64> = controls.phase_shifters.iter().map(|p| p.angle).collect();
        angles.resize(self.count, f64::NAN);
        for s in &self.shifters {
            angles[s.index] = s.angle;
        }
        angles
    }
}

/// `slopes[i][j]`: change of shifter `i`'s flow (per unit) per radian of
/// shifter `j`'s angle, with the rest of the network responding as the
/// Newton equations say: generators hold their buses' voltages and the
/// reference buses take up the change in losses.
fn sensitivities(
    net: &Network,
    v: &[C],
    kinds: &[BusType],
    terms: &[&Terminal],
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
    let m = terms.len();
    let mut slopes = vec![vec![0.0; m]; m];
    // Direct effect of each angle on its own branch's flow, with every
    // voltage held: d(Vf conj(yft Vt))/dphi = -j a, so dP/dphi = Im(a).
    for (i, t) in terms.iter().enumerate() {
        slopes[i][i] = t.a.im;
    }
    if n == 0 {
        return Ok(slopes);
    }
    let entries = crate::solver::jacobian(net, v, &angles, &pq);
    let a = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &entries)
        .map_err(|e| Error::Numerical(format!("sparse assembly: {e:?}")))?;
    let lu = a
        .sp_lu()
        .map_err(|e| Error::Numerical(format!("singular Jacobian for phase shifters: {e:?}")))?;
    // Right-hand sides: the change in the power each terminal bus sends
    // into the branch per radian, -j a at the from end and j b at the to
    // end. The Newton step dx = -J^-1 dF moves the state to absorb it.
    let rhs = Mat::from_fn(n, m, |row, j| {
        let t = terms[j];
        let mut x = 0.0;
        for (bus, ds) in [
            (t.f, C::new(0.0, -1.0) * t.a),
            (t.t, C::new(0.0, 1.0) * t.b),
        ] {
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
    // Each flow's change with the state: P = Re(own_f + a) with
    // d/dθf = Re(j a), d/dθt = Re(-j a), d/d|Vf| = Re(2 own_f + a)/|Vf|,
    // d/d|Vt| = Re(a)/|Vt|.
    for (i, t) in terms.iter().enumerate() {
        let vf = v[t.f].norm();
        let vt = v[t.t].norm();
        let grads = [
            (angle_of[t.f], -t.a.im),
            (angle_of[t.t], t.a.im),
            (magnitude_of[t.f], (2.0 * t.own[0].re + t.a.re) / vf),
            (magnitude_of[t.t], t.a.re / vt),
        ];
        for j in 0..m {
            for &(col, g) in &grads {
                if let Some(col) = col {
                    slopes[i][j] += g * dx[(col, j)];
                }
            }
        }
    }
    if slopes.iter().flatten().any(|x| !x.is_finite()) {
        return Err(Error::Numerical(
            "non-finite phase shifter sensitivities".into(),
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
