//! Linear (DC) outage screening: how branch flows move when branches trip or
//! injections change, from one factorization of the DC susceptance matrix.
//!
//! [`make_lodf`](crate::make_lodf) builds the whole branch-by-branch matrix,
//! which grows with the square of the case size. [`DcFactors`] factorizes
//! once and answers one outage at a time with a sparse solve per tripped
//! branch, so screening every outage of an 80,000-bus case stays practical.

use crate::{Case, Error, network::Network};
use faer::{
    Mat,
    prelude::Solve,
    sparse::{SparseColMat, Triplet, linalg::solvers::Lu},
};

/// Reactance used for branches with zero reactance (jumpers and bus ties),
/// so they carry flow in the linear model instead of failing it.
const ZERO_X: f64 = 1e-4;

/// Smallest pivot accepted when tripping several branches together; below it
/// the outage splits the network and has no linear answer.
const ISLANDING_PIVOT: f64 = 1e-6;

/// One factorization of a case's DC network, for outage screening.
pub struct DcFactors {
    /// Case bus row of each active (internal) bus.
    external: Vec<usize>,
    /// Internal bus of each case bus row, `None` for isolated buses.
    internal: Vec<Option<usize>>,
    /// Position of each internal bus among the solved angles; `None` for
    /// island reference buses, whose angle is fixed.
    position: Vec<Option<usize>>,
    /// Island of each internal bus, and the reference bus of each island.
    island: Vec<usize>,
    references: Vec<usize>,
    /// Internal ends and susceptance of each case branch row in service.
    branches: Vec<Option<(usize, usize, f64)>>,
    /// Neighbours of each internal bus, with the branch row joining them.
    adjacency: Vec<Vec<(usize, usize)>>,
    lu: Option<Lu<usize, f64>>,
    size: usize,
    bus_by_id: std::collections::HashMap<u32, usize>,
}

/// Branch flow changes caused by tripping branches.
#[derive(Clone, Debug, PartialEq)]
pub enum OutageFlows {
    /// Flow on every branch row after the outage, in the units of the flows
    /// given (the tripped rows are zero).
    Flows(Vec<f64>),
    /// The outage splits the network: these case bus rows lose their path to
    /// a reference bus.
    Islands(Vec<usize>),
}

impl DcFactors {
    /// Factorize the DC susceptance matrix of `case` as it is (branch and
    /// generator statuses included).
    ///
    /// # Errors
    ///
    /// The same topology errors as a power flow, or [`Error::Numerical`] if
    /// the matrix can't be factorized.
    pub fn new(case: &Case) -> Result<Self, Error> {
        let net = Network::new(case)?;
        let n = net.external.len();
        let mut internal = vec![None; case.buses.len()];
        for (bus, &row) in net.external.iter().enumerate() {
            internal[row] = Some(bus);
        }
        let mut island = vec![0; n];
        for (index, members) in net.islands.iter().enumerate() {
            for &bus in members {
                island[bus] = index;
            }
        }
        let mut position = vec![None; n];
        let mut size = 0;
        for (bus, slot) in position.iter_mut().enumerate() {
            if !net.references.contains(&bus) {
                *slot = Some(size);
                size += 1;
            }
        }
        let mut branches = vec![None; case.branches.len()];
        let mut adjacency = vec![Vec::new(); n];
        let mut entries = Vec::new();
        for edge in &net.edges {
            let branch = &case.branches[edge.original];
            let x = if branch.x == 0.0 { ZERO_X } else { branch.x };
            let b = 1.0 / x / if branch.tap == 0.0 { 1.0 } else { branch.tap };
            branches[edge.original] = Some((edge.f, edge.t, b));
            adjacency[edge.f].push((edge.t, edge.original));
            adjacency[edge.t].push((edge.f, edge.original));
            for (r, c, v) in [
                (edge.f, edge.f, b),
                (edge.f, edge.t, -b),
                (edge.t, edge.f, -b),
                (edge.t, edge.t, b),
            ] {
                if let (Some(r), Some(c)) = (position[r], position[c]) {
                    entries.push(Triplet::new(r, c, v));
                }
            }
        }
        let lu = if size == 0 {
            None
        } else {
            let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(size, size, &entries)
                .map_err(|e| Error::Numerical(format!("sparse assembly: {e:?}")))?;
            Some(
                matrix
                    .sp_lu()
                    .map_err(|e| Error::Numerical(format!("singular DC network: {e:?}")))?,
            )
        };
        log::debug!(
            "factorized DC network for outage screening: {n} buses, {} branches",
            net.edges.len()
        );
        Ok(Self {
            external: net.external,
            internal,
            position,
            island,
            references: net.references,
            branches,
            adjacency,
            lu,
            size,
            bus_by_id: case
                .buses
                .iter()
                .enumerate()
                .map(|(row, b)| (b.id, row))
                .collect(),
        })
    }

    /// Branch rows the factorization treats as in service.
    pub fn in_service(&self, row: usize) -> bool {
        self.branches.get(row).is_some_and(Option::is_some)
    }

    /// Bus angles (internal order) for each column of injections, given as
    /// (internal bus, per-unit amount) pairs and balanced at each island's
    /// reference bus.
    fn angles(&self, columns: &[Vec<(usize, f64)>]) -> Vec<Vec<f64>> {
        let n = self.external.len();
        let Some(lu) = &self.lu else {
            return vec![vec![0.0; n]; columns.len()];
        };
        let rhs = Mat::from_fn(self.size, columns.len(), |_, _| 0.0);
        let mut rhs = rhs;
        for (c, column) in columns.iter().enumerate() {
            for &(bus, amount) in column {
                if let Some(p) = self.position[bus] {
                    rhs[(p, c)] += amount;
                }
            }
        }
        let solved = lu.solve(rhs);
        columns
            .iter()
            .enumerate()
            .map(|(c, _)| {
                (0..n)
                    .map(|bus| self.position[bus].map_or(0.0, |p| solved[(p, c)]))
                    .collect()
            })
            .collect()
    }

    fn flows_from_angles(&self, theta: &[f64]) -> Vec<f64> {
        self.branches
            .iter()
            .map(|b| b.map_or(0.0, |(f, t, b)| b * (theta[f] - theta[t])))
            .collect()
    }

    /// Change in flow on every branch row (from end, positive into the
    /// branch) when the given injections change, each as (bus id, amount).
    /// Each island's reference bus takes up the difference. Amounts may be
    /// in MW or per unit; the result is in the same units.
    pub fn injection_flows(&self, injections: &[(u32, f64)]) -> Vec<f64> {
        let column: Vec<(usize, f64)> = injections
            .iter()
            .filter_map(|&(id, amount)| {
                let row = *self.bus_by_id.get(&id)?;
                Some((self.internal[row]?, amount))
            })
            .collect();
        let theta = self.angles(&[column]).pop().unwrap_or_default();
        self.flows_from_angles(&theta)
    }

    /// How much each branch in `rows` changes flow (from end, positive into
    /// the branch) per unit injected at each case bus row and taken out at
    /// that island's reference bus: one row of the PTDF matrix per branch,
    /// from one solve each (the susceptance matrix is symmetric). Branches
    /// out of service, and buses in other islands, give zeros.
    pub fn branch_sensitivities(&self, rows: &[usize]) -> Vec<Vec<f64>> {
        let columns: Vec<Vec<(usize, f64)>> = rows
            .iter()
            .map(|&row| match self.branches.get(row).copied().flatten() {
                Some((f, t, b)) => vec![(f, b), (t, -b)],
                None => Vec::new(),
            })
            .collect();
        self.angles(&columns)
            .into_iter()
            .map(|x| {
                (0..self.internal.len())
                    .map(|row| self.internal[row].map_or(0.0, |bus| x[bus]))
                    .collect()
            })
            .collect()
    }

    /// Flows after tripping branch `rows` together, given every branch's
    /// flow before (from end, positive into the branch). Rows already out of
    /// service are ignored.
    pub fn trip(&self, rows: &[usize], flows: &[f64]) -> OutageFlows {
        match self.trip_each(rows, &[flows]) {
            Ok(mut after) => OutageFlows::Flows(after.pop().unwrap_or_default()),
            Err(cut) => OutageFlows::Islands(cut),
        }
    }

    /// [`trip`](Self::trip) for several sets of flows at once, with one
    /// solve. Outage factors are linear, so a set can also be a transfer's
    /// distribution factors: tripping them gives the factors after the
    /// outage (OTDFs). On islanding, the case bus rows cut off.
    ///
    /// # Errors
    ///
    /// The bus rows cut off when the outage splits the network.
    pub fn trip_each(&self, rows: &[usize], sets: &[&[f64]]) -> Result<Vec<Vec<f64>>, Vec<usize>> {
        let tripped: Vec<(usize, usize, usize)> = rows
            .iter()
            .filter_map(|&row| {
                let (f, t, _) = (*self.branches.get(row)?)?;
                Some((row, f, t))
            })
            .collect();
        let mut out: Vec<Vec<f64>> = sets.iter().map(|flows| flows.to_vec()).collect();
        if tripped.is_empty() {
            return Ok(out);
        }
        // Flow on every branch from a unit transfer across each tripped one.
        let columns: Vec<Vec<(usize, f64)>> = tripped
            .iter()
            .map(|&(_, f, t)| vec![(f, 1.0), (t, -1.0)])
            .collect();
        let transfers: Vec<Vec<f64>> = self
            .angles(&columns)
            .iter()
            .map(|theta| self.flows_from_angles(theta))
            .collect();
        // Solve (I - T) x = F over the tripped branches, where T[i][j] is the
        // flow on tripped branch i from a transfer across tripped branch j.
        let k = tripped.len();
        let matrix: Vec<Vec<f64>> = (0..k)
            .map(|i| {
                (0..k)
                    .map(|j| f64::from(i == j) - transfers[j][tripped[i].0])
                    .collect()
            })
            .collect();
        for (after, flows) in out.iter_mut().zip(sets) {
            let mut m = matrix.clone();
            let mut x: Vec<f64> = tripped.iter().map(|&(row, _, _)| flows[row]).collect();
            if !solve_dense(&mut m, &mut x) {
                return Err(self.cut_off(rows));
            }
            for (j, transfer) in transfers.iter().enumerate() {
                for (value, &delta) in after.iter_mut().zip(transfer) {
                    *value += delta * x[j];
                }
            }
            for &(row, _, _) in &tripped {
                after[row] = 0.0;
            }
        }
        Ok(out)
    }

    /// Case bus rows split off when branch `rows` trip: in each island,
    /// every bus outside the largest part that remains. That part keeps
    /// running even when it lost its reference bus (as when a reference
    /// generator's step-up transformer trips), as another bus would take up
    /// the slack.
    pub fn cut_off(&self, rows: &[usize]) -> Vec<usize> {
        let n = self.external.len();
        let mut part = vec![usize::MAX; n];
        let mut sizes: Vec<(usize, usize)> = Vec::new(); // (island, size)
        for start in 0..n {
            if part[start] != usize::MAX {
                continue;
            }
            let id = sizes.len();
            let mut size = 0;
            let mut stack = vec![start];
            part[start] = id;
            while let Some(bus) = stack.pop() {
                size += 1;
                for &(next, row) in &self.adjacency[bus] {
                    if part[next] == usize::MAX && !rows.contains(&row) {
                        part[next] = id;
                        stack.push(next);
                    }
                }
            }
            sizes.push((self.island[start], size));
        }
        let mut keep = vec![usize::MAX; self.references.len()];
        for (id, &(island, size)) in sizes.iter().enumerate() {
            if keep[island] == usize::MAX || size > sizes[keep[island]].1 {
                keep[island] = id;
            }
        }
        (0..n)
            .filter(|&bus| keep[self.island[bus]] != part[bus])
            .map(|bus| self.external[bus])
            .collect()
    }

    /// The case bus row of the reference bus in the island of case bus row
    /// `bus`, if it's active.
    pub fn reference_of(&self, bus: usize) -> Option<usize> {
        let internal = self.internal.get(bus).copied().flatten()?;
        Some(self.external[self.references[self.island[internal]]])
    }
}

/// Solve the small dense system `m x = b` in place with partial pivoting.
/// Returns false when a pivot is too small (the outage splits the network).
fn solve_dense(m: &mut [Vec<f64>], b: &mut [f64]) -> bool {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))
            .unwrap_or(col);
        if m[pivot][col].abs() < ISLANDING_PIVOT {
            return false;
        }
        m.swap(col, pivot);
        b.swap(col, pivot);
        let (top, rest) = m.split_at_mut(col + 1);
        let pivot_row = &top[col];
        for (offset, row) in rest.iter_mut().enumerate() {
            let factor = row[col] / pivot_row[col];
            if factor != 0.0 {
                for (value, &p) in row[col..].iter_mut().zip(&pivot_row[col..]) {
                    *value -= factor * p;
                }
                b[col + 1 + offset] -= factor * b[col];
            }
        }
    }
    for col in (0..n).rev() {
        let value = b[col]
            - m[col][col + 1..]
                .iter()
                .zip(&b[col + 1..])
                .map(|(a, x)| a * x)
                .sum::<f64>();
        b[col] = value / m[col][col];
    }
    b.iter().all(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LodfBridgeHandling, make_lodf_with_bridge_handling, make_ptdf, parse_matpower};

    fn case9() -> Case {
        parse_matpower(include_str!("../tests/fixtures/case9.m")).unwrap()
    }

    #[test]
    fn single_trip_matches_the_lodf_matrix() {
        let case = case9();
        let ptdf = make_ptdf(&case, None).unwrap();
        let lodf = make_lodf_with_bridge_handling(&case, &ptdf, LodfBridgeHandling::Mask).unwrap();
        let factors = DcFactors::new(&case).unwrap();
        let flows: Vec<f64> = (0..case.branches.len()).map(|i| 10.0 + i as f64).collect();
        for outage in 0..case.branches.len() {
            match factors.trip(&[outage], &flows) {
                OutageFlows::Flows(after) => {
                    for m in 0..case.branches.len() {
                        let expected = if m == outage {
                            0.0
                        } else {
                            flows[m] + lodf[m][outage] * flows[outage]
                        };
                        assert!(
                            (after[m] - expected).abs() < 1e-9,
                            "outage {outage}, branch {m}: {} vs {expected}",
                            after[m]
                        );
                    }
                }
                OutageFlows::Islands(buses) => {
                    assert!(
                        lodf[0][outage].is_nan(),
                        "outage {outage} cut off {buses:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn branch_sensitivities_are_ptdf_rows() {
        let case = case9();
        let ptdf = make_ptdf(&case, None).unwrap();
        let factors = DcFactors::new(&case).unwrap();
        let rows: Vec<usize> = (0..case.branches.len()).collect();
        let sens = factors.branch_sensitivities(&rows);
        for (row, s) in sens.iter().enumerate() {
            for bus in 0..case.buses.len() {
                assert!(
                    (s[bus] - ptdf[row][bus]).abs() < 1e-9,
                    "branch {row}, bus {bus}: {} vs {}",
                    s[bus],
                    ptdf[row][bus]
                );
            }
        }
    }

    #[test]
    fn bridge_trips_report_the_buses_cut_off() {
        // case9's generator step-up transformers are bridges. Tripping the
        // reference generator's leaves the rest running.
        let case = case9();
        let factors = DcFactors::new(&case).unwrap();
        let flows = vec![1.0; case.branches.len()];
        let row = case
            .branches
            .iter()
            .position(|b| (b.from, b.to) == (1, 4))
            .unwrap();
        assert_eq!(factors.trip(&[row], &flows), OutageFlows::Islands(vec![0]));
    }

    #[test]
    fn double_trip_matches_two_single_trips_in_sequence() {
        let case = parse_matpower(include_str!("../tests/fixtures/case30.m")).unwrap();
        let factors = DcFactors::new(&case).unwrap();
        let flows: Vec<f64> = (0..case.branches.len())
            .map(|i| 5.0 * i as f64 - 12.0)
            .collect();
        // Two meshed lines far apart: 1-2 and 27-30.
        let find = |f, t| {
            case.branches
                .iter()
                .position(|b| (b.from, b.to) == (f, t))
                .unwrap()
        };
        let (a, b) = (find(1, 2), find(27, 30));
        let OutageFlows::Flows(together) = factors.trip(&[a, b], &flows) else {
            panic!("double trip islanded");
        };
        let OutageFlows::Flows(first) = factors.trip(&[a], &flows) else {
            panic!("first trip islanded");
        };
        let mut without_a = case.clone();
        without_a.branches[a].online = false;
        let OutageFlows::Flows(second) = DcFactors::new(&without_a).unwrap().trip(&[b], &first)
        else {
            panic!("second trip islanded");
        };
        for (x, y) in together.iter().zip(&second) {
            assert!((x - y).abs() < 1e-9, "{x} vs {y}");
        }
    }

    #[test]
    fn tripping_transfer_factors_gives_otdfs() {
        // OTDF(m, k) = PTDF(m) + LODF(m, k) * PTDF(k) for a 2-to-3 transfer.
        let case = case9();
        let ptdf = make_ptdf(&case, None).unwrap();
        let lodf = make_lodf_with_bridge_handling(&case, &ptdf, LodfBridgeHandling::Mask).unwrap();
        let factors = DcFactors::new(&case).unwrap();
        let transfer = factors.injection_flows(&[(2, 1.0), (3, -1.0)]);
        let flows = vec![7.0; case.branches.len()];
        let line = case
            .branches
            .iter()
            .position(|b| (b.from, b.to) == (5, 6))
            .unwrap();
        let [after, otdf] =
            <[Vec<f64>; 2]>::try_from(factors.trip_each(&[line], &[&flows, &transfer]).unwrap())
                .unwrap();
        for m in 0..case.branches.len() {
            if m == line {
                assert_eq!((after[m], otdf[m]), (0.0, 0.0));
                continue;
            }
            let expected = transfer[m] + lodf[m][line] * transfer[line];
            assert!(
                (otdf[m] - expected).abs() < 1e-9,
                "{m}: {} vs {expected}",
                otdf[m]
            );
            assert!((after[m] - (7.0 + lodf[m][line] * 7.0)).abs() < 1e-9);
        }
    }

    #[test]
    fn injection_change_follows_the_ptdf() {
        let case = case9();
        let ptdf = make_ptdf(&case, None).unwrap();
        let factors = DcFactors::new(&case).unwrap();
        let delta = factors.injection_flows(&[(3, -85.0)]);
        for (m, value) in delta.iter().enumerate() {
            assert!((value - ptdf[m][2] * -85.0).abs() < 1e-9);
        }
    }
}
