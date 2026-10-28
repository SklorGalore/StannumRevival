//! A dc power flow on substations (paper eq. 6), solved with conjugate
//! gradients so each placement round can re-solve cheaply from the last
//! answer.

/// Angles θ (radians) with `B θ = p` on a weighted graph, holding `ground`
/// at zero. `edges` are (a, b, susceptance in per unit); `p` is the net
/// injection in per unit. `theta` is the starting guess and the answer.
/// Vertices with no path to `ground` keep angle zero.
pub fn solve(
    n: usize,
    edges: &[(usize, usize, f64)],
    p: &[f64],
    ground: usize,
    theta: &mut Vec<f64>,
) {
    theta.resize(n, 0.0);
    let mut diag = vec![0.0; n];
    for &(a, b, y) in edges {
        diag[a] += y;
        diag[b] += y;
    }
    // Only solve the component holding the ground vertex.
    let mut uf = crate::graph::UnionFind::new(n);
    for &(a, b, _) in edges {
        uf.union(a, b);
    }
    let root = uf.find(ground);
    let live: Vec<bool> = (0..n).map(|i| i != ground && uf.find(i) == root).collect();
    // The ground absorbs any imbalance.
    let mul = |x: &[f64], out: &mut [f64]| {
        out.iter_mut()
            .zip(x)
            .zip(&diag)
            .for_each(|((o, xi), d)| *o = d * xi);
        for &(a, b, y) in edges {
            out[a] -= y * x[b];
            out[b] -= y * x[a];
        }
    };
    let mask = |v: &mut [f64]| {
        for (i, x) in v.iter_mut().enumerate() {
            if !live[i] {
                *x = 0.0;
            }
        }
    };
    for (i, t) in theta.iter_mut().enumerate() {
        if !live[i] {
            *t = 0.0;
        }
    }
    let mut ax = vec![0.0; n];
    mul(theta, &mut ax);
    let mut r: Vec<f64> = (0..n)
        .map(|i| if live[i] { p[i] - ax[i] } else { 0.0 })
        .collect();
    let inv: Vec<f64> = diag
        .iter()
        .map(|&d| if d > 0.0 { 1.0 / d } else { 0.0 })
        .collect();
    let mut z: Vec<f64> = r.iter().zip(&inv).map(|(a, b)| a * b).collect();
    let mut d = z.clone();
    let mut rz: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
    let norm_p: f64 = p
        .iter()
        .enumerate()
        .filter(|(i, _)| live[*i])
        .map(|(_, x)| x * x)
        .sum::<f64>()
        .sqrt();
    // Flows only steer line choice, so a loose tolerance is plenty.
    let tol = 1e-4 * norm_p.max(1e-9);
    let mut ad = vec![0.0; n];
    for _ in 0..n.clamp(100, 400) {
        let rn: f64 = r.iter().map(|x| x * x).sum::<f64>().sqrt();
        if rn <= tol {
            break;
        }
        mul(&d, &mut ad);
        mask(&mut ad);
        let dad: f64 = d.iter().zip(&ad).map(|(a, b)| a * b).sum();
        if dad <= 0.0 {
            break;
        }
        let alpha = rz / dad;
        for i in 0..n {
            theta[i] += alpha * d[i];
            r[i] -= alpha * ad[i];
        }
        z.iter_mut()
            .zip(&r)
            .zip(&inv)
            .for_each(|((zi, ri), vi)| *zi = ri * vi);
        let rz_next: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
        let beta = rz_next / rz;
        rz = rz_next;
        for i in 0..n {
            d[i] = z[i] + beta * d[i];
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn three_bus_matches_hand_solution() {
        // 0 is ground; 1 MW-pu into bus 2 flows back through two paths.
        let edges = [(0, 1, 10.0), (1, 2, 10.0), (0, 2, 5.0)];
        let p = [0.0, 0.0, -1.0];
        let mut th = vec![];
        super::solve(3, &edges, &p, 0, &mut th);
        // Path 0-1-2 has y = 5, direct y = 5: half each, θ2 = -0.1.
        assert!((th[2] + 0.1).abs() < 1e-6, "{th:?}");
        assert!((th[1] + 0.05).abs() < 1e-6);
    }
}
