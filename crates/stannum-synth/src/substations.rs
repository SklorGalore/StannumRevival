//! Section II of the paper: place load from population, place power
//! plants, and group both into substations of three kinds: load only,
//! generation only, and both.

use crate::cluster::{Item, agglomerate};
use crate::config::{Config, Fuel};
use crate::geo::{LatLon, Projection, dist};
use crate::rng::{Rng, Weighted};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct PopulationPoint {
    pub at: LatLon,
    #[serde(skip)]
    pub xy: [f64; 2],
    pub people: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Plant {
    pub at: LatLon,
    #[serde(skip)]
    pub xy: [f64; 2],
    pub fuel: Fuel,
    pub mw: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Load,
    Generation,
    Both,
}

#[derive(Clone, Debug, Serialize)]
pub struct Substation {
    pub at: LatLon,
    #[serde(skip)]
    pub xy: [f64; 2],
    pub kind: Kind,
    pub load_mw: f64,
    pub people: f64,
    /// Indices into [`Placement::plants`].
    pub plants: Vec<usize>,
    pub area: u32,
    /// Indices into the config's levels, lowest first.
    pub levels: Vec<usize>,
}

impl Substation {
    pub fn capacity(&self, plants: &[Plant]) -> f64 {
        self.plants.iter().map(|&p| plants[p].mw).sum()
    }
}

pub struct Placement {
    pub projection: Projection,
    pub population: Vec<PopulationPoint>,
    pub plants: Vec<Plant>,
    pub substations: Vec<Substation>,
}

fn clamp_to(cfg: &Config, p: LatLon) -> LatLon {
    let f = &cfg.footprint;
    LatLon {
        lat: p.lat.clamp(f.lat_min, f.lat_max),
        lon: p.lon.clamp(f.lon_min, f.lon_max),
    }
}

fn uniform(cfg: &Config, rng: &mut Rng) -> LatLon {
    let f = &cfg.footprint;
    LatLon {
        lat: rng.range(f.lat_min, f.lat_max),
        lon: rng.range(f.lon_min, f.lon_max),
    }
}

/// A point `sd` miles (per axis) around `at`.
fn around(cfg: &Config, proj: &Projection, at: LatLon, sd: f64, rng: &mut Rng) -> LatLon {
    let xy = proj.xy(at);
    let moved = proj.latlon([xy[0] + rng.normal() * sd, xy[1] + rng.normal() * sd]);
    clamp_to(cfg, moved)
}

/// Synthetic census: Zipf-sized cities plus scattered rural points.
pub fn population(
    cfg: &Config,
    proj: &Projection,
    load_subs: usize,
    rng: &mut Rng,
) -> Vec<PopulationPoint> {
    let p = &cfg.population;
    let n_points = ((p.points_per_substation * load_subs as f64).ceil() as usize).max(load_subs);
    let n_cities = ((p.cities_per_substation * load_subs as f64).round() as usize).max(3);
    let f = &cfg.footprint;
    let inset = |lo: f64, hi: f64| (lo + 0.04 * (hi - lo), hi - 0.04 * (hi - lo));
    let (la, lb) = inset(f.lat_min, f.lat_max);
    let (oa, ob) = inset(f.lon_min, f.lon_max);
    let cities: Vec<(LatLon, f64)> = (0..n_cities)
        .map(|rank| {
            let at = LatLon {
                lat: rng.range(la, lb),
                lon: rng.range(oa, ob),
            };
            (at, 1.0 / (rank as f64 + 1.0))
        })
        .collect();
    let biggest = cities[0].1;
    let pick = Weighted::new(&cities.iter().map(|c| c.1).collect::<Vec<_>>());
    let urban = (p.urban_share * n_points as f64).round() as usize;
    let mut out = Vec::with_capacity(n_points);
    for i in 0..n_points {
        let (at, people) = if i < urban {
            let (centre, w) = cities[pick.sample(rng)];
            // Bigger cities sprawl further.
            let sd = 4.0 + 22.0 * (w / biggest).sqrt();
            (
                around(cfg, proj, centre, sd, rng),
                rng.lognormal(12_000.0, 0.5),
            )
        } else {
            (uniform(cfg, rng), rng.lognormal(2_500.0, 0.7))
        };
        out.push(PopulationPoint {
            at,
            xy: proj.xy(at),
            people,
        });
    }
    out
}

/// Power plants by fuel, sized log-normally and scaled so each fuel has its
/// share of `capacity_mw`.
pub fn plants(
    cfg: &Config,
    proj: &Projection,
    capacity_mw: f64,
    count: usize,
    population: &[PopulationPoint],
    rng: &mut Rng,
) -> Vec<Plant> {
    let mixes = &cfg.generation.fuels;
    let total_share: f64 = mixes.iter().map(|m| m.capacity_share).sum();
    let natural: Vec<f64> = mixes
        .iter()
        .map(|m| {
            let mean = m.median_mw * (m.sigma * m.sigma / 2.0).exp();
            capacity_mw * m.capacity_share / total_share / mean
        })
        .collect();
    let scale = count as f64 / natural.iter().sum::<f64>().max(1e-9);
    let people = Weighted::new(&population.iter().map(|p| p.people).collect::<Vec<_>>());
    let f = &cfg.footprint;
    let west = f.lon_min + 0.55 * (f.lon_max - f.lon_min);
    let mut out = Vec::new();
    for (m, n) in mixes.iter().zip(natural) {
        let n = ((n * scale).round() as usize).max(1);
        let target = capacity_mw * m.capacity_share / total_share;
        let start = out.len();
        for _ in 0..n {
            let at = match m.fuel {
                Fuel::Coal | Fuel::Gas | Fuel::Nuclear => {
                    let near = population[people.sample(rng)].at;
                    let sd = match m.fuel {
                        Fuel::Gas => 12.0,
                        Fuel::Coal => 30.0,
                        _ => 45.0,
                    };
                    around(cfg, proj, near, sd, rng)
                }
                Fuel::Wind => {
                    // Wind leans to the open west of the footprint.
                    let mut p = uniform(cfg, rng);
                    if rng.f64() < 0.7 {
                        p.lon = rng.range(f.lon_min, west);
                    }
                    p
                }
                Fuel::Hydro | Fuel::Solar => uniform(cfg, rng),
            };
            out.push(Plant {
                at,
                xy: proj.xy(at),
                fuel: m.fuel,
                mw: rng.lognormal(m.median_mw, m.sigma).max(5.0),
            });
        }
        let made: f64 = out[start..].iter().map(|p| p.mw).sum();
        for p in &mut out[start..] {
            p.mw = (p.mw * target / made * 10.0).round() / 10.0;
        }
    }
    out
}

/// Split points into `k` compact regions of about equal count by repeated
/// median cuts along the wider side.
pub fn regions(xy: &[[f64; 2]], k: usize) -> Vec<u32> {
    fn split(xy: &[[f64; 2]], ids: &mut [usize], k: usize, first: u32, out: &mut [u32]) {
        if k <= 1 || ids.len() <= 1 {
            for &i in ids.iter() {
                out[i] = first;
            }
            return;
        }
        let span = |axis: usize| {
            let (lo, hi) = ids.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &i| {
                (lo.min(xy[i][axis]), hi.max(xy[i][axis]))
            });
            hi - lo
        };
        let axis = if span(0) >= span(1) { 0 } else { 1 };
        let left_k = k / 2;
        let cut = ids.len() * left_k / k;
        ids.select_nth_unstable_by(cut.min(ids.len() - 1), |&a, &b| {
            xy[a][axis].total_cmp(&xy[b][axis]).then(a.cmp(&b))
        });
        let (left, right) = ids.split_at_mut(cut);
        split(xy, left, left_k, first, out);
        split(xy, right, k - left_k, first + left_k as u32, out);
    }
    let mut ids: Vec<usize> = (0..xy.len()).collect();
    let mut out = vec![0u32; xy.len()];
    split(xy, &mut ids, k.max(1), 1, &mut out);
    out
}

/// Place substations: Table I for load, Table II for generation, then pick
/// which substations also get higher-voltage buses.
pub fn place(cfg: &Config, rng: &mut Rng) -> Placement {
    let f = &cfg.footprint;
    let projection = Projection::new((f.lat_min + f.lat_max) / 2.0, (f.lon_min + f.lon_max) / 2.0);
    let n_s = cfg.substations.max(4);
    let g = &cfg.generation;
    let n_g = ((g.share_generation_only * n_s as f64).round() as usize).clamp(1, n_s / 2);
    let n_b = ((g.share_both * n_s as f64).round() as usize).clamp(1, n_s - n_g);
    let n_lb = n_s - n_g;

    // Load substations from population (Table I).
    let mut r = rng.fork(1);
    let population = population(cfg, &projection, n_lb, &mut r);
    let total_people: f64 = population.iter().map(|p| p.people).sum();
    let cap = cfg.population.cap_factor * total_people / n_lb as f64;
    let items: Vec<Item> = population
        .iter()
        .map(|p| Item {
            xy: p.xy,
            weight: p.people,
            group: 0,
        })
        .collect();
    let clusters = agglomerate(&items, n_lb, cap);
    let mut subs: Vec<Substation> = clusters
        .centres
        .iter()
        .zip(&clusters.weights)
        .map(|(&xy, &people)| Substation {
            at: projection.latlon(xy),
            xy,
            kind: Kind::Load,
            load_mw: 0.0,
            people,
            plants: vec![],
            area: 0,
            levels: vec![0],
        })
        .collect();

    // Areas, each with its own consumption per person (Table IV), scaled to
    // the system's total load.
    let n_areas = cfg.area_count();
    let area_of = regions(&subs.iter().map(|s| s.xy).collect::<Vec<_>>(), n_areas);
    let mut r = rng.fork(2);
    let spread = cfg.load.per_capita_spread;
    let factor: Vec<f64> = (0..=n_areas)
        .map(|_| r.range(1.0 - spread, 1.0 + spread))
        .collect();
    for (s, &a) in subs.iter_mut().zip(&area_of) {
        s.area = a;
        s.load_mw = s.people * factor[a as usize];
    }
    let total_load = cfg.load.mw_per_substation * subs.len() as f64;
    let raw: f64 = subs.iter().map(|s| s.load_mw).sum();
    for s in &mut subs {
        s.load_mw = (s.load_mw * total_load / raw * 100.0).round() / 100.0;
    }

    // Power plants.
    let mut r = rng.fork(3);
    let n_plants = ((g.plants_per_substation * n_s as f64).round() as usize).max(n_g + n_b);
    let plants = plants(
        cfg,
        &projection,
        total_load * g.capacity_margin,
        n_plants,
        &population,
        &mut r,
    );

    // Substations with both (Table II, steps 1-2): nearby coal and gas plants
    // until each meets a capacity drawn from the generation-to-load ratio.
    let mut r = rng.fork(4);
    let mut order: Vec<usize> = (0..subs.len()).collect();
    r.shuffle(&mut order);
    let mut taken = vec![false; plants.len()];
    for &si in order.iter().take(n_b) {
        let ln =
            (g.ratio_ln_mean + g.ratio_ln_sd * r.normal()).clamp(g.ratio_ln_min, g.ratio_ln_max);
        let need = ln.exp() * subs[si].load_mw;
        let at = subs[si].xy;
        let mut have = 0.0;
        while have < need {
            let next = plants
                .iter()
                .enumerate()
                .filter(|&(i, p)| !taken[i] && p.fuel.may_share_with_load())
                .min_by(|a, b| dist(at, a.1.xy).total_cmp(&dist(at, b.1.xy)));
            let Some((i, p)) = next else { break };
            taken[i] = true;
            have += p.mw;
            subs[si].plants.push(i);
        }
        if !subs[si].plants.is_empty() {
            subs[si].kind = Kind::Both;
        }
    }

    // Generation-only substations (Table II, step 3): cluster what's left,
    // never mixing hydro, nuclear or renewables with another fuel.
    let rest: Vec<usize> = (0..plants.len()).filter(|&i| !taken[i]).collect();
    let mut by_group: std::collections::BTreeMap<u32, Vec<usize>> = Default::default();
    for &i in &rest {
        by_group.entry(plants[i].fuel.group()).or_default().push(i);
    }
    for (_, members) in by_group {
        let share = (members.len() as f64 / rest.len().max(1) as f64 * n_g as f64).round() as usize;
        let target = share.clamp(1, members.len());
        let items: Vec<Item> = members
            .iter()
            .map(|&i| Item {
                xy: plants[i].xy,
                weight: plants[i].mw,
                group: 0,
            })
            .collect();
        let c = agglomerate(&items, target, f64::INFINITY);
        let first = subs.len();
        for &xy in &c.centres {
            subs.push(Substation {
                at: projection.latlon(xy),
                xy,
                kind: Kind::Generation,
                load_mw: 0.0,
                people: 0.0,
                plants: vec![],
                area: 0,
                levels: vec![0],
            });
        }
        for (k, &i) in members.iter().enumerate() {
            subs[first + c.of_item[k]].plants.push(i);
        }
    }
    // Generation-only substations join the nearest load substation's area.
    let load_count = n_lb;
    for i in load_count..subs.len() {
        let near = (0..load_count)
            .min_by(|&a, &b| dist(subs[i].xy, subs[a].xy).total_cmp(&dist(subs[i].xy, subs[b].xy)))
            .unwrap_or(0);
        subs[i].area = subs[near].area;
    }

    // Higher voltage levels. A substation whose generation or load is more
    // than a few lines at its level could carry gets the next level up;
    // the rest of each level's share is drawn at random, more likely where
    // load and generation are large (paper Section III).
    let carry: Vec<f64> = cfg
        .levels
        .iter()
        .map(|l| 1.8 * crate::params::Design::for_kv(l.kv).largest_rating())
        .collect();
    for s in subs.iter_mut() {
        let size = s.capacity(&plants).max(s.load_mw);
        if cfg.levels.len() > 1 && size > carry[0] {
            // The lowest level that can carry it, else the top one.
            let fit = (1..cfg.levels.len())
                .find(|&l| carry[l] >= size)
                .unwrap_or(cfg.levels.len() - 1);
            s.levels.push(fit);
        }
    }
    let mut r = rng.fork(5);
    for (li, level) in cfg.levels.iter().enumerate().skip(1) {
        let k = ((level.share * subs.len() as f64).round() as usize).clamp(2, subs.len());
        let have = subs.iter().filter(|s| s.levels.contains(&li)).count();
        let w: Vec<f64> = subs
            .iter()
            .map(|s| {
                if s.levels.contains(&li) {
                    0.0
                } else {
                    1.0 + s.load_mw + 3.0 * s.capacity(&plants)
                }
            })
            .collect();
        for i in r.weighted_sample(&w, k.saturating_sub(have)) {
            subs[i].levels.push(li);
        }
    }
    for s in subs.iter_mut() {
        s.levels.sort_unstable();
        s.levels.dedup();
    }
    Placement {
        projection,
        population,
        plants,
        substations: subs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportions_follow_the_settings() {
        let cfg = Config {
            substations: 400,
            ..Config::default()
        };
        let p = place(&cfg, &mut Rng::new(3));
        let count = |k: Kind| p.substations.iter().filter(|s| s.kind == k).count();
        let n = p.substations.len();
        assert!((n as i64 - 400).abs() <= 12, "{n}");
        assert!(count(Kind::Generation) >= 15 && count(Kind::Generation) <= 30);
        assert!(count(Kind::Both) >= 15 && count(Kind::Both) <= 25);
        let load: f64 = p.substations.iter().map(|s| s.load_mw).sum();
        assert!((load - 35.0 * (n - count(Kind::Generation)) as f64).abs() < 50.0);
        let capacity: f64 = p.plants.iter().map(|p| p.mw).sum();
        assert!((capacity / load - 1.3).abs() < 0.02);
        // Every plant is in exactly one substation; renewables stay apart.
        let mut seen = vec![0; p.plants.len()];
        for s in &p.substations {
            for &i in &s.plants {
                seen[i] += 1;
            }
            let groups: std::collections::BTreeSet<u32> =
                s.plants.iter().map(|&i| p.plants[i].fuel.group()).collect();
            assert!(groups.len() <= 1);
            if s.kind == Kind::Both {
                assert!(
                    s.plants
                        .iter()
                        .all(|&i| p.plants[i].fuel.may_share_with_load())
                );
            }
        }
        assert!(seen.iter().all(|&c| c == 1));
        let high = p.substations.iter().filter(|s| s.levels.len() > 1).count();
        assert!((high as f64 / n as f64 - 0.15).abs() < 0.01);
    }

    #[test]
    fn regions_are_balanced() {
        let xy: Vec<[f64; 2]> = (0..100)
            .map(|i| [(i % 10) as f64, (i / 10) as f64])
            .collect();
        let r = regions(&xy, 4);
        for a in 1..=4 {
            assert_eq!(r.iter().filter(|&&x| x == a).count(), 25);
        }
    }
}
