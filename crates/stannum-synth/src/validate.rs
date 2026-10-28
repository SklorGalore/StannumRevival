//! The paper's validation criteria, measured on any network whose
//! substations have coordinates: lines per substation, overlap with the
//! minimum spanning tree and Delaunay triangulation (Table III), line
//! crossings, and connectivity at each level and under the loss of any one
//! substation.

use crate::geo::{LatLon, Projection, dist, segments_cross};
use crate::graph::{UnionFind, blocks, delaunay, euclidean_mst, hops};
use crate::lines::{Category, SegmentGrid};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Debug, Serialize)]
pub struct LevelStats {
    pub kv: f64,
    /// Substations with a bus at this level.
    pub n: usize,
    /// Lines between distinct substation pairs (parallel circuits count once).
    pub m: usize,
    pub m_per_n: f64,
    /// Share of lines on the minimum spanning tree.
    pub mst: f64,
    /// Share on the Delaunay triangulation (minimum spanning tree included).
    pub delaunay: f64,
    pub two: f64,
    pub three: f64,
    pub four: f64,
    pub five_plus: f64,
    /// Share of lines that cross another line of this level.
    pub crossings: f64,
    pub connected: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Stats {
    pub levels: Vec<LevelStats>,
    /// Every substation reaches every other.
    pub connected: bool,
    /// Substations whose loss would split the network (0 means it survives
    /// any single substation outage).
    pub cut_substations: usize,
}

/// A line between substations `a` and `b` at the level with nominal `kv`.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub kv: f64,
    pub a: usize,
    pub b: usize,
}

/// Measure `spans` between substations at `at`. `levels` lists each
/// substation's nominal voltages (a substation with no listed level is
/// taken to be at every level its lines use).
pub fn measure(at: &[LatLon], spans: &[Span], levels: &[Vec<f64>]) -> Stats {
    let n = at.len();
    let (lat0, lon0) = at.iter().fold((0.0, 0.0), |(a, b), p| {
        (a + p.lat / n.max(1) as f64, b + p.lon / n.max(1) as f64)
    });
    let proj = Projection::new(lat0, lon0);
    let xy: Vec<[f64; 2]> = at.iter().map(|p| proj.xy(*p)).collect();
    let key = |kv: f64| (kv * 1000.0).round() as i64;
    let mut kvs: BTreeMap<i64, f64> = BTreeMap::new();
    for s in spans {
        kvs.insert(key(s.kv), s.kv);
    }
    for list in levels {
        for &kv in list {
            kvs.insert(key(kv), kv);
        }
    }
    let mut out = Vec::new();
    for (&k, &kv) in &kvs {
        let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
        for s in spans.iter().filter(|s| key(s.kv) == k && s.a != s.b) {
            pairs.insert((s.a.min(s.b), s.a.max(s.b)));
        }
        let mut ids: BTreeSet<usize> = (0..n)
            .filter(|&i| {
                levels
                    .get(i)
                    .is_some_and(|l| l.iter().any(|&v| key(v) == k))
            })
            .collect();
        for &(a, b) in &pairs {
            ids.insert(a);
            ids.insert(b);
        }
        let ids: Vec<usize> = ids.into_iter().collect();
        if ids.is_empty() {
            continue;
        }
        let local: std::collections::HashMap<usize, usize> =
            ids.iter().enumerate().map(|(i, &s)| (s, i)).collect();
        let lxy: Vec<[f64; 2]> = ids.iter().map(|&i| xy[i]).collect();
        let adj = delaunay(&lxy);
        let mst: HashSet<(usize, usize)> = euclidean_mst(&lxy, &adj).into_iter().collect();
        let mut count: BTreeMap<u8, usize> = BTreeMap::new();
        let mut on_mst = 0;
        let mut uf = UnionFind::new(ids.len());
        for &(a, b) in &pairs {
            let (la, lb) = (local[&a], local[&b]);
            uf.union(la, lb);
            let h = hops(&adj, la, lb, 5).unwrap_or(5).max(1);
            *count.entry(h).or_default() += 1;
            if mst.contains(&(la.min(lb), la.max(lb))) {
                on_mst += 1;
            }
        }
        // Lines that cross another line of this level.
        let segs: Vec<(usize, usize)> = pairs.iter().copied().collect();
        let mut lengths: Vec<f64> = segs.iter().map(|&(a, b)| dist(xy[a], xy[b])).collect();
        lengths.sort_by(f64::total_cmp);
        let mut grid =
            SegmentGrid::new(2.0 * lengths.get(lengths.len() / 2).copied().unwrap_or(1.0));
        for (i, &(a, b)) in segs.iter().enumerate() {
            grid.insert(i as u32, xy[a], xy[b]);
        }
        let mut near = Vec::new();
        let crossing = segs
            .iter()
            .enumerate()
            .filter(|&(i, &(a, b))| {
                grid.near(xy[a], xy[b], &mut near);
                near.iter().any(|&j| {
                    let (c, d) = segs[j as usize];
                    j as usize != i && segments_cross(xy[a], xy[b], xy[c], xy[d])
                })
            })
            .count();
        let m = pairs.len();
        let share = |x: usize| if m == 0 { 0.0 } else { x as f64 / m as f64 };
        let c = |h: u8| count.get(&h).copied().unwrap_or(0);
        out.push(LevelStats {
            kv,
            n: ids.len(),
            m,
            m_per_n: m as f64 / ids.len() as f64,
            mst: share(on_mst),
            delaunay: share(c(1)),
            two: share(c(2)),
            three: share(c(3)),
            four: share(c(4)),
            five_plus: share(c(5)),
            crossings: share(crossing),
            connected: uf.components == 1,
        });
    }
    let edges: Vec<(usize, usize)> = spans
        .iter()
        .filter(|s| s.a != s.b)
        .map(|s| (s.a, s.b))
        .collect();
    let mut uf = UnionFind::new(n);
    for &(a, b) in &edges {
        uf.union(a, b);
    }
    let member = blocks(n, &edges);
    Stats {
        levels: out,
        connected: uf.components == 1,
        cut_substations: member.iter().filter(|m| m.len() > 1).count(),
    }
}

/// A row of the statistics table: name, target, and how to show a level.
type Row = (
    &'static str,
    &'static str,
    Box<dyn Fn(&LevelStats) -> String>,
);

impl Stats {
    /// The paper's Table VI layout, as Markdown.
    pub fn table(&self) -> String {
        let mut s = String::from("| Statistic | Target |");
        for l in &self.levels {
            s.push_str(&format!(" {} kV |", l.kv));
        }
        s.push_str("\n| --- | --- |");
        for _ in &self.levels {
            s.push_str(" --- |");
        }
        s.push('\n');
        let pct = |x: f64| format!("{:.1}%", 100.0 * x);
        let rows: [Row; 10] = [
            ("n", "", Box::new(|l| l.n.to_string())),
            ("m", "", Box::new(|l| l.m.to_string())),
            ("m/n", "1.22", Box::new(|l| format!("{:.3}", l.m_per_n))),
            (
                "Minimum spanning tree",
                "50%",
                Box::new(move |l| pct(l.mst)),
            ),
            (
                "Delaunay triangulation",
                "70%",
                Box::new(move |l| pct(l.delaunay)),
            ),
            ("Delaunay 2 neighbor", "25%", Box::new(move |l| pct(l.two))),
            ("Delaunay 3 neighbor", "5%", Box::new(move |l| pct(l.three))),
            (
                "Delaunay 4+ neighbor",
                "",
                Box::new(move |l| pct(l.four + l.five_plus)),
            ),
            (
                "Line intersections",
                "< 20%",
                Box::new(move |l| pct(l.crossings)),
            ),
            (
                "Connected",
                "yes",
                Box::new(|l| {
                    if l.connected {
                        "yes".into()
                    } else {
                        "no".into()
                    }
                }),
            ),
        ];
        for (name, target, f) in rows.iter() {
            s.push_str(&format!("| {name} | {target} |"));
            for l in &self.levels {
                s.push_str(&format!(" {} |", f(l)));
            }
            s.push('\n');
        }
        s
    }
}

/// The category a line between two substations would have on its level.
pub fn category(hops: u8, on_mst: bool) -> Category {
    match hops {
        0 | 1 if on_mst => Category::Mst,
        0 | 1 => Category::Delaunay,
        2 => Category::Two,
        3 => Category::Three,
        _ => Category::Farther,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_on_a_square_grid() {
        let at: Vec<LatLon> = (0..4)
            .map(|i| LatLon {
                lat: 30.0 + (i / 2) as f64,
                lon: -97.0 + (i % 2) as f64 * 1.15,
            })
            .collect();
        let spans: Vec<Span> = [(0, 1), (1, 3), (3, 2), (2, 0)]
            .iter()
            .map(|&(a, b)| Span { kv: 115.0, a, b })
            .collect();
        let s = measure(&at, &spans, &[]);
        let l = &s.levels[0];
        assert_eq!((l.n, l.m), (4, 4));
        assert!((l.delaunay - 1.0).abs() < 1e-9);
        assert!((l.mst - 0.75).abs() < 1e-9);
        assert_eq!(l.crossings, 0.0);
        assert!(s.connected && s.cut_substations == 0);
        // Add both diagonals: one is not on the triangulation, and they cross.
        let mut more = spans.clone();
        more.push(Span {
            kv: 115.0,
            a: 0,
            b: 3,
        });
        more.push(Span {
            kv: 115.0,
            a: 1,
            b: 2,
        });
        let s = measure(&at, &more, &[]);
        assert!((s.levels[0].crossings - 2.0 / 6.0).abs() < 1e-9);
        assert!((s.levels[0].two - 1.0 / 6.0).abs() < 1e-9);
    }
}
