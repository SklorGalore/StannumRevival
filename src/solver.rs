// Formulations and result allocation adapted from MATPOWER 8.1; see NOTICE.
use crate::{BusType, Case, Error, network::Network};
use faer::{
    Mat,
    prelude::Solve,
    sparse::{SparseColMat, Triplet},
};
use num_complex::Complex64 as C;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Ac,
    Dc,
}

#[derive(Clone, Debug)]
pub struct PowerFlowOptions {
    pub model: Model,
    /// Infinity norm of the constrained power mismatch, in per unit.
    pub tolerance: f64,
    /// Newton steps per reactive-limit pass. Zero evaluates the initial state.
    pub max_iterations: usize,
    pub enforce_q_limits: bool,
    /// Reactive-limit violation tolerance in MVAr.
    pub q_limit_tolerance: f64,
}
impl Default for PowerFlowOptions {
    fn default() -> Self {
        Self {
            model: Model::Ac,
            tolerance: 1e-8,
            max_iterations: 10,
            enforce_q_limits: false,
            q_limit_tolerance: 5e-6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveStatus {
    Converged,
    IterationLimit,
    ReactiveLimitsInfeasible,
}

#[derive(Clone, Debug)]
pub struct PowerFlowResult {
    /// Solved voltages and generator outputs, in original case order and units.
    pub case: Case,
    /// [PF, QF, PT, QT] in MW/MVAr, in original branch order.
    pub branch_flows: Vec<[f64; 4]>,
    pub status: SolveStatus,
    /// Total Newton steps across all reactive-limit passes (one for DC).
    pub iterations: usize,
    pub residual: f64,
    /// Number of passes that fixed one or more generators at reactive limits.
    pub q_limit_passes: usize,
}

fn linear(
    n: usize,
    entries: &[Triplet<usize, usize, f64>],
    rhs: &[f64],
) -> Result<Vec<f64>, Error> {
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

fn variables(kinds: &[BusType]) -> (Vec<usize>, Vec<usize>) {
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

fn jacobian(
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

fn newton(
    net: &Network,
    case: &Case,
    kinds: &[BusType],
    v: &mut [C],
    opt: &PowerFlowOptions,
) -> Result<(SolveStatus, usize, f64), Error> {
    let (angles, pq) = variables(kinds);
    let s = net.injections(case);
    for iteration in 0..=opt.max_iterations {
        let (f, residual) = mismatch(net, v, &s, &angles, &pq)?;
        if residual < opt.tolerance {
            return Ok((SolveStatus::Converged, iteration, residual));
        }
        if iteration == opt.max_iterations {
            return Ok((SolveStatus::IterationLimit, iteration, residual));
        }
        let j = jacobian(net, v, &angles, &pq);
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
            v[i] = C::from_polar(vm[i], va[i]);
            if !v[i].re.is_finite() || !v[i].im.is_finite() || v[i].norm() == 0.0 {
                return Err(Error::Numerical(
                    "Newton step produced an invalid voltage".into(),
                ));
            }
        }
    }
    unreachable!()
}

fn allocate(net: &Network, case: &mut Case, kinds: &[BusType], reference: usize, v: &[C]) {
    let power = net.power(v);
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
    let gens = &net.gens[reference];
    let other: f64 = gens.iter().skip(1).map(|&g| case.generators[g].pg).sum();
    case.generators[gens[0]].pg =
        power[reference].re * case.base_mva + case.buses[net.external[reference]].pd - other;
}

fn dc(net: &Network, mut case: Case, opt: &PowerFlowOptions) -> Result<PowerFlowResult, Error> {
    let n = net.external.len();
    let reference = net.reference;
    let vars: Vec<_> = (0..n).filter(|&i| i != reference).collect();
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
    let gens = &net.gens[reference];
    let other: f64 = gens.iter().skip(1).map(|&g| case.generators[g].pg).sum();
    let b = &case.buses[net.external[reference]];
    case.generators[gens[0]].pg = balance[reference] * case.base_mva + b.pd + b.gs - other;
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

/// Solve a validated single-network case. Input data is never modified.
pub fn solve(input: &Case, opt: &PowerFlowOptions) -> Result<PowerFlowResult, Error> {
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
    let mut kinds = net.kinds.clone();
    let mut reference = net.reference;
    let mut v: Vec<_> = net
        .external
        .iter()
        .map(|&e| {
            let b = &case.buses[e];
            C::from_polar(b.vm, b.va.to_radians())
        })
        .collect();
    for (i, gens) in net.gens.iter().enumerate() {
        if kinds[i] != BusType::Pq {
            for &g in gens {
                v[i] = C::from_polar(case.generators[g].vg, v[i].arg());
            }
        }
    }
    let (mut iterations, mut passes) = (0, 0);
    let (status, residual) = loop {
        let (status, it, residual) = newton(&net, &case, &kinds, &mut v, opt)?;
        iterations += it;
        allocate(&net, &mut case, &kinds, reference, &v);
        if status != SolveStatus::Converged || !opt.enforce_q_limits {
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
        if remaining.len() == violations.len()
            && remaining
                .iter()
                .all(|g| violations.iter().any(|x| x.1 == *g))
            && violations.iter().all(|x| x.3 == violations[0].3)
        {
            break (SolveStatus::ReactiveLimitsInfeasible, residual);
        }
        // Keep the last solved state intact if switching would remove every
        // possible reference bus, including mixed upper/lower violations.
        if kinds
            .iter()
            .enumerate()
            .all(|(i, kind)| *kind == BusType::Pq || violations.iter().any(|v| v.0 == i))
        {
            break (SolveStatus::ReactiveLimitsInfeasible, residual);
        }
        for &(i, g, limit, _) in &violations {
            case.generators[g].qg = limit;
            kinds[i] = BusType::Pq;
        }
        passes += 1;
        if kinds[reference] == BusType::Pq {
            if let Some(i) = kinds.iter().position(|k| *k == BusType::Pv) {
                reference = i;
                kinds[i] = BusType::Reference;
            } else {
                break (SolveStatus::ReactiveLimitsInfeasible, residual);
            }
        }
        if passes > case.generators.len() {
            return Err(Error::Numerical(
                "reactive-limit loop did not terminate".into(),
            ));
        }
    };
    let offset = if passes > 0 && reference != net.reference {
        input.buses[net.external[net.reference]].va - v[net.reference].arg().to_degrees()
    } else {
        0.0
    };
    for (i, &e) in net.external.iter().enumerate() {
        case.buses[e].vm = v[i].norm();
        case.buses[e].va = v[i].arg().to_degrees() + offset;
        case.buses[e].kind = kinds[i];
    }
    let mut flows = vec![[0.0; 4]; case.branches.len()];
    for e in &net.edges {
        let sf = v[e.f] * (e.yff * v[e.f] + e.yft * v[e.t]).conj() * case.base_mva;
        let st = v[e.t] * (e.ytf * v[e.f] + e.ytt * v[e.t]).conj() * case.base_mva;
        flows[e.original] = [sf.re, sf.im, st.re, st.im];
    }
    zero_offline(&net, &mut case);
    Ok(PowerFlowResult {
        case,
        branch_flows: flows,
        status,
        iterations,
        residual,
        q_limit_passes: passes,
    })
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
