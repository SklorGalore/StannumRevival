// Formulations and result allocation adapted from MATPOWER 8.1; see NOTICE.
use crate::{
    BusType, Case, Error, Generator,
    dc::{self, LccLine, LccSolution, TapTargets, Taps},
    network::Network,
    vsc::{self, DcState, VscAcControl, VscLine, VscSolution},
};
use faer::{
    Mat,
    prelude::Solve,
    sparse::{SparseColMat, Triplet},
};
use num_complex::Complex64 as C;

/// Power-flow formulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// AC power/polar Newton–Raphson with constant-power loads.
    Ac,
    /// Linear DC power flow for active power and bus angles.
    Dc,
}

/// AC voltage initialization. Both choices apply generator setpoints at controlled buses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initialization {
    /// Start from the voltage magnitudes and angles supplied by the case.
    Case,
    /// Start each island at 1 p.u. and its reference-bus angle.
    Flat,
}

/// Solver settings; defaults to AC with a 1e-8 p.u. tolerance and 10 Newton steps.
#[derive(Clone, Debug)]
pub struct PowerFlowOptions {
    /// Formulation; defaults to [`Model::Ac`].
    pub model: Model,
    /// AC starting voltages; defaults to [`Initialization::Case`]. Ignored for DC.
    pub initialization: Initialization,
    /// Infinity norm of the constrained power mismatch, in per unit.
    pub tolerance: f64,
    /// Newton steps per reactive-limit pass. Zero evaluates the initial state.
    pub max_iterations: usize,
    /// Fix AC reactive-limit violations and switch affected buses to PQ.
    /// Defaults to false; enabling this for DC returns [`Error::Unsupported`].
    pub enforce_q_limits: bool,
    /// Reactive-limit violation tolerance in MVAr.
    pub q_limit_tolerance: f64,
    /// How DC lines are modelled; defaults to [`DcModel::Simplified`].
    pub dc_model: DcModel,
}

/// DC line model for AC solves.
#[derive(Clone, Debug, Default)]
pub enum DcModel {
    /// Every `case.dcline` row is MATPOWER's generator pair: scheduled P,
    /// linear losses, and terminals that hold voltage within their Q range.
    #[default]
    Simplified,
    /// The listed rows use the detailed two-terminal LCC model (see
    /// [`LccLine`]) or VSC model (see [`VscLine`]); other rows stay
    /// simplified. The LCC operating point is recomputed from the terminal
    /// voltages at every Newton step, and the converter terminals become
    /// voltage-dependent loads that absorb reactive power. VSC lines keep
    /// their AC voltage or power factor control, with active power from the
    /// DC-side solution and passes that enforce converter MVA and current
    /// limits. DC (linear) solves use the simplified model.
    Detailed {
        /// Detailed LCC data, each naming its `case.dcline` row.
        lines: Vec<LccLine>,
        /// Detailed VSC data, each naming its `case.dcline` row.
        vsc_lines: Vec<VscLine>,
        /// Step converter taps between Newton solves to hold the scheduled
        /// DC voltage and keep firing angles in range (PSS/E's DC tap
        /// adjustment). When false, the taps stay at their case values.
        adjust_taps: bool,
    },
}
impl Default for PowerFlowOptions {
    fn default() -> Self {
        Self {
            model: Model::Ac,
            initialization: Initialization::Case,
            tolerance: 1e-8,
            max_iterations: 10,
            enforce_q_limits: false,
            q_limit_tolerance: 5e-6,
            dc_model: DcModel::Simplified,
        }
    }
}

/// Solver outcome, distinct from input and numerical errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveStatus {
    /// The constrained mismatch meets the requested tolerance.
    Converged,
    /// Newton reached its step limit; the result contains the final iterate.
    IterationLimit,
    /// Reactive-limit enforcement cannot retain a feasible controlled/reference bus.
    ReactiveLimitsInfeasible,
}

/// Final case and diagnostics, including results from unsuccessful iterations.
#[derive(Clone, Debug)]
pub struct PowerFlowResult {
    /// Solved voltages and generator outputs, in original case order and units.
    pub case: Case,
    /// `[PF, QF, PT, QT]` in MW/MVAr, in original branch order.
    /// Positive values enter the branch at that end; excluded branches are zero.
    pub branch_flows: Vec<[f64; 4]>,
    /// Outcome to check even when [`solve`] returns `Ok`.
    pub status: SolveStatus,
    /// Total Newton steps across all reactive-limit passes (one for DC).
    pub iterations: usize,
    /// Infinity norm of the final constrained mismatch in per unit.
    /// This does not measure reactive-limit feasibility.
    pub residual: f64,
    /// Number of passes that fixed one or more generators at reactive limits.
    pub q_limit_passes: usize,
    /// Operating points of the detailed DC lines, in the order given by
    /// [`DcModel::Detailed`]. Empty for the simplified model.
    pub lcc: Vec<LccSolution>,
    /// Number of passes that changed one or more DC converter taps.
    pub dc_tap_passes: usize,
    /// Operating points of the detailed VSC lines, in the order given by
    /// [`DcModel::Detailed`]. Empty for the simplified model.
    pub vsc: Vec<VscSolution>,
    /// Number of passes that reduced VSC output to converter limits.
    pub vsc_limit_passes: usize,
}

fn linear(
    n: usize,
    entries: &[Triplet<usize, usize, f64>],
    rhs: &[f64],
) -> Result<Vec<f64>, Error> {
    log::trace!(
        "solving sparse linear system (dimension={n}, entries={})",
        entries.len()
    );
    if n == 0 {
        return Ok(Vec::new());
    }
    if rhs.iter().any(|x| !x.is_finite()) || entries.iter().any(|t| !t.val.is_finite()) {
        return Err(Error::Numerical("non-finite linear system".into()));
    }
    let a = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, entries)
        .map_err(|e| Error::Numerical(format!("sparse assembly: {e:?}")))?;
    let lu = a
        .sp_lu()
        .map_err(|e| Error::Numerical(format!("singular or unfactorable system: {e:?}")))?;
    let x = lu.solve(Mat::from_fn(n, 1, |i, _| rhs[i]));
    let out: Vec<_> = (0..n).map(|i| x[(i, 0)]).collect();
    if out.iter().any(|x| !x.is_finite()) {
        return Err(Error::Numerical(
            "linear solve produced non-finite values".into(),
        ));
    }
    let mut residual = rhs.to_vec();
    let mut scale = rhs.iter().fold(0.0_f64, |a, &v| a.max(v.abs()));
    for t in entries {
        residual[t.row] -= t.val * out[t.col];
        scale = scale.max((t.val * out[t.col]).abs());
    }
    if residual.iter().any(|r| r.abs() > 1e-8 * (1.0 + scale)) {
        return Err(Error::Numerical(
            "linear solve failed residual check".into(),
        ));
    }
    Ok(out)
}

pub(crate) fn variables(kinds: &[BusType]) -> (Vec<usize>, Vec<usize>) {
    let angles = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == BusType::Pv)
        .chain(kinds.iter().enumerate().filter(|(_, k)| **k == BusType::Pq))
        .map(|(i, _)| i)
        .collect();
    let pq = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == BusType::Pq)
        .map(|(i, _)| i)
        .collect();
    (angles, pq)
}

fn mismatch(
    net: &Network,
    v: &[C],
    s: &[C],
    angles: &[usize],
    pq: &[usize],
) -> Result<(Vec<f64>, f64), Error> {
    let power = net.power(v);
    let f: Vec<_> = angles
        .iter()
        .map(|&i| (power[i] - s[i]).re)
        .chain(pq.iter().map(|&i| (power[i] - s[i]).im))
        .collect();
    if power.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
        return Err(Error::Numerical("non-finite power mismatch".into()));
    }
    let norm = f.iter().fold(0.0_f64, |a, &b| a.max(b.abs()));
    Ok((f, norm))
}

pub(crate) fn jacobian(
    net: &Network,
    v: &[C],
    angles: &[usize],
    pq: &[usize],
) -> Vec<Triplet<usize, usize, f64>> {
    let n = v.len();
    let mut ai = vec![None; n];
    let mut qi = vec![None; n];
    for (k, &i) in angles.iter().enumerate() {
        ai[i] = Some(k);
    }
    for (k, &i) in pq.iter().enumerate() {
        qi[i] = Some(k + angles.len());
    }
    let mut entries = Vec::new();
    for (i, row) in net.y.iter().enumerate() {
        let current: C = row.iter().map(|(&j, &y)| y * v[j]).sum();
        for (&j, &y) in row {
            let mut da = -C::i() * v[i] * (y * v[j]).conj();
            let mut dm = v[i] * (y * v[j] / v[j].norm()).conj();
            if i == j {
                da += C::i() * v[i] * current.conj();
                dm += v[i] / v[i].norm() * current.conj();
            }
            if let Some(r) = ai[i] {
                if let Some(c) = ai[j] {
                    entries.push(Triplet::new(r, c, da.re));
                }
                if let Some(c) = qi[j] {
                    entries.push(Triplet::new(r, c, dm.re));
                }
            }
            if let Some(r) = qi[i] {
                if let Some(c) = ai[j] {
                    entries.push(Triplet::new(r, c, da.im));
                }
                if let Some(c) = qi[j] {
                    entries.push(Triplet::new(r, c, dm.im));
                }
            }
        }
    }
    entries
}

/// A detailed DC line during an AC solve.
struct LccTerminal<'a> {
    line: &'a LccLine,
    /// Internal bus indices, rectifier then inverter.
    ends: [usize; 2],
    /// Converter base voltages in kV.
    kv: [f64; 2],
    taps: Taps,
    solution: LccSolution,
    targets: TapTargets,
}

impl LccTerminal<'_> {
    fn evaluate(&self, vm: [f64; 2]) -> (LccSolution, TapTargets) {
        dc::operating_point(self.line, vm[0], vm[1], self.kv, self.taps)
    }

    /// Per-unit injections at the two ends (negative: the converters load the bus).
    fn injections(solution: &LccSolution, base_mva: f64) -> [C; 2] {
        [
            -C::new(solution.rectifier_mw, solution.rectifier_mvar) / base_mva,
            C::new(solution.inverter_mw, -solution.inverter_mvar) / base_mva,
        ]
    }
}

/// A detailed VSC line during an AC solve: its terminal generators (from
/// end, then to end of its `dcline` row) and the scale on its power order.
pub(crate) struct VscTerminal<'a> {
    line: &'a VscLine,
    gens: [usize; 2],
    scale: f64,
    state: DcState,
    limited: [bool; 2],
}

impl VscTerminal<'_> {
    /// RAW converter index at row end `e` (0 from, 1 to).
    fn converter(&self, e: usize) -> usize {
        if e == 0 {
            self.line.from
        } else {
            1 - self.line.from
        }
    }

    /// Write the DC solution into the terminal generators.
    fn apply(&self, case: &mut Case) {
        for e in 0..2 {
            let c = self.converter(e);
            let p = self.state.injection_mw[c];
            let g = &mut case.generators[self.gens[e]];
            g.pg = p;
            let conv = &self.line.converters[c];
            if conv.ac_control == VscAcControl::PowerFactor {
                let q = conv.power_factor_q(p);
                (g.qg, g.qmin, g.qmax) = (q, q, q);
            }
        }
    }

    fn solution(&self, case: &Case) -> VscSolution {
        let (f, t) = (self.converter(0), self.converter(1));
        let s = &self.state;
        let current = if f == 0 { s.current_ka } else { -s.current_ka };
        VscSolution {
            row: self.line.row,
            current_ka: current,
            from_kv: s.kv[f],
            to_kv: s.kv[t],
            from_mw: -s.injection_mw[f],
            to_mw: s.injection_mw[t],
            from_mvar: case.generators[self.gens[0]].qg,
            to_mvar: case.generators[self.gens[1]].qg,
            from_loss_mw: s.loss_mw[f],
            to_loss_mw: s.loss_mw[t],
            line_loss_mw: self.line.resistance * current * current,
            order_scale: self.scale,
            from_limited: self.limited[0],
            to_limited: self.limited[1],
        }
    }
}

/// The reported operating point of a detailed VSC line that carries nothing.
fn idle_vsc(line: &VscLine) -> VscSolution {
    VscSolution {
        row: line.row,
        current_ka: 0.0,
        from_kv: 0.0,
        to_kv: 0.0,
        from_mw: 0.0,
        to_mw: 0.0,
        from_mvar: 0.0,
        to_mvar: 0.0,
        from_loss_mw: 0.0,
        to_loss_mw: 0.0,
        line_loss_mw: 0.0,
        order_scale: 1.0,
        from_limited: false,
        to_limited: false,
    }
}

/// Reduce VSC output where a converter exceeds its MVA or current limit.
/// Returns whether anything changed that needs another Newton solve.
fn vsc_limit_pass(
    terminals: &mut [VscTerminal<'_>],
    net: &Network,
    case: &mut Case,
    v: &[C],
    enforce_q_limits: bool,
) -> Result<bool, Error> {
    let mut changed = false;
    for t in terminals.iter_mut() {
        let mut scale = t.scale;
        for e in 0..2 {
            let conv = &t.line.converters[t.converter(e)];
            let g = &case.generators[t.gens[e]];
            let Some(&i) = net.by_id.get(&g.bus) else {
                continue;
            };
            let kv = v[i].norm() * case.buses[net.external[i]].base_kv;
            let limit = conv.mva_limit(kv);
            let s = g.pg.hypot(g.qg);
            if s <= limit * (1.0 + 1e-6) + 1e-6 {
                continue;
            }
            t.limited[e] = true;
            let (p, q) = if conv.ac_control == VscAcControl::PowerFactor {
                // P and Q move together at a fixed power factor.
                (g.pg * limit / s, g.qg * limit / s)
            } else {
                vsc::reduce(g.pg, g.qg, limit, conv.power_weighting)
            };
            if g.pg.abs() > 1e-9 && p.abs() < g.pg.abs() * (1.0 - 1e-9) {
                scale = scale.min(t.scale * p.abs() / g.pg.abs());
            }
            if conv.ac_control == VscAcControl::Voltage && q.abs() < g.qg.abs() * (1.0 - 1e-9) {
                let g = &mut case.generators[t.gens[e]];
                let cap = q.abs();
                if g.qmax > cap || g.qmin < -cap {
                    g.qmax = g.qmax.min(cap);
                    g.qmin = g.qmin.max(-cap);
                    // Tighter limits only act when they are enforced.
                    changed |= enforce_q_limits;
                }
            }
        }
        if scale < t.scale - 1e-12 {
            t.scale = scale;
            t.state = t.line.dc_state(scale).map_err(Error::InvalidCase)?;
            t.apply(case);
            changed = true;
        }
    }
    Ok(changed)
}

/// Update every detailed DC line at voltages `v`; return per-bus injections.
fn lcc_injections(terminals: &mut [LccTerminal<'_>], v: &[C], base_mva: f64) -> Vec<C> {
    let mut s = vec![C::new(0.0, 0.0); v.len()];
    for t in terminals {
        (t.solution, t.targets) = t.evaluate([v[t.ends[0]].norm(), v[t.ends[1]].norm()]);
        for (k, z) in LccTerminal::injections(&t.solution, base_mva)
            .into_iter()
            .enumerate()
        {
            s[t.ends[k]] += z;
        }
    }
    s
}

/// Jacobian terms for the DC injections' dependence on terminal voltage
/// magnitudes, by central differences of the operating point.
fn lcc_jacobian(
    terminals: &[LccTerminal<'_>],
    v: &[C],
    angles: &[usize],
    pq: &[usize],
    base_mva: f64,
) -> Vec<Triplet<usize, usize, f64>> {
    if terminals.is_empty() {
        return Vec::new();
    }
    let n = v.len();
    let mut ai = vec![None; n];
    let mut qi = vec![None; n];
    for (k, &i) in angles.iter().enumerate() {
        ai[i] = Some(k);
    }
    for (k, &i) in pq.iter().enumerate() {
        qi[i] = Some(k + angles.len());
    }
    let mut entries = Vec::new();
    for t in terminals {
        let vm = [v[t.ends[0]].norm(), v[t.ends[1]].norm()];
        for j in 0..2 {
            let Some(col) = qi[t.ends[j]] else {
                continue;
            };
            let h = 1e-6 * vm[j].max(1e-3);
            let (mut plus, mut minus) = (vm, vm);
            plus[j] += h;
            minus[j] -= h;
            let sp = LccTerminal::injections(&t.evaluate(plus).0, base_mva);
            let sm = LccTerminal::injections(&t.evaluate(minus).0, base_mva);
            for k in 0..2 {
                // The mismatch is power(v) − s, so its derivative is −ds/dVm.
                let d = -(sp[k] - sm[k]) / (2.0 * h);
                if let Some(row) = ai[t.ends[k]] {
                    entries.push(Triplet::new(row, col, d.re));
                }
                if let Some(row) = qi[t.ends[k]] {
                    entries.push(Triplet::new(row, col, d.im));
                }
            }
        }
    }
    entries
}

fn newton(
    net: &Network,
    case: &Case,
    kinds: &[BusType],
    v: &mut [C],
    terminals: &mut [LccTerminal<'_>],
    opt: &PowerFlowOptions,
) -> Result<(SolveStatus, usize, f64), Error> {
    let (angles, pq) = variables(kinds);
    let base = net.injections(case);
    log::debug!(
        "starting Newton solve ({} angle variables, {} magnitude variables, {} detailed DC lines)",
        angles.len(),
        pq.len(),
        terminals.len()
    );
    for iteration in 0..=opt.max_iterations {
        let s = if terminals.is_empty() {
            base.clone()
        } else {
            let dc = lcc_injections(terminals, v, case.base_mva);
            base.iter().zip(dc).map(|(a, b)| a + b).collect()
        };
        let (f, residual) = mismatch(net, v, &s, &angles, &pq)?;
        log::debug!(
            "Newton iteration {iteration}: residual={residual:.6e} p.u., tolerance={:.6e}",
            opt.tolerance
        );
        if residual < opt.tolerance {
            log::debug!("Newton solve converged at iteration {iteration}: residual={residual:.6e}");
            return Ok((SolveStatus::Converged, iteration, residual));
        }
        if iteration == opt.max_iterations {
            log::warn!("Newton solve reached iteration limit {iteration}: residual={residual:.6e}");
            return Ok((SolveStatus::IterationLimit, iteration, residual));
        }
        let mut j = jacobian(net, v, &angles, &pq);
        j.extend(lcc_jacobian(terminals, v, &angles, &pq, case.base_mva));
        let dx = linear(f.len(), &j, &f.iter().map(|x| -x).collect::<Vec<_>>())?;
        let mut va: Vec<_> = v.iter().map(|z| z.arg()).collect();
        let mut vm: Vec<_> = v.iter().map(|z| z.norm()).collect();
        for (k, &i) in angles.iter().enumerate() {
            va[i] += dx[k];
        }
        for (k, &i) in pq.iter().enumerate() {
            vm[i] += dx[k + angles.len()];
        }
        for i in 0..v.len() {
            if !vm[i].is_finite() || vm[i] <= 0.0 || !va[i].is_finite() {
                return Err(Error::Numerical(
                    "Newton step produced a non-positive or non-finite voltage magnitude".into(),
                ));
            }
            v[i] = C::from_polar(vm[i], va[i]);
        }
    }
    unreachable!()
}

fn allocate(
    net: &Network,
    case: &mut Case,
    kinds: &[BusType],
    references: &[usize],
    v: &[C],
    terminals: &[LccTerminal<'_>],
) {
    let mut power = net.power(v);
    // Generators supply the network injection plus what the converters draw.
    for t in terminals {
        for (k, z) in LccTerminal::injections(&t.solution, case.base_mva)
            .into_iter()
            .enumerate()
        {
            power[t.ends[k]] -= z;
        }
    }
    for (i, gens) in net.gens.iter().enumerate() {
        if kinds[i] == BusType::Pq || gens.is_empty() {
            continue;
        }
        let b = &case.buses[net.external[i]];
        let total = power[i].im * case.base_mva + b.qd;
        let proxy = total.abs()
            + gens
                .iter()
                .map(|&g| {
                    let g = &case.generators[g];
                    (if g.qmin.is_finite() {
                        g.qmin.abs()
                    } else {
                        0.0
                    }) + (if g.qmax.is_finite() {
                        g.qmax.abs()
                    } else {
                        0.0
                    })
                })
                .sum::<f64>();
        let limits: Vec<_> = gens
            .iter()
            .map(|&g| {
                let g = &case.generators[g];
                (
                    if g.qmin.is_infinite() {
                        g.qmin.signum() * proxy
                    } else {
                        g.qmin
                    },
                    if g.qmax.is_infinite() {
                        g.qmax.signum() * proxy
                    } else {
                        g.qmax
                    },
                )
            })
            .collect();
        let lo: f64 = limits.iter().map(|x| x.0).sum();
        let hi: f64 = limits.iter().map(|x| x.1).sum();
        for (k, &g) in gens.iter().enumerate() {
            case.generators[g].qg = if gens.len() == 1 {
                total
            } else if (hi - lo).abs() < 10.0 * f64::EPSILON {
                limits[k].0 + (total - lo) / gens.len() as f64
            } else {
                limits[k].0 + (total - lo) / (hi - lo + f64::EPSILON) * (limits[k].1 - limits[k].0)
            };
        }
    }
    for &reference in references {
        let gens = &net.gens[reference];
        let other: f64 = gens.iter().skip(1).map(|&g| case.generators[g].pg).sum();
        case.generators[gens[0]].pg =
            power[reference].re * case.base_mva + case.buses[net.external[reference]].pd - other;
    }
}

fn dc(net: &Network, mut case: Case, opt: &PowerFlowOptions) -> Result<PowerFlowResult, Error> {
    let n = net.external.len();
    let vars: Vec<_> = (0..n).filter(|i| !net.references.contains(i)).collect();
    log::debug!(
        "starting DC solve ({n} buses, {} variables, {} branches)",
        vars.len(),
        net.edges.len()
    );
    let mut idx = vec![None; n];
    for (k, &i) in vars.iter().enumerate() {
        idx[i] = Some(k);
    }
    let mut entries = Vec::new();
    let mut va: Vec<_> = net
        .external
        .iter()
        .map(|&e| case.buses[e].va.to_radians())
        .collect();
    let mut injection: Vec<_> = net
        .injections(&case)
        .iter()
        .enumerate()
        .map(|(i, s)| s.re - case.buses[net.external[i]].gs / case.base_mva)
        .collect();
    let mut coeffs = Vec::new();
    for e in &net.edges {
        let b = &case.branches[e.original];
        if b.x == 0.0 {
            return Err(Error::InvalidCase(
                "DC flow requires nonzero active branch reactance".into(),
            ));
        }
        let coeff = 1.0 / b.x / if b.tap == 0.0 { 1.0 } else { b.tap };
        let shift = -coeff * b.shift.to_radians();
        coeffs.push((coeff, shift));
        injection[e.f] -= shift;
        injection[e.t] += shift;
        for (i, j, x) in [
            (e.f, e.f, coeff),
            (e.f, e.t, -coeff),
            (e.t, e.f, -coeff),
            (e.t, e.t, coeff),
        ] {
            if let Some(r) = idx[i] {
                if let Some(c) = idx[j] {
                    entries.push(Triplet::new(r, c, x));
                } else {
                    injection[i] -= x * va[j];
                }
            }
        }
    }
    let x = linear(
        vars.len(),
        &entries,
        &vars.iter().map(|&i| injection[i]).collect::<Vec<_>>(),
    )?;
    for (k, &i) in vars.iter().enumerate() {
        va[i] = x[k];
    }
    let mut flows = vec![[0.0; 4]; case.branches.len()];
    let mut balance = vec![0.0; n];
    for (e, (coeff, shift)) in net.edges.iter().zip(coeffs) {
        let p = coeff * (va[e.f] - va[e.t]) + shift;
        flows[e.original] = [p * case.base_mva, 0.0, -p * case.base_mva, 0.0];
        balance[e.f] += p;
        balance[e.t] -= p;
    }
    let expected = net.injections(&case);
    let residual = vars
        .iter()
        .map(|&i| {
            (balance[i] - expected[i].re + case.buses[net.external[i]].gs / case.base_mva).abs()
        })
        .fold(0.0_f64, f64::max);
    if !residual.is_finite() || residual >= opt.tolerance {
        return Err(Error::Numerical(
            "DC solution failed power balance tolerance".into(),
        ));
    }
    log::debug!("DC solve converged: residual={residual:.6e}");
    for &reference in &net.references {
        let gens = &net.gens[reference];
        let other: f64 = gens.iter().skip(1).map(|&g| case.generators[g].pg).sum();
        let b = &case.buses[net.external[reference]];
        case.generators[gens[0]].pg = balance[reference] * case.base_mva + b.pd + b.gs - other;
    }
    for (i, &e) in net.external.iter().enumerate() {
        case.buses[e].vm = 1.0;
        case.buses[e].va = va[i].to_degrees();
    }
    zero_offline(net, &mut case);
    Ok(PowerFlowResult {
        case,
        branch_flows: flows,
        status: SolveStatus::Converged,
        iterations: 1,
        residual,
        q_limit_passes: 0,
        lcc: Vec::new(),
        dc_tap_passes: 0,
        vsc: Vec::new(),
        vsc_limit_passes: 0,
    })
}

fn zero_offline(net: &Network, case: &mut Case) {
    for g in &mut case.generators {
        if !g.online || !net.by_id.contains_key(&g.bus) {
            g.pg = 0.0;
            g.qg = 0.0;
        }
    }
}

fn solve_core(
    input: &Case,
    opt: &PowerFlowOptions,
    lines: &[&LccLine],
    adjust_taps: bool,
    vsc_terminals: &mut [VscTerminal<'_>],
) -> Result<PowerFlowResult, Error> {
    if !opt.tolerance.is_finite()
        || opt.tolerance <= 0.0
        || !opt.q_limit_tolerance.is_finite()
        || opt.q_limit_tolerance < 0.0
    {
        return Err(Error::InvalidCase("invalid solver tolerance".into()));
    }
    if opt.model == Model::Dc && opt.enforce_q_limits {
        return Err(Error::Unsupported(
            "reactive-limit enforcement is only available for AC".into(),
        ));
    }
    let net = Network::new(input)?;
    let mut case = input.clone();
    if opt.model == Model::Dc {
        return dc(&net, case, opt);
    }
    let mut terminals = Vec::with_capacity(lines.len());
    for &line in lines {
        let mut ends = [0; 2];
        let mut kv = [0.0; 2];
        for (k, c) in [&line.rectifier, &line.inverter].into_iter().enumerate() {
            let Some(&i) = net.by_id.get(&c.bus) else {
                return Err(Error::InvalidCase(format!(
                    "DC line {} references an unknown or isolated bus {}",
                    line.name, c.bus
                )));
            };
            ends[k] = i;
            kv[k] = if c.base_kv > 0.0 {
                c.base_kv
            } else {
                case.buses[net.external[i]].base_kv
            };
            if kv[k].is_nan() || kv[k] <= 0.0 {
                return Err(Error::InvalidCase(format!(
                    "DC line {} needs a converter base voltage (EBAS or bus base kV)",
                    line.name
                )));
            }
        }
        let taps = [line.rectifier.tap, line.inverter.tap];
        let (solution, targets) = dc::operating_point(line, 1.0, 1.0, kv, taps);
        terminals.push(LccTerminal {
            line,
            ends,
            kv,
            taps,
            solution,
            targets,
        });
    }
    let mut kinds = net.kinds.clone();
    let mut references = net.references.clone();
    let mut v = vec![C::new(1.0, 0.0); net.external.len()];
    for (island, &reference) in net.islands.iter().zip(&net.references) {
        let reference_angle = case.buses[net.external[reference]].va.to_radians();
        for &i in island {
            let b = &case.buses[net.external[i]];
            v[i] = match opt.initialization {
                Initialization::Case => C::from_polar(b.vm, b.va.to_radians()),
                Initialization::Flat => C::from_polar(1.0, reference_angle),
            };
        }
    }
    for (i, gens) in net.gens.iter().enumerate() {
        if kinds[i] != BusType::Pq {
            for &g in gens {
                v[i] = C::from_polar(case.generators[g].vg, v[i].arg());
            }
        }
    }
    let (mut iterations, mut passes, mut tap_passes, mut vsc_passes) = (0, 0, 0, 0);
    let (status, residual) = loop {
        let (status, it, residual) = newton(&net, &case, &kinds, &mut v, &mut terminals, opt)?;
        iterations += it;
        allocate(&net, &mut case, &kinds, &references, &v, &terminals);
        if status != SolveStatus::Converged {
            break (status, residual);
        }
        if adjust_taps && tap_passes < MAX_TAP_PASSES {
            let mut changed = 0;
            for t in &mut terminals {
                let vm = [v[t.ends[0]].norm(), v[t.ends[1]].norm()];
                let next = dc::adjust_taps(t.line, &t.solution, &t.targets, vm, t.kv, t.taps);
                if next != t.taps {
                    t.taps = next;
                    changed += 1;
                }
            }
            if changed > 0 {
                tap_passes += 1;
                log::debug!("DC tap pass {tap_passes} changed taps on {changed} lines");
                if tap_passes == MAX_TAP_PASSES {
                    log::warn!(
                        "DC converter taps still moving after {MAX_TAP_PASSES} passes; holding them"
                    );
                }
                continue;
            }
        }
        if vsc_passes < MAX_TAP_PASSES
            && vsc_limit_pass(vsc_terminals, &net, &mut case, &v, opt.enforce_q_limits)?
        {
            vsc_passes += 1;
            log::debug!("VSC limit pass {vsc_passes} reduced converter output");
            continue;
        }
        if !opt.enforce_q_limits {
            break (status, residual);
        }
        let mut violations = Vec::new();
        let mut remaining = Vec::new();
        for (i, gens) in net.gens.iter().enumerate() {
            if kinds[i] != BusType::Pq {
                remaining.extend(gens.iter().copied());
            }
            for &g in gens {
                let gen_data = &case.generators[g];
                if gen_data.qg > gen_data.qmax + opt.q_limit_tolerance {
                    violations.push((i, g, gen_data.qmax, true));
                } else if gen_data.qg < gen_data.qmin - opt.q_limit_tolerance {
                    violations.push((i, g, gen_data.qmin, false));
                }
            }
        }
        if violations.is_empty() {
            break (status, residual);
        }
        log::warn!(
            "reactive-limit pass {} is enforcing {} generator violations",
            passes + 1,
            violations.len()
        );
        for island in &net.islands {
            let island_remaining: Vec<_> = remaining
                .iter()
                .copied()
                .filter(|g| island.iter().any(|&i| net.gens[i].contains(g)))
                .collect();
            let island_violations: Vec<_> = violations
                .iter()
                .filter(|violation| island.contains(&violation.0))
                .collect();
            let all_generators_violate_one_side = !island_remaining.is_empty()
                && island_remaining.len() == island_violations.len()
                && island_remaining
                    .iter()
                    .all(|g| island_violations.iter().any(|x| x.1 == *g))
                && island_violations
                    .iter()
                    .all(|x| x.3 == island_violations[0].3);
            let no_reference_candidate = island
                .iter()
                .all(|&i| kinds[i] == BusType::Pq || island_violations.iter().any(|v| v.0 == i));
            if all_generators_violate_one_side || no_reference_candidate {
                return finish_ac(
                    input,
                    &net,
                    case,
                    kinds,
                    references,
                    v,
                    SolveStatus::ReactiveLimitsInfeasible,
                    iterations,
                    residual,
                    [passes, tap_passes, vsc_passes],
                    &terminals,
                    vsc_terminals,
                );
            }
        }
        for &(i, g, limit, _) in &violations {
            case.generators[g].qg = limit;
            kinds[i] = BusType::Pq;
        }
        passes += 1;
        for (island, reference) in net.islands.iter().zip(&mut references) {
            if kinds[*reference] == BusType::Pq {
                let i = island
                    .iter()
                    .copied()
                    .find(|&i| kinds[i] == BusType::Pv)
                    .expect("reference candidate checked above");
                *reference = i;
                kinds[i] = BusType::Reference;
            }
        }
        if passes > case.generators.len() {
            return Err(Error::Numerical(
                "reactive-limit loop did not terminate".into(),
            ));
        }
    };
    finish_ac(
        input,
        &net,
        case,
        kinds,
        references,
        v,
        status,
        iterations,
        residual,
        [passes, tap_passes, vsc_passes],
        &terminals,
        vsc_terminals,
    )
}

/// Tap passes before the DC converter taps are held where they are.
const MAX_TAP_PASSES: usize = 20;

#[allow(clippy::too_many_arguments)]
fn finish_ac(
    input: &Case,
    net: &Network,
    mut case: Case,
    kinds: Vec<BusType>,
    references: Vec<usize>,
    v: Vec<C>,
    status: SolveStatus,
    iterations: usize,
    residual: f64,
    [passes, tap_passes, vsc_passes]: [usize; 3],
    terminals: &[LccTerminal<'_>],
    vsc_terminals: &[VscTerminal<'_>],
) -> Result<PowerFlowResult, Error> {
    let mut offsets = vec![0.0; net.external.len()];
    for ((island, &original), &reference) in
        net.islands.iter().zip(&net.references).zip(&references)
    {
        let offset = if passes > 0 && reference != original {
            input.buses[net.external[original]].va - v[original].arg().to_degrees()
        } else {
            0.0
        };
        for &i in island {
            offsets[i] = offset;
        }
    }
    for (i, &e) in net.external.iter().enumerate() {
        case.buses[e].vm = v[i].norm();
        case.buses[e].va = v[i].arg().to_degrees() + offsets[i];
        case.buses[e].kind = kinds[i];
    }
    let mut flows = vec![[0.0; 4]; case.branches.len()];
    for e in &net.edges {
        let sf = v[e.f] * (e.yff * v[e.f] + e.yft * v[e.t]).conj() * case.base_mva;
        let st = v[e.t] * (e.ytf * v[e.f] + e.ytt * v[e.t]).conj() * case.base_mva;
        flows[e.original] = [sf.re, sf.im, st.re, st.im];
    }
    zero_offline(net, &mut case);
    let vsc = vsc_terminals.iter().map(|t| t.solution(&case)).collect();
    Ok(PowerFlowResult {
        case,
        branch_flows: flows,
        status,
        iterations,
        residual,
        q_limit_passes: passes,
        lcc: terminals.iter().map(|t| t.solution.clone()).collect(),
        dc_tap_passes: tap_passes,
        vsc,
        vsc_limit_passes: vsc_passes,
    })
}

pub(crate) struct DcLineGenerators {
    row: usize,
    from_generator: usize,
    to_generator: usize,
}

fn dc_bus(value: f64) -> Option<u32> {
    if value.is_finite() && value >= 1.0 && value <= u32::MAX as f64 && value.fract() == 0.0 {
        Some(value as u32)
    } else {
        None
    }
}

pub(crate) fn add_dc_line_generators(case: &mut Case) -> Result<Vec<DcLineGenerators>, Error> {
    add_dc_line_generators_except(case, &[])
}

/// As [`add_dc_line_generators`], leaving out the rows marked in `skip`
/// (detailed DC lines), which are still validated.
fn add_dc_line_generators_except(
    case: &mut Case,
    skip: &[bool],
) -> Result<Vec<DcLineGenerators>, Error> {
    let Some(lines) = &case.dcline else {
        return Ok(Vec::new());
    };
    let buses: std::collections::HashMap<_, _> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, bus)| (bus.id, i))
        .collect();
    let original_generators = case.generators.len();
    let mut mappings = Vec::new();
    let mut from_generators = Vec::new();
    let mut to_generators = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        if line.len() < 17 {
            return Err(Error::InvalidCase(
                "dcline requires at least 17 columns".into(),
            ));
        }
        if line.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidCase("dcline values must be finite".into()));
        }
        let Some(from) = dc_bus(line[0]) else {
            return Err(Error::InvalidCase("invalid dcline from bus".into()));
        };
        let Some(to) = dc_bus(line[1]) else {
            return Err(Error::InvalidCase("invalid dcline to bus".into()));
        };
        let (Some(&from_bus), Some(&to_bus)) = (buses.get(&from), buses.get(&to)) else {
            return Err(Error::InvalidCase("dcline references unknown bus".into()));
        };
        if line[2] != 0.0 && line[2] != 1.0 {
            return Err(Error::InvalidCase("dcline status must be 0 or 1".into()));
        }
        if line[9] > line[10] || line[11] > line[12] || line[13] > line[14] {
            return Err(Error::InvalidCase("invalid dcline power limits".into()));
        }
        if line[7] <= 0.0 || line[8] <= 0.0 {
            return Err(Error::InvalidCase(
                "dcline voltage setpoints must be positive".into(),
            ));
        }
        if line[2] == 0.0 || skip.get(row).copied().unwrap_or(false) {
            continue;
        }
        if case.buses[from_bus].kind == BusType::Isolated
            || case.buses[to_bus].kind == BusType::Isolated
        {
            return Err(Error::InvalidCase(
                "active dcline references isolated bus".into(),
            ));
        }
        // A terminal with a zero-width reactive range (a VSC end in power
        // factor control) injects that fixed Q instead of holding voltage.
        let fixed_from = line[11] == line[12];
        let fixed_to = line[13] == line[14];
        if !fixed_from && case.buses[from_bus].kind != BusType::Reference {
            case.buses[from_bus].kind = BusType::Pv;
        }
        if !fixed_to && case.buses[to_bus].kind != BusType::Reference {
            case.buses[to_bus].kind = BusType::Pv;
        }
        let from_generator = original_generators + from_generators.len();
        from_generators.push(Generator {
            bus: from,
            pg: -line[3],
            qg: if fixed_from { line[11] } else { line[5] },
            qmax: line[12],
            qmin: line[11],
            vg: line[7],
            mbase: 100.0,
            online: true,
            pmax: f64::INFINITY,
            pmin: f64::NEG_INFINITY,
            extra: Vec::new(),
        });
        to_generators.push(Generator {
            bus: to,
            pg: line[3] - line[15] - line[16] * line[3],
            qg: if fixed_to { line[13] } else { line[6] },
            qmax: line[14],
            qmin: line[13],
            vg: line[8],
            mbase: 100.0,
            online: true,
            pmax: f64::INFINITY,
            pmin: f64::NEG_INFINITY,
            extra: Vec::new(),
        });
        mappings.push(DcLineGenerators {
            row,
            from_generator,
            to_generator: 0,
        });
    }
    let from_count = from_generators.len();
    for (index, mapping) in mappings.iter_mut().enumerate() {
        mapping.to_generator = original_generators + from_count + index;
    }
    case.generators.extend(from_generators);
    case.generators.extend(to_generators);
    Ok(mappings)
}

/// The operating point reported for a detailed line that is switched off.
fn blocked(line: &LccLine) -> LccSolution {
    let mut off = line.clone();
    off.control = dc::LccControl::Blocked;
    let taps = [line.rectifier.tap, line.inverter.tap];
    dc::operating_point(&off, 1.0, 1.0, [1.0, 1.0], taps).0
}

/// Validate and solve a case. Each active electrical island needs one reference bus.
/// Active MATPOWER DC lines are represented internally by a generator at each
/// terminal. Input data is never modified.
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] for invalid data or options,
/// [`Error::Unsupported`] for unsupported configurations, and
/// [`Error::Numerical`] for failed numerical operations. Nonconvergence and
/// reactive-limit infeasibility return an `Ok` result with a non-converged
/// [`SolveStatus`].
pub fn solve(input: &Case, opt: &PowerFlowOptions) -> Result<PowerFlowResult, Error> {
    let started = std::time::Instant::now();
    log::info!(
        "starting {:?} power-flow solve: {} buses, {} generators, {} branches, initialization={:?}, tolerance={:.6e}, max_iterations={}, enforce_q_limits={}",
        opt.model,
        input.buses.len(),
        input.generators.len(),
        input.branches.len(),
        opt.initialization,
        opt.tolerance,
        opt.max_iterations,
        opt.enforce_q_limits
    );
    let original_generators = input.generators.len();
    let mut working = input.clone();
    let rows = input.dcline.as_ref().map_or(0, Vec::len);
    let (detailed, adjust_taps, detailed_vsc): (Vec<&LccLine>, bool, Vec<&VscLine>) =
        match &opt.dc_model {
            DcModel::Detailed {
                lines,
                vsc_lines,
                adjust_taps,
            } if opt.model == Model::Ac => {
                let mut seen = vec![false; rows];
                let mut claim = |row: usize, name: &str| {
                    if row >= rows || std::mem::replace(&mut seen[row], true) {
                        Err(Error::InvalidCase(format!(
                            "detailed DC line {name} names a missing or repeated dcline row {row}"
                        )))
                    } else {
                        Ok(())
                    }
                };
                for line in lines {
                    line.validate().map_err(Error::InvalidCase)?;
                    claim(line.row, &line.name)?;
                }
                for line in vsc_lines {
                    line.validate().map_err(Error::InvalidCase)?;
                    claim(line.row, &line.name)?;
                }
                (
                    lines.iter().collect(),
                    *adjust_taps,
                    vsc_lines.iter().collect(),
                )
            }
            _ => (Vec::new(), false, Vec::new()),
        };
    // Detailed VSC rows keep their terminal generators, with active power
    // from the DC-side solution.
    let mut vsc_states = Vec::new();
    if let Some(table) = &mut working.dcline {
        for line in &detailed_vsc {
            let row = &mut table[line.row];
            if row[2] == 0.0 || !line.in_service {
                row[2] = 0.0;
                continue;
            }
            let state = line.dc_state(1.0).map_err(Error::InvalidCase)?;
            let (f, t) = (line.from, 1 - line.from);
            row[3] = -state.injection_mw[f];
            row[15] = row[3] - state.injection_mw[t];
            row[16] = 0.0;
            for (k, c) in [(11, f), (13, t)] {
                let conv = &line.converters[c];
                if conv.ac_control == VscAcControl::PowerFactor {
                    let q = conv.power_factor_q(state.injection_mw[c]);
                    (row[k], row[k + 1]) = (q, q);
                } else {
                    (row[k], row[k + 1]) = (conv.qmin, conv.qmax);
                }
            }
            vsc_states.push((*line, state));
        }
    }
    let mut skip = vec![false; rows];
    for line in &detailed {
        skip[line.row] = true;
    }
    let mappings = add_dc_line_generators_except(&mut working, &skip).inspect_err(|error| {
        log::error!("power-flow setup failed while converting DC lines: {error}");
    })?;
    // Rows switched off in the case stay off in the detailed model too.
    let active: Vec<_> = detailed
        .iter()
        .copied()
        .filter(|line| {
            input
                .dcline
                .as_ref()
                .is_some_and(|rows| rows[line.row][2] != 0.0)
        })
        .collect();
    let mut vsc_terminals: Vec<_> = vsc_states
        .into_iter()
        .filter_map(|(line, state)| {
            let m = mappings.iter().find(|m| m.row == line.row)?;
            Some(VscTerminal {
                line,
                gens: [m.from_generator, m.to_generator],
                scale: 1.0,
                state,
                limited: [false; 2],
            })
        })
        .collect();
    let mut result = solve_core(&working, opt, &active, adjust_taps, &mut vsc_terminals)
        .inspect_err(|error| {
            log::error!("{:?} power-flow solve failed: {error}", opt.model);
        })?;
    let solved_vsc = std::mem::take(&mut result.vsc);
    result.vsc = detailed_vsc
        .iter()
        .map(|line| {
            solved_vsc
                .iter()
                .find(|s| s.row == line.row)
                .cloned()
                .unwrap_or_else(|| idle_vsc(line))
        })
        .collect();
    let vms: std::collections::HashMap<_, _> =
        result.case.buses.iter().map(|b| (b.id, b.vm)).collect();
    let vm = |bus: u32| vms.get(&bus).copied().unwrap_or(1.0);
    if let Some(lines) = &mut result.case.dcline {
        let mut solutions = result.lcc.iter();
        let mut all = Vec::with_capacity(detailed.len());
        for line in &detailed {
            let row = &mut lines[line.row];
            let solution = if row[2] != 0.0 {
                solutions.next().cloned()
            } else {
                None
            };
            let solution = solution.unwrap_or_else(|| blocked(line));
            row[3] = solution.rectifier_mw;
            row[4] = solution.inverter_mw;
            row[5] = -solution.rectifier_mvar;
            row[6] = -solution.inverter_mvar;
            row[7] = vm(line.rectifier.bus);
            row[8] = vm(line.inverter.bus);
            all.push(solution);
        }
        result.lcc = all;
    }
    if let Some(lines) = &mut result.case.dcline {
        for mapping in mappings {
            let from = &result.case.generators[mapping.from_generator];
            let to = &result.case.generators[mapping.to_generator];
            let line = &mut lines[mapping.row];
            line[3] = -from.pg;
            line[4] = to.pg;
            line[5] = from.qg;
            line[6] = to.qg;
            line[7] = from.vg;
            line[8] = to.vg;
        }
    }
    result.case.generators.truncate(original_generators);
    log::info!(
        "finished {:?} power-flow solve: status={:?}, iterations={}, residual={:.6e}, q_limit_passes={}, elapsed={} ms",
        opt.model,
        result.status,
        result.iterations,
        result.residual,
        result.q_limit_passes,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytical_jacobian_matches_central_differences() {
        let mut case = crate::parse_matpower(include_str!("../tests/fixtures/case9.m")).unwrap();
        case.branches[1].tap = 1.07;
        case.branches[1].shift = 13.0;
        case.buses[4].gs = 3.0;
        case.buses[4].bs = 2.0;
        let net = Network::new(&case).unwrap();
        let v: Vec<_> = (0..net.external.len())
            .map(|i| C::from_polar(0.95 + 0.01 * i as f64, 0.025 * i as f64))
            .collect();
        let (angles, pq) = variables(&net.kinds);
        let n = angles.len() + pq.len();
        let mut exact = vec![vec![0.0; n]; n];
        for t in jacobian(&net, &v, &angles, &pq) {
            exact[t.row][t.col] += t.val;
        }
        let s = net.injections(&case);
        let h = 1e-6;
        for col in 0..n {
            let mut plus = v.clone();
            let mut minus = v.clone();
            if col < angles.len() {
                let i = angles[col];
                plus[i] *= C::from_polar(1.0, h);
                minus[i] *= C::from_polar(1.0, -h);
            } else {
                let i = pq[col - angles.len()];
                plus[i] = C::from_polar(v[i].norm() + h, v[i].arg());
                minus[i] = C::from_polar(v[i].norm() - h, v[i].arg());
            }
            let (fp, _) = mismatch(&net, &plus, &s, &angles, &pq).unwrap();
            let (fm, _) = mismatch(&net, &minus, &s, &angles, &pq).unwrap();
            for row in 0..n {
                assert!(
                    (exact[row][col] - (fp[row] - fm[row]) / (2.0 * h)).abs() < 1e-7,
                    "Jacobian ({row},{col})"
                );
            }
        }
    }

    #[test]
    fn singular_sparse_system_returns_error() {
        let entries = [
            Triplet::new(0, 0, 1.0),
            Triplet::new(0, 1, 1.0),
            Triplet::new(1, 0, 1.0),
            Triplet::new(1, 1, 1.0),
        ];
        assert!(linear(2, &entries, &[1.0, 2.0]).is_err());
    }
}
