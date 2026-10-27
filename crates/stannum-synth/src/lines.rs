//! Sections III-B to III-E: place transmission lines at every voltage level
//! by ranking candidate lines with the penalties of Table V.
//!
//! Candidates are pairs of substations within three steps of each other on
//! that level's Delaunay triangulation (about 23 per substation). Each
//! round, every candidate is scored by length, estimated dc flow, how far
//! ahead of quota its category is, whether it helps connectivity, and how
//! many lines it would cross; the best few at each level are added.

use crate::config::Config;
use crate::dcflow;
use crate::geo::{dist, haversine_miles, segments_cross};
use crate::graph::{UnionFind, blocks, delaunay, euclidean_mst, pairs_within, share_block};
use crate::params::Design;
use crate::substations::Placement;
use serde::Serialize;
use std::collections::HashMap;

/// Where a line sits relative to its level's Delaunay triangulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// An edge of the Euclidean minimum spanning tree.
    Mst,
    /// Another Delaunay edge.
    Delaunay,
    /// Two steps apart on the triangulation.
    Two,
    /// Three steps apart.
    Three,
    /// Four or more steps apart (only seen when validating other cases).
    Farther,
}

#[derive(Clone, Debug, Serialize)]
pub struct Line {
    /// Index into the config's levels.
    pub level: usize,
    /// Substation indices, a < b.
    pub a: usize,
    pub b: usize,
    pub miles: f64,
    pub category: Category,
}

struct Candidate {
    level: usize,
    a: usize,
    b: usize,
    miles: f64,
    category: Category,
    crossings: u32,
    placed: bool,
}

/// A uniform grid of segment ids, for finding which candidates a new line
/// crosses.
pub(crate) struct SegmentGrid {
    cell: f64,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl SegmentGrid {
    pub(crate) fn new(cell: f64) -> Self {
        Self {
            cell: cell.max(1e-6),
            cells: HashMap::new(),
        }
    }

    fn range(&self, a: [f64; 2], b: [f64; 2]) -> (i32, i32, i32, i32) {
        let c = |v: f64| (v / self.cell).floor() as i32;
        (
            c(a[0].min(b[0])),
            c(a[0].max(b[0])),
            c(a[1].min(b[1])),
            c(a[1].max(b[1])),
        )
    }

    pub(crate) fn insert(&mut self, id: u32, a: [f64; 2], b: [f64; 2]) {
        let (x0, x1, y0, y1) = self.range(a, b);
        for x in x0..=x1 {
            for y in y0..=y1 {
                self.cells.entry((x, y)).or_default().push(id);
            }
        }
    }

    pub(crate) fn near(&self, a: [f64; 2], b: [f64; 2], out: &mut Vec<u32>) {
        out.clear();
        let (x0, x1, y0, y1) = self.range(a, b);
        for x in x0..=x1 {
            for y in y0..=y1 {
                if let Some(ids) = self.cells.get(&(x, y)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

/// Placement statistics per level, for the report.
#[derive(Clone, Debug, Serialize)]
pub struct PlacementSummary {
    pub kv: f64,
    pub substations: usize,
    pub candidates: usize,
    pub target: usize,
    pub placed: usize,
    pub rounds: usize,
}

pub struct Network {
    pub lines: Vec<Line>,
    /// Lines added after the main rounds to join split levels and remove
    /// cut substations.
    pub repair_lines: usize,
    pub summary: Vec<PlacementSummary>,
}

#[allow(clippy::too_many_arguments)]
fn add(
    id: u32,
    cands: &mut [Candidate],
    grids: &mut [SegmentGrid],
    level_uf: &mut [UnionFind],
    system_uf: &mut UnionFind,
    degree: &mut [Vec<u32>],
    counts: &mut [HashMap<Category, usize>],
    placed: &mut [usize],
    real: &mut Vec<(usize, usize, f64)>,
    lines: &mut Vec<Line>,
    xy: &[[f64; 2]],
    x_mi_pu: &[f64],
) {
    let c = &mut cands[id as usize];
    c.placed = true;
    let (l, a, b, cat, miles) = (c.level, c.a, c.b, c.category, c.miles);
    level_uf[l].union(a, b);
    system_uf.union(a, b);
    degree[l][a] += 1;
    degree[l][b] += 1;
    *counts[l].entry(cat).or_default() += 1;
    placed[l] += 1;
    real.push((a, b, 1.0 / (x_mi_pu[l] * miles.max(0.1))));
    lines.push(Line {
        level: l,
        a,
        b,
        miles,
        category: cat,
    });
    // Candidates this line crosses become less attractive.
    let mut near = Vec::new();
    grids[l].near(xy[a], xy[b], &mut near);
    for &o in &near {
        let other = &cands[o as usize];
        if o != id && other.level == l && segments_cross(xy[a], xy[b], xy[other.a], xy[other.b]) {
            cands[o as usize].crossings += 1;
        }
    }
}

pub fn place(cfg: &Config, placement: &Placement) -> Network {
    let subs = &placement.substations;
    let n = subs.len();
    let lc = &cfg.lines;
    let pen = &lc.penalties;
    let designs: Vec<Design> = cfg.levels.iter().map(|l| Design::for_kv(l.kv)).collect();
    let quota = |c: Category| match c {
        Category::Mst => lc.quota_mst,
        Category::Delaunay => lc.quota_delaunay,
        Category::Two => lc.quota_two,
        Category::Three | Category::Farther => lc.quota_three,
    };

    // Candidates at each level.
    let mut cands: Vec<Candidate> = Vec::new();
    let mut by_level: Vec<Vec<u32>> = vec![Vec::new(); cfg.levels.len()];
    let mut targets = vec![0usize; cfg.levels.len()];
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); cfg.levels.len()];
    for (i, s) in subs.iter().enumerate() {
        for &l in &s.levels {
            members[l].push(i);
        }
    }
    let mut grids: Vec<SegmentGrid> = Vec::new();
    for (l, ids) in members.iter().enumerate() {
        let xy: Vec<[f64; 2]> = ids.iter().map(|&i| subs[i].xy).collect();
        let adj = delaunay(&xy);
        let mst: std::collections::HashSet<(usize, usize)> =
            euclidean_mst(&xy, &adj).into_iter().collect();
        let mut edge_lengths: Vec<f64> = adj
            .iter()
            .enumerate()
            .flat_map(|(a, n)| n.iter().filter(move |&&b| b > a).map(move |&b| (a, b)))
            .map(|(a, b)| dist(xy[a], xy[b]))
            .collect();
        edge_lengths.sort_by(f64::total_cmp);
        let longest = lc.longest
            * edge_lengths
                .get(edge_lengths.len() / 2)
                .copied()
                .unwrap_or(f64::INFINITY);
        let mut lengths = Vec::new();
        for (a, b, hops) in pairs_within(&adj, 3) {
            let (la, lb) = (a as usize, b as usize);
            if !mst.contains(&(la, lb)) && dist(xy[la], xy[lb]) > longest {
                continue;
            }
            let category = match hops {
                1 if mst.contains(&(la, lb)) => Category::Mst,
                1 => Category::Delaunay,
                2 => Category::Two,
                _ => Category::Three,
            };
            let (sa, sb) = (ids[la], ids[lb]);
            let id = cands.len() as u32;
            lengths.push(dist(subs[sa].xy, subs[sb].xy));
            cands.push(Candidate {
                level: l,
                a: sa.min(sb),
                b: sa.max(sb),
                miles: haversine_miles(subs[sa].at, subs[sb].at),
                category,
                crossings: 0,
                placed: false,
            });
            by_level[l].push(id);
        }
        targets[l] = ((lc.per_substation * ids.len() as f64).round() as usize)
            .max(ids.len().saturating_sub(1))
            .min(by_level[l].len());
        lengths.sort_by(f64::total_cmp);
        let median = lengths.get(lengths.len() / 2).copied().unwrap_or(1.0);
        let mut grid = SegmentGrid::new(2.0 * median);
        for &id in &by_level[l] {
            let c = &cands[id as usize];
            grid.insert(id, subs[c.a].xy, subs[c.b].xy);
        }
        grids.push(grid);
    }

    // Temporary impedances along the Euclidean minimum spanning tree of all
    // substations keep the dc power flow solvable until real lines connect
    // everything (Section III-D).
    let xy: Vec<[f64; 2]> = subs.iter().map(|s| s.xy).collect();
    let all_xy = &xy;
    let temp: Vec<(usize, usize, f64)> = euclidean_mst(all_xy, &delaunay(all_xy))
        .into_iter()
        .map(|(a, b)| (a, b, haversine_miles(subs[a].at, subs[b].at).max(0.1)))
        .collect();
    let x_mi_pu: Vec<f64> = designs
        .iter()
        .map(|d| d.typical_x_ohm_mi() / (d.kv * d.kv / cfg.base_mva))
        .collect();
    let flow_cap: Vec<f64> = designs.iter().map(|d| 2.0 * d.largest_rating()).collect();

    // Injections: load, and generation dispatched in proportion to capacity.
    let capacity: Vec<f64> = subs.iter().map(|s| s.capacity(&placement.plants)).collect();
    let total_load: f64 = subs.iter().map(|s| s.load_mw).sum();
    let total_cap: f64 = capacity.iter().sum::<f64>().max(1e-9);
    let p: Vec<f64> = subs
        .iter()
        .zip(&capacity)
        .map(|(s, c)| (c * total_load / total_cap - s.load_mw) / cfg.base_mva)
        .collect();
    let ground = (0..n)
        .max_by(|&a, &b| capacity[a].total_cmp(&capacity[b]))
        .unwrap_or(0);

    let mut level_uf: Vec<UnionFind> = (0..cfg.levels.len()).map(|_| UnionFind::new(n)).collect();
    let mut degree: Vec<Vec<u32>> = vec![vec![0; n]; cfg.levels.len()];
    let mut placed = vec![0usize; cfg.levels.len()];
    let mut counts: Vec<HashMap<Category, usize>> = vec![HashMap::new(); cfg.levels.len()];
    let mut system_uf = UnionFind::new(n);
    let mut lines: Vec<Line> = Vec::new();
    let mut real: Vec<(usize, usize, f64)> = Vec::new();
    let mut theta: Vec<f64> = vec![0.0; n];
    let mut rounds_used = vec![0usize; cfg.levels.len()];
    let rounds = lc.iterations.max(1);
    let growth = 1e4f64.powf(1.0 / rounds as f64);
    let dc_every = match lc.dc_every {
        0 if n > 5000 => (rounds / 150).max(1),
        0 => 1,
        k => k,
    };

    let score = |c: &Candidate,
                 theta: &[f64],
                 level_uf: &mut [UnionFind],
                 degree: &[Vec<u32>],
                 counts: &[HashMap<Category, usize>],
                 placed: &[usize],
                 member: &[Vec<u32>]|
     -> f64 {
        let l = c.level;
        let mut s = pen.per_mile * c.miles;
        let mw = ((theta[c.a] - theta[c.b]).abs() / (x_mi_pu[l] * c.miles.max(0.1)) * cfg.base_mva)
            .min(flow_cap[l]);
        s += pen.per_mw * mw;
        let have = counts[l].get(&c.category).copied().unwrap_or(0) as f64;
        if have >= quota(c.category) * (placed[l] as f64 + 1.0) {
            s += pen.over_quota;
        }
        let uf = &mut level_uf[l];
        if uf.find(c.a) != uf.find(c.b) || degree[l][c.a] <= 1 || degree[l][c.b] <= 1 {
            s += pen.level_connectivity;
        }
        if !share_block(member, c.a, c.b) {
            s += pen.system_connectivity;
        }
        s + pen.crossing * c.crossings as f64
    };

    for round in 0..rounds {
        let open: Vec<usize> = (0..cfg.levels.len())
            .filter(|&l| placed[l] < targets[l])
            .collect();
        if open.is_empty() {
            break;
        }
        if round % dc_every == 0 {
            let mut edges = real.clone();
            if system_uf.components > 1 {
                let scale = growth.powi(round as i32);
                for &(a, b, miles) in &temp {
                    edges.push((a, b, 1.0 / (x_mi_pu[0] * miles * scale)));
                }
            }
            dcflow::solve(n, &edges, &p, ground, &mut theta);
        }
        let member = blocks(n, &real.iter().map(|&(a, b, _)| (a, b)).collect::<Vec<_>>());
        let comp: Vec<usize> = (0..n).map(|i| system_uf.find(i)).collect();
        for l in open {
            rounds_used[l] += 1;
            let left = targets[l].saturating_sub(placed[l]);
            let rounds_left = rounds.saturating_sub(round).max(1);
            let k = left.div_ceil(rounds_left).max(1);
            let mut scored: Vec<(f64, u32)> = by_level[l]
                .iter()
                .filter(|&&id| !cands[id as usize].placed)
                .map(|&id| {
                    let c = &cands[id as usize];
                    (
                        score(c, &theta, &mut level_uf, &degree, &counts, &placed, &member),
                        id,
                    )
                })
                .collect();
            if scored.is_empty() {
                targets[l] = placed[l];
                continue;
            }
            // Once the lines left at this level are about what it takes to
            // join its split parts and close loops around its cut
            // substations, spend them only on that.
            let parts = level_uf[l].components - (n - members[l].len());
            let cuts = members[l].iter().filter(|&&i| member[i].len() > 1).count();
            if placed[l] + parts.saturating_sub(1) + cuts / 2 >= targets[l] {
                let helps: Vec<(f64, u32)> = scored
                    .iter()
                    .copied()
                    .filter(|&(_, id)| {
                        let c = &cands[id as usize];
                        let uf = &mut level_uf[l];
                        uf.find(c.a) != uf.find(c.b)
                            || (comp[c.a] == comp[c.b] && !share_block(&member, c.a, c.b))
                    })
                    .collect();
                if !helps.is_empty() {
                    scored = helps;
                }
            }
            let short = (3 * k).min(scored.len());
            scored
                .select_nth_unstable_by(short - 1, |a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            scored.truncate(short);
            for _ in 0..k {
                // Each pick changes connectivity and quotas, so re-score the
                // shortlist before every pick.
                let mut best: Option<(f64, usize)> = None;
                for (pos, &(_, id)) in scored.iter().enumerate() {
                    let c = &cands[id as usize];
                    if c.placed {
                        continue;
                    }
                    let now = score(c, &theta, &mut level_uf, &degree, &counts, &placed, &member);
                    if best.is_none_or(|(b, _)| now < b) {
                        best = Some((now, pos));
                    }
                }
                let Some((_, pos)) = best else { break };
                let id = scored[pos].1;
                add(
                    id,
                    &mut cands,
                    &mut grids,
                    &mut level_uf,
                    &mut system_uf,
                    &mut degree,
                    &mut counts,
                    &mut placed,
                    &mut real,
                    &mut lines,
                    &xy,
                    &x_mi_pu,
                );
            }
        }
    }

    // Repair: the quotas and the line budget can leave a level split, or
    // substations whose loss would split the network. Join what's left with
    // the shortest candidates that cross the fewest lines, and count them.
    let cost = |c: &Candidate| pen.per_mile * c.miles + pen.crossing * c.crossings as f64;
    let mut repair_lines = 0;
    for l in 0..cfg.levels.len() {
        let mut order: Vec<u32> = by_level[l]
            .iter()
            .copied()
            .filter(|&id| !cands[id as usize].placed)
            .collect();
        order.sort_by(|&a, &b| {
            cost(&cands[a as usize])
                .total_cmp(&cost(&cands[b as usize]))
                .then(a.cmp(&b))
        });
        for id in order {
            let c = &cands[id as usize];
            if level_uf[l].find(c.a) != level_uf[l].find(c.b) {
                add(
                    id,
                    &mut cands,
                    &mut grids,
                    &mut level_uf,
                    &mut system_uf,
                    &mut degree,
                    &mut counts,
                    &mut placed,
                    &mut real,
                    &mut lines,
                    &xy,
                    &x_mi_pu,
                );
                repair_lines += 1;
            }
        }
    }
    // Then close loops around cut substations, shortest first. Passes add
    // one line per block at a time while that adds many; the last few are
    // added one by one, re-reading the blocks after each (blocks only ever
    // merge, so one sweep in cost order is enough).
    let edges_now =
        |real: &[(usize, usize, f64)]| real.iter().map(|&(a, b, _)| (a, b)).collect::<Vec<_>>();
    loop {
        let member = blocks(n, &edges_now(&real));
        let mut order: Vec<u32> = (0..cands.len() as u32)
            .filter(|&id| {
                let c = &cands[id as usize];
                !c.placed && !share_block(&member, c.a, c.b)
            })
            .collect();
        order.sort_by(|&a, &b| {
            cost(&cands[a as usize])
                .total_cmp(&cost(&cands[b as usize]))
                .then(a.cmp(&b))
        });
        let mut touched = std::collections::HashSet::new();
        let mut added = 0;
        for id in order {
            let c = &cands[id as usize];
            if member[c.a]
                .iter()
                .chain(&member[c.b])
                .any(|b| touched.contains(b))
            {
                continue;
            }
            touched.extend(member[c.a].iter().chain(&member[c.b]).copied());
            add(
                id,
                &mut cands,
                &mut grids,
                &mut level_uf,
                &mut system_uf,
                &mut degree,
                &mut counts,
                &mut placed,
                &mut real,
                &mut lines,
                &xy,
                &x_mi_pu,
            );
            added += 1;
        }
        repair_lines += added;
        if added < 10 {
            break;
        }
    }
    let mut member = blocks(n, &edges_now(&real));
    let mut order: Vec<u32> = (0..cands.len() as u32)
        .filter(|&id| {
            let c = &cands[id as usize];
            !c.placed && !share_block(&member, c.a, c.b)
        })
        .collect();
    order.sort_by(|&a, &b| {
        cost(&cands[a as usize])
            .total_cmp(&cost(&cands[b as usize]))
            .then(a.cmp(&b))
    });
    for id in order {
        if member.iter().all(|m| m.len() <= 1) {
            break;
        }
        let c = &cands[id as usize];
        // Only lines that close a loop through a cut substation help.
        let cut_end = member[c.a].len() > 1
            || member[c.b].len() > 1
            || member[c.a].is_empty()
            || member[c.b].is_empty();
        if share_block(&member, c.a, c.b) || !cut_end && !member.iter().any(|m| m.len() > 1) {
            continue;
        }
        add(
            id,
            &mut cands,
            &mut grids,
            &mut level_uf,
            &mut system_uf,
            &mut degree,
            &mut counts,
            &mut placed,
            &mut real,
            &mut lines,
            &xy,
            &x_mi_pu,
        );
        repair_lines += 1;
        member = blocks(n, &edges_now(&real));
    }

    let summary = cfg
        .levels
        .iter()
        .enumerate()
        .map(|(l, lv)| PlacementSummary {
            kv: lv.kv,
            substations: members[l].len(),
            candidates: by_level[l].len(),
            target: targets[l],
            placed: placed[l],
            rounds: rounds_used[l],
        })
        .collect();
    Network {
        lines,
        repair_lines,
        summary,
    }
}
