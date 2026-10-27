//! Decoupled AC iterations: fixed-slope decoupled Newton and the fast
//! decoupled load flow (XB and BX versions).
//!
//! Both solve the active power–angle and reactive power–magnitude halves of
//! the Newton equations in turn, each with a matrix that is factored once
//! per pass and reused. The fast decoupled matrices follow B. Stott and
//! O. Alsac, "Fast Decoupled Load Flow", IEEE Trans. PAS-93 (1974), and
//! R. A. M. van Amerongen, "A general-purpose version of the fast decoupled
//! load flow", IEEE Trans. Power Systems 4(2) (1989), as in MATPOWER's
//! `makeB` (BSD licence; see NOTICE). The fixed-slope matrices are the two
//! diagonal blocks of the Newton Jacobian at the start of the pass, the
//! magnitude block taken without series resistance: with the exact block,
//! the halves of a decoupled iteration can push each other apart on lines
//! whose resistance matches their reactance (IEEE 300-bus buses 9043-9044,
//! the synthetic 70-82k bus cases), while leaving resistance out of one of
//! the two matrices keeps them converging, as van Amerongen showed for the
//! fast decoupled BX version.
use super::{
    DcTerminal, Pass, admittance_jacobian, amount, count, dc_jacobian, largest, layout_jacobian,
    set_voltages, step_scale,
};
use crate::{Error, SolveStatus};
use faer::{
    Mat,
    prelude::Solve,
    sparse::{SparseColMat, Triplet, linalg::solvers::Lu},
};
use num_complex::Complex64 as C;
use std::collections::BTreeMap;

/// Which matrices the halves use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Matrices {
    /// Diagonal blocks of the Newton Jacobian at the starting voltages.
    FixedSlope,
    /// Fast decoupled, resistance left out of the angle matrix.
    Xb,
    /// Fast decoupled, resistance left out of the magnitude matrix.
    Bx,
}

impl Matrices {
    fn name(self) -> &'static str {
        match self {
            Self::FixedSlope => "Fixed-slope",
            Self::Xb | Self::Bx => "Fast decoupled",
        }
    }
}

/// How a decoupled solve ended.
pub(super) enum Outcome {
    /// Converged or reached the iteration limit: status, iterations and
    /// final mismatch (p.u.).
    Finished(SolveStatus, usize, f64),
    /// A watched solve stopped because convergence slowed down or went the
    /// wrong way, leaving the voltages at the best iterate: iterations and
    /// that iterate's mismatch.
    GaveUp(usize, f64),
}

/// A factored square sparse matrix.
struct Factored {
    n: usize,
    lu: Option<Lu<usize, f64>>,
}

impl Factored {
    fn new(n: usize, entries: &[Triplet<usize, usize, f64>], what: &str) -> Result<Self, Error> {
        if n == 0 {
            return Ok(Self { n, lu: None });
        }
        if entries.iter().any(|t| !t.val.is_finite()) {
            return Err(Error::Numerical(format!("non-finite {what} matrix")));
        }
        let a = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, entries)
            .map_err(|e| Error::Numerical(format!("sparse assembly: {e:?}")))?;
        let lu = a.sp_lu().map_err(|e| {
            Error::Numerical(format!("singular or unfactorable {what} matrix: {e:?}"))
        })?;
        Ok(Self { n, lu: Some(lu) })
    }

    fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>, Error> {
        let Some(lu) = &self.lu else {
            return Ok(Vec::new());
        };
        if rhs.iter().any(|x| !x.is_finite()) {
            return Err(Error::Numerical("non-finite power mismatch".into()));
        }
        let x = lu.solve(Mat::from_fn(self.n, 1, |i, _| rhs[i]));
        let out: Vec<_> = (0..self.n).map(|i| x[(i, 0)]).collect();
        if out.iter().any(|x| !x.is_finite()) {
            return Err(Error::Numerical(
                "decoupled solve produced non-finite values".into(),
            ));
        }
        Ok(out)
    }
}

/// The two factored matrices: angle rows and columns, then reactive rows
/// and magnitude columns, both numbered from zero.
struct Halves {
    p: Factored,
    q: Factored,
}

/// Fast decoupled susceptance matrices B′ and B″ as entries between
/// network buses `(i, j, B′ij, B″ij)`; B = −Im(Y) of a simplified network.
fn fast_matrices(pass: &Pass<'_>, kind: Matrices) -> Vec<(usize, usize, f64, f64)> {
    let (net, case) = (pass.net, pass.case);
    let mut entries = Vec::with_capacity(4 * net.edges.len() + net.external.len());
    for e in &net.edges {
        let br = &case.branches[e.original];
        let full = C::new(1.0, 0.0) / C::new(br.r, br.x);
        let reactive = reactance_only(br);
        let (angle, magnitude) = match kind {
            Matrices::Bx => (full, reactive),
            _ => (reactive, full),
        };
        // B′: no shunts, unity taps and no phase shift.
        let bp = -angle.im;
        // B″: charging and off-nominal taps, but no phase shift.
        let tap = if br.tap == 0.0 { 1.0 } else { br.tap };
        let ytt = magnitude + C::new(0.0, br.b / 2.0);
        let yff = ytt / (tap * tap);
        let yft = -magnitude / tap;
        entries.push((e.f, e.f, bp, -yff.im));
        entries.push((e.t, e.t, bp, -ytt.im));
        entries.push((e.f, e.t, -bp, -yft.im));
        entries.push((e.t, e.f, -bp, -yft.im));
    }
    for (i, &k) in net.external.iter().enumerate() {
        let b = &case.buses[k];
        // Bus shunts and constant-impedance load, then constant-current load,
        // whose reactive power grows with |V|.
        let shunt = -(b.bs - b.qd_z) / case.base_mva;
        entries.push((i, i, 0.0, shunt + net.current_loads[i].im));
    }
    entries
}

/// Series admittance of `br` without its resistance; a branch without
/// reactance keeps its impedance magnitude instead.
fn reactance_only(br: &crate::Branch) -> C {
    if br.x != 0.0 {
        C::new(0.0, -1.0 / br.x)
    } else {
        C::new(0.0, -(C::new(1.0, 0.0) / C::new(br.r, br.x)).norm())
    }
}

/// The bus admittance rows with every branch's series resistance left out.
fn without_resistance(pass: &Pass<'_>) -> Vec<BTreeMap<usize, C>> {
    let mut y = pass.net.y.clone();
    for e in &pass.net.edges {
        let br = &pass.case.branches[e.original];
        // Change in series admittance, through the same tap as the branch.
        let delta = reactance_only(br) - C::new(1.0, 0.0) / C::new(br.r, br.x);
        let tap = C::from_polar(
            if br.tap == 0.0 { 1.0 } else { br.tap },
            br.shift.to_radians(),
        );
        for (r, c, d) in [
            (e.f, e.f, delta / tap.norm_sqr()),
            (e.f, e.t, -delta / tap.conj()),
            (e.t, e.f, -delta / tap),
            (e.t, e.t, delta),
        ] {
            *y[r].entry(c).or_default() += d;
        }
    }
    y
}

/// Build and factor the matrices for `kind` at voltages `v`.
fn halves(
    pass: &Pass<'_>,
    v: &[C],
    terminals: &[DcTerminal<'_>],
    kind: Matrices,
) -> Result<Halves, Error> {
    let layout = pass.layout;
    let na = layout.angles.len();
    let nq = layout.magnitudes.len();
    let (mut p, mut q) = (Vec::new(), Vec::new());
    let mut split = |t: Triplet<usize, usize, f64>| {
        if t.row < na && t.col < na {
            p.push(t);
        } else if t.row >= na && t.col >= na {
            q.push(Triplet::new(t.row - na, t.col - na, t.val));
        }
    };
    if kind == Matrices::FixedSlope {
        // Angle block from the network as it is, magnitude block from the
        // network without series resistance.
        for t in layout_jacobian(pass.net, v, layout) {
            if t.row < na {
                split(t);
            }
        }
        let y = without_resistance(pass);
        for t in admittance_jacobian(&y, &pass.net.current_loads, v, layout) {
            if t.row >= na {
                split(t);
            }
        }
    } else {
        let at = layout.indices(v.len());
        for (i, j, bp, bpp) in fast_matrices(pass, kind) {
            if let (Some(r), Some(c)) = (at.angle[i], at.angle[j])
                && bp != 0.0
            {
                split(Triplet::new(r, c, bp));
            }
            if let Some(c) = at.magnitude[j] {
                for &(r, k) in &at.reactive[i] {
                    split(Triplet::new(r, c, k * bpp));
                }
            }
        }
    }
    // Converter reactive power moves with the terminal voltages.
    for t in dc_jacobian(terminals, v, layout, pass.case.base_mva)? {
        split(t);
    }
    Ok(Halves {
        p: Factored::new(na, &p, "angle")?,
        q: Factored::new(nq, &q, "magnitude")?,
    })
}

/// A watched solve hands over to Newton when, at the rate of its last two
/// iterations, it would still need more than this many iterations to reach
/// the tolerance: by then Newton, with its quadratic convergence, is
/// cheaper despite factoring the whole Jacobian at every step.
const WATCH_PREDICTED: f64 = 20.0;

/// A watched solve gives up at once when one iteration multiplies the
/// mismatch by more than this, and otherwise on the second iteration that
/// does not reduce it: a single rise often comes before fast convergence
/// from a poor start.
const WATCH_GROWTH: f64 = 10.0;

/// Times the fixed-slope matrices may be taken again in one pass.
const MAX_REFRESHES: usize = 3;

/// Solve one pass with decoupled iterations. With `watch`, give up when
/// convergence slows (see [`Outcome::GaveUp`]) instead of carrying on to the
/// iteration limit.
pub(super) fn solve(
    pass: &Pass<'_>,
    v: &mut [C],
    terminals: &mut [DcTerminal<'_>],
    kind: Matrices,
    watch: bool,
) -> Result<Outcome, Error> {
    let Pass {
        layout,
        opt,
        tolerance,
        ..
    } = *pass;
    let na = layout.angles.len();
    let name = kind.name();
    let limit = opt.decoupled_iterations;
    log::debug!(
        "starting {name} solve ({} angle variables, {} magnitude variables, {} remotely held buses, {} detailed DC lines)",
        na,
        layout.magnitudes.len(),
        layout.targets.len(),
        terminals.len()
    );
    let (mut f, mut residual) = pass.evaluate(v, terminals)?;
    let mut matrices = halves(pass, v, terminals, kind)?;
    let scaled = kind != Matrices::FixedSlope;
    // Fast decoupled halves solve ΔP/|V| and ΔQ/|V|; reactive-sharing rows,
    // which mix two buses, stay as they are.
    let scale = |f: &[f64], rows: &[usize], v: &[C]| -> Vec<f64> {
        f.iter()
            .enumerate()
            .map(|(k, &x)| {
                let s = if scaled && k < rows.len() {
                    v[rows[k]].norm()
                } else {
                    1.0
                };
                -x / s
            })
            .collect()
    };
    let mut best = (residual, v.to_vec());
    let (mut previous, mut slow, mut grew, mut refreshes) = (residual, 0, 0, 0);
    for iteration in 0..=limit {
        if opt.cancelled() {
            log::info!("solve cancelled before {name} iteration {iteration}");
            return Err(Error::Cancelled);
        }
        if residual < tolerance {
            log::info!(
                "{name} iteration {iteration}: {}; converged (largest mismatch {residual:.2e} pu, tolerance {tolerance:.0e})",
                pass.mismatch_text(&f)
            );
            return Ok(Outcome::Finished(
                SolveStatus::Converged,
                iteration,
                residual,
            ));
        }
        if iteration == limit {
            if watch {
                v.copy_from_slice(&best.1);
                return Ok(Outcome::GaveUp(iteration, best.0));
            }
            log::warn!(
                "{name} iteration {iteration}: {}; stopped at the iteration limit with largest mismatch {residual:.2e} pu",
                pass.mismatch_text(&f)
            );
            return Ok(Outcome::Finished(
                SolveStatus::IterationLimit,
                iteration,
                residual,
            ));
        }
        let mismatch_now = pass.mismatch_text(&f);
        // Angle half.
        let dva = matrices.p.solve(&scale(&f[..na], &layout.angles, v))?;
        let mut va: Vec<_> = v.iter().map(|z| z.arg()).collect();
        let mut vm: Vec<_> = v.iter().map(|z| z.norm()).collect();
        for (k, &i) in layout.angles.iter().enumerate() {
            va[i] += dva[k];
        }
        set_voltages(v, &vm, &va)?;
        (f, residual) = pass.evaluate(v, terminals)?;
        // Magnitude half, unless the angle half already converged.
        let mut step = String::new();
        let mut dvm = Vec::new();
        if residual >= tolerance {
            dvm = matrices.q.solve(&scale(&f[na..], &layout.q_rows, v))?;
            let s = step_scale(&dvm, &layout.magnitudes, &vm);
            for (k, &i) in layout.magnitudes.iter().enumerate() {
                vm[i] += s * dvm[k];
            }
            for d in &mut dvm {
                *d *= s;
            }
            set_voltages(v, &vm, &va)?;
            (f, residual) = pass.evaluate(v, terminals)?;
        }
        let (dv, at_vm) = largest(&dvm, &layout.magnitudes);
        if let Some(i) = at_vm {
            step += &format!(
                "; correction: largest |dV| {} pu at bus {}",
                amount(dv.abs()),
                pass.bus_id(i)
            );
        }
        let (da, at_va) = largest(&dva, &layout.angles);
        if let Some(i) = at_va {
            step += &format!(
                ", largest |dθ| {}° at bus {}",
                amount(da.abs().to_degrees()),
                pass.bus_id(i)
            );
        }
        log::info!("{name} iteration {iteration}: {mismatch_now}{step}");
        if residual < best.0 {
            best = (residual, v.to_vec());
        }
        let ratio = residual / previous;
        previous = residual;
        if ratio >= 1.0 {
            grew += 1;
        }
        if watch {
            // Iterations still needed at this rate of convergence.
            let needed = if ratio < 1.0 {
                (tolerance / residual).ln() / ratio.ln()
            } else {
                f64::INFINITY
            };
            slow = if needed > WATCH_PREDICTED {
                slow + 1
            } else {
                0
            };
            if !residual.is_finite() || ratio > WATCH_GROWTH || grew >= 2 || slow >= 2 {
                log::debug!(
                    "{name} convergence ratio {ratio:.3} after {}",
                    count(iteration + 1, "iteration", "iterations")
                );
                v.copy_from_slice(&best.1);
                return Ok(Outcome::GaveUp(iteration + 1, best.0));
            }
        }
        if kind == Matrices::FixedSlope && ratio >= 1.0 && refreshes < MAX_REFRESHES {
            refreshes += 1;
            log::info!(
                "{name}: the mismatch grew, so the matrices are taken again at the present voltages"
            );
            matrices = halves(pass, v, terminals, kind)?;
        }
    }
    unreachable!()
}
