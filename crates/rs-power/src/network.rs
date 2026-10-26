use crate::{BusType, Case, Error};
use num_complex::Complex64 as C;
use std::collections::{BTreeMap, HashMap};

pub(crate) struct Edge {
    pub original: usize,
    pub f: usize,
    pub t: usize,
    pub yff: C,
    pub yft: C,
    pub ytf: C,
    pub ytt: C,
}

pub(crate) struct Network {
    pub external: Vec<usize>,
    pub by_id: HashMap<u32, usize>,
    pub y: Vec<BTreeMap<usize, C>>,
    pub edges: Vec<Edge>,
    pub gens: Vec<Vec<usize>>,
    pub kinds: Vec<BusType>,
    /// Constant-current load per bus in per unit at 1 p.u. voltage; the
    /// load drawn is this times |V|. Constant-impedance load is in `y`.
    pub current_loads: Vec<C>,
    pub islands: Vec<Vec<usize>>,
    pub references: Vec<usize>,
}

impl Network {
    pub fn new(case: &Case) -> Result<Self, Error> {
        log::debug!(
            "assembling active network from {} buses, {} generators, and {} branches",
            case.buses.len(),
            case.generators.len(),
            case.branches.len()
        );
        let invalid = |msg: &str| Error::InvalidCase(msg.into());
        if !case.base_mva.is_finite() || case.base_mva <= 0.0 {
            return Err(invalid("baseMVA must be finite and positive"));
        }
        let mut all = HashMap::new();
        let mut external = Vec::new();
        let mut by_id = HashMap::new();
        for (i, b) in case.buses.iter().enumerate() {
            if b.id == 0 || all.insert(b.id, i).is_some() {
                return Err(invalid("bus IDs must be positive and unique"));
            }
            if [
                b.pd, b.qd, b.pd_i, b.qd_i, b.pd_z, b.qd_z, b.gs, b.bs, b.vm, b.va, b.base_kv,
                b.vmax, b.vmin,
            ]
            .iter()
            .any(|x| !x.is_finite())
                || b.vm <= 0.0
            {
                return Err(invalid(
                    "bus values must be finite and voltage magnitudes positive",
                ));
            }
            if b.kind != BusType::Isolated {
                by_id.insert(b.id, external.len());
                external.push(i);
            }
        }
        let n = external.len();
        if n == 0 {
            return Err(invalid("no active buses"));
        }
        let mut gens = vec![Vec::new(); n];
        for (i, g) in case.generators.iter().enumerate() {
            if !all.contains_key(&g.bus) {
                return Err(invalid("generator references unknown bus"));
            }
            if [g.pg, g.qg, g.vg, g.mbase].iter().any(|x| !x.is_finite())
                || g.vg <= 0.0
                || g.qmin.is_nan()
                || g.qmax.is_nan()
                || g.qmin == f64::INFINITY
                || g.qmax == f64::NEG_INFINITY
                || g.qmin > g.qmax
            {
                return Err(invalid(
                    "invalid generator power, voltage or reactive limits",
                ));
            }
            if g.online
                && let Some(&k) = by_id.get(&g.bus)
            {
                gens[k].push(i);
            }
        }
        let kinds: Vec<_> = external
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                if gens[i].is_empty() {
                    BusType::Pq
                } else {
                    case.buses[e].kind
                }
            })
            .collect();
        let mut y = vec![BTreeMap::new(); n];
        let mut edges = Vec::new();
        let mut adj = vec![Vec::new(); n];
        for (i, br) in case.branches.iter().enumerate() {
            if !all.contains_key(&br.from) || !all.contains_key(&br.to) {
                return Err(invalid("branch references unknown bus"));
            }
            if [br.r, br.x, br.b, br.tap, br.shift]
                .iter()
                .any(|x| !x.is_finite())
                || br.tap < 0.0
            {
                return Err(invalid("invalid branch impedance, charging or tap"));
            }
            if !br.online {
                continue;
            }
            let (Some(&f), Some(&t)) = (by_id.get(&br.from), by_id.get(&br.to)) else {
                continue;
            };
            if br.r == 0.0 && br.x == 0.0 {
                return Err(invalid("active branch has zero impedance"));
            }
            let ys = C::new(1.0, 0.0) / C::new(br.r, br.x);
            let tap = C::from_polar(
                if br.tap == 0.0 { 1.0 } else { br.tap },
                br.shift.to_radians(),
            );
            let ytt = ys + C::new(0.0, br.b / 2.0);
            let edge = Edge {
                original: i,
                f,
                t,
                yff: ytt / tap.norm_sqr(),
                yft: -ys / tap.conj(),
                ytf: -ys / tap,
                ytt,
            };
            for (r, c, v) in [
                (f, f, edge.yff),
                (f, t, edge.yft),
                (t, f, edge.ytf),
                (t, t, edge.ytt),
            ] {
                *y[r].entry(c).or_default() += v;
            }
            adj[f].push(t);
            adj[t].push(f);
            edges.push(edge);
        }
        let mut current_loads = Vec::with_capacity(n);
        for (i, &e) in external.iter().enumerate() {
            let b = &case.buses[e];
            // A constant-impedance load drawing pd_z + j·qd_z at 1 p.u. is a
            // shunt admittance of pd_z − j·qd_z.
            *y[i].entry(i).or_default() += C::new(b.gs + b.pd_z, b.bs - b.qd_z) / case.base_mva;
            current_loads.push(C::new(b.pd_i, b.qd_i) / case.base_mva);
        }
        let mut seen = vec![false; n];
        let mut islands = Vec::new();
        for start in 0..n {
            if seen[start] {
                continue;
            }
            let mut island = Vec::new();
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(i) = stack.pop() {
                island.push(i);
                for &j in &adj[i] {
                    if !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            island.sort_unstable();
            islands.push(island);
        }
        let mut references = Vec::with_capacity(islands.len());
        for island in &islands {
            let refs: Vec<_> = island
                .iter()
                .copied()
                .filter(|&i| case.buses[external[i]].kind == BusType::Reference)
                .collect();
            if refs.len() != 1 {
                return Err(Error::Unsupported(
                    "each active island requires exactly one reference bus".into(),
                ));
            }
            if gens[refs[0]].is_empty() {
                return Err(invalid("reference bus needs an online generator"));
            }
            references.push(refs[0]);
        }
        log::debug!(
            "assembled active network: {} buses, {} branches, {} islands, {} references",
            external.len(),
            edges.len(),
            islands.len(),
            references.len()
        );
        Ok(Self {
            external,
            by_id,
            y,
            edges,
            gens,
            kinds,
            current_loads,
            islands,
            references,
        })
    }

    pub fn injections(&self, case: &Case) -> Vec<C> {
        self.external
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                let b = &case.buses[e];
                (self.gens[i]
                    .iter()
                    .map(|&g| C::new(case.generators[g].pg, case.generators[g].qg))
                    .sum::<C>()
                    - C::new(b.pd, b.qd))
                    / case.base_mva
            })
            .collect()
    }

    /// Power each bus sends into the network and its voltage-dependent
    /// load: branch flows, shunts and constant-impedance load through `y`,
    /// plus constant-current load. Balances [`Self::injections`] at a solution.
    pub fn power(&self, v: &[C]) -> Vec<C> {
        self.y
            .iter()
            .enumerate()
            .map(|(i, row)| {
                v[i] * row.iter().map(|(&j, &y)| y * v[j]).sum::<C>().conj()
                    + self.current_loads[i] * v[i].norm()
            })
            .collect()
    }
}
