//! Places for the buses no source places, from the branches that reach them.
//!
//! A transformer or switching device keeps both its buses at one place, so a
//! bus tied that way to a placed bus goes exactly there. A line says how far
//! apart its buses are: from its `LEN` where the case gives one, else from
//! its reactance in ohms, which grows with length. Both are scaled to
//! kilometres by the lines whose two ends are already placed (the median
//! ratio of straight distance to length), so the file's units and the
//! detours real lines take cancel out; reactance is scaled per voltage level
//! where enough lines allow it.
//!
//! The remaining buses start at the average of their placed neighbours,
//! ring by ring outward, a bus hanging off a single placed bus starting one
//! line length away from it, on the side away from that bus's other
//! neighbours. Then each is moved in turn to best fit its lines' lengths
//! (stress majorization with the placed buses held still) until nothing
//! moves more than a metre. Without enough placed lines to scale the
//! lengths, the averages stand.

use rs_power::Case;
use std::collections::HashMap;

/// Kilometres per degree of latitude.
const KM_PER_DEGREE: f64 = 111.195;
/// Placed lines needed before lengths are trusted, and per voltage level
/// before that level gets its own reactance scale.
const MIN_SAMPLES: usize = 3;
const MIN_LEVEL_SAMPLES: usize = 5;
/// Lines shorter than this don't stretch the fit (km).
const MIN_TARGET: f64 = 1.0;
const MAX_SWEEPS: usize = 500;
/// Stop when no bus moves farther than this in a sweep (km).
const SETTLED: f64 = 0.001;
/// Turns successive nodes' nudges well apart (radians).
const GOLDEN_ANGLE: f64 = 2.399_963;

/// What one branch says about how far apart its buses are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Span {
    /// A transformer or switching device: both buses share a place.
    Site,
    /// A line: its length in the case's own units (0 when not given).
    Line { length: f64 },
}

/// How the estimates were made, for the summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Line lengths (and reactances where a line has no length).
    Lengths,
    /// Line reactances.
    Reactance,
    /// Averages of neighbours only.
    Neighbours,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::Lengths => "lengths",
            Method::Reactance => "reactance",
            Method::Neighbours => "neighbours",
        }
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Straight distance in km, flat at the pair's mean latitude.
fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let c = ((a.0 + b.0) / 2.0).to_radians().cos();
    let dy = (a.0 - b.0) * KM_PER_DEGREE;
    let dx = (a.1 - b.1) * KM_PER_DEGREE * c;
    dx.hypot(dy)
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// Fills the `None` entries of `place` (one per bus, in case order) that a
/// branch path reaches from a placed bus. `spans` follows `case.branches`;
/// a branch past its end is read from the buses' voltages alone.
pub fn estimate(place: &mut [Option<(f64, f64)>], case: &Case, spans: &[Span]) -> Method {
    let n = place.len();
    let index: HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let kv = |i: usize| case.buses[i].base_kv;
    let known: Vec<bool> = place.iter().map(Option::is_some).collect();

    // Buses that share a place: unplaced ones tied by a transformer or
    // switch become one node, pinned to a placed bus it's tied to.
    let mut parent: Vec<usize> = (0..n).collect();
    let mut pairs = Vec::new();
    let mut ties = Vec::new();
    for (k, br) in case.branches.iter().enumerate() {
        let (Some(&f), Some(&t)) = (index.get(&br.from), index.get(&br.to)) else {
            continue;
        };
        if f == t {
            continue;
        }
        let (a, b) = (kv(f), kv(t));
        let levels_differ = a > 0.0 && b > 0.0 && (a - b).abs() > 0.01 * a.max(b);
        let span = match spans.get(k) {
            Some(Span::Site) => Span::Site,
            _ if levels_differ => Span::Site,
            Some(s) => *s,
            None => Span::Line { length: 0.0 },
        };
        match span {
            Span::Site => ties.push((f, t)),
            Span::Line { length } => {
                let v = a.max(b);
                let ohms = if v > 0.0 && case.base_mva > 0.0 {
                    br.x.abs() * v * v / case.base_mva
                } else {
                    0.0
                };
                pairs.push((f, t, length, ohms, v.round() as i64));
            }
        }
    }
    for &(f, t) in &ties {
        if !known[f] && !known[t] {
            let (rf, rt) = (find(&mut parent, f), find(&mut parent, t));
            if rf != rt {
                parent[rf.max(rt)] = rf.min(rt);
            }
        }
    }
    let rep: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut pos: Vec<Option<(f64, f64)>> = place.to_vec();
    for &(f, t) in &ties {
        for (u, k) in [(f, t), (t, f)] {
            if !known[u] && known[k] && pos[rep[u]].is_none() {
                pos[rep[u]] = place[k];
            }
        }
    }
    let fixed: Vec<bool> = (0..n).map(|r| pos[r].is_some()).collect();
    // Nodes tied together by transformers, for which way a spur points.
    let mut tied: Vec<Vec<usize>> = vec![vec![]; n];
    for &(f, t) in &ties {
        let (a, b) = (rep[f], rep[t]);
        if a != b {
            tied[a].push(b);
            tied[b].push(a);
        }
    }

    // Lines between nodes, each pair once (the shortest of parallel lines).
    let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
    let mut lines: Vec<(usize, usize, f64, f64, i64)> = Vec::new();
    for (f, t, length, ohms, level) in pairs {
        let (a, b) = (rep[f], rep[t]);
        if a == b || (fixed[a] && fixed[b] && !(known[f] && known[t])) {
            continue;
        }
        let key = (a.min(b), a.max(b));
        match seen.get(&key) {
            Some(&j) => {
                let l = &mut lines[j];
                if length > 0.0 && (l.2 == 0.0 || length < l.2) {
                    l.2 = length;
                }
                if ohms > 0.0 && (l.3 == 0.0 || ohms < l.3) {
                    l.3 = ohms;
                }
            }
            None => {
                seen.insert(key, lines.len());
                lines.push((key.0, key.1, length, ohms, level));
            }
        }
    }

    // Scales from lines whose ends the sources placed.
    let mut per_length = Vec::new();
    let mut per_ohm: HashMap<i64, Vec<f64>> = HashMap::new();
    for &(a, b, length, ohms, level) in &lines {
        if !(known[a] && known[b]) {
            continue;
        }
        let d = distance(place[a].unwrap(), place[b].unwrap());
        if d < 0.05 {
            continue;
        }
        if length > 0.0 {
            per_length.push(d / length);
        }
        if ohms > 0.0 {
            per_ohm.entry(level).or_default().push(d / ohms);
        }
    }
    let length_scale = if per_length.len() >= MIN_SAMPLES {
        median(per_length)
    } else {
        None
    };
    let all_ohm: Vec<f64> = per_ohm.values().flatten().copied().collect();
    let ohm_scale = if all_ohm.len() >= MIN_SAMPLES {
        median(all_ohm)
    } else {
        None
    };
    let level_scale: HashMap<i64, f64> = per_ohm
        .into_iter()
        .filter(|(_, v)| v.len() >= MIN_LEVEL_SAMPLES)
        .filter_map(|(k, v)| Some((k, median(v)?)))
        .collect();
    let mut used_length = false;
    let mut used_ohms = false;
    let mut target = |length: f64, ohms: f64, level: i64| -> Option<f64> {
        if let (Some(s), true) = (length_scale, length > 0.0) {
            used_length = true;
            return Some(length * s);
        }
        let s = level_scale.get(&level).copied().or(ohm_scale)?;
        (ohms > 0.0).then(|| {
            used_ohms = true;
            ohms * s
        })
    };
    let targets: Vec<Option<f64>> = lines
        .iter()
        .map(|&(a, b, length, ohms, level)| {
            // Only lines reaching an unplaced node need a length.
            if fixed[a] && fixed[b] {
                None
            } else {
                target(length, ohms, level)
            }
        })
        .collect();
    let typical = median(targets.iter().flatten().copied().collect());
    let method = match (typical, used_length, used_ohms) {
        (None, ..) => Method::Neighbours,
        (_, true, _) => Method::Lengths,
        _ => Method::Reactance,
    };
    let mut adj: Vec<Vec<(usize, Option<f64>)>> = vec![vec![]; n];
    for (&(a, b, ..), &d) in lines.iter().zip(&targets) {
        let d = typical.map(|t| d.unwrap_or(t));
        adj[a].push((b, d));
        adj[b].push((a, d));
    }

    // Starting places, ring by ring outward from the placed nodes.
    let mut frontier: Vec<usize> = (0..n)
        .filter(|&r| {
            rep[r] == r && pos[r].is_none() && adj[r].iter().any(|&(o, _)| pos[o].is_some())
        })
        .collect();
    let mut order = Vec::new();
    while !frontier.is_empty() {
        // How many nodes hang off each lone anchor this round, to fan them.
        let mut fan: HashMap<usize, (usize, usize)> = HashMap::new();
        let mut placed = Vec::with_capacity(frontier.len());
        for &r in &frontier {
            let near: Vec<(usize, (f64, f64), Option<f64>)> = adj[r]
                .iter()
                .filter_map(|&(o, d)| pos[o].map(|p| (o, p, d)))
                .collect();
            let k = near.len() as f64;
            let lat = near.iter().map(|x| x.1.0).sum::<f64>() / k;
            let lon = near.iter().map(|x| x.1.1).sum::<f64>() / k;
            let lone = near.iter().all(|x| distance(x.1, near[0].1) < 1e-6);
            let mut p = (lat, lon);
            if lone && let Some(d) = near[0].2 {
                let anchor = near[0].0;
                let slot = fan.entry(anchor).or_insert((0, 0));
                slot.0 += 1;
                let nth = slot.0 - 1;
                p = away(anchor, near[0].1, d, nth, &adj, &tied, &pos);
            } else if let Some(t) = typical {
                // Nudged off the average, which can sit on a line of
                // symmetry the fit couldn't leave (a bus between two).
                let angle = r as f64 * GOLDEN_ANGLE;
                let c = lat.to_radians().cos().max(1e-6);
                p.0 += 0.05 * t * angle.sin() / KM_PER_DEGREE;
                p.1 += 0.05 * t * angle.cos() / (KM_PER_DEGREE * c);
            }
            placed.push((r, p));
        }
        for &(r, p) in &placed {
            pos[r] = Some(p);
            order.push(r);
        }
        let mut next: Vec<usize> = placed
            .iter()
            .flat_map(|&(r, _)| adj[r].iter().map(|&(o, _)| o))
            .filter(|&o| pos[o].is_none())
            .collect();
        next.sort_unstable();
        next.dedup();
        frontier = next;
    }

    // Fit the lines' lengths, holding the placed nodes still.
    if typical.is_some() {
        for _ in 0..MAX_SWEEPS {
            let mut moved: f64 = 0.0;
            for &r in &order {
                let p = pos[r].unwrap();
                let c = p.0.to_radians().cos().max(1e-6);
                let (mut w_sum, mut lat, mut lon) = (0.0, 0.0, 0.0);
                for &(o, d) in &adj[r] {
                    let (Some(q), Some(d)) = (pos[o], d) else {
                        continue;
                    };
                    let dy = (p.0 - q.0) * KM_PER_DEGREE;
                    let dx = (p.1 - q.1) * KM_PER_DEGREE * c;
                    let now = dx.hypot(dy);
                    let w = 1.0 / d.max(MIN_TARGET).powi(2);
                    let (ty, tx) = if now > 1e-9 {
                        (
                            q.0 + d * dy / now / KM_PER_DEGREE,
                            q.1 + d * dx / now / (KM_PER_DEGREE * c),
                        )
                    } else {
                        q
                    };
                    w_sum += w;
                    lat += w * ty;
                    lon += w * tx;
                }
                if w_sum > 0.0 {
                    let new = (lat / w_sum, lon / w_sum);
                    moved = moved.max(distance(p, new));
                    pos[r] = Some(new);
                }
            }
            if moved < SETTLED {
                break;
            }
        }
    }

    for i in 0..n {
        if place[i].is_none() {
            place[i] = pos[rep[i]];
        }
    }
    method
}

/// A start for the `nth` node hanging off `anchor` alone: `d` km from it,
/// on the side away from its other placed neighbours (and those of the
/// buses tied to it by transformers), fanned 30° apart.
fn away(
    anchor: usize,
    at: (f64, f64),
    d: f64,
    nth: usize,
    adj: &[Vec<(usize, Option<f64>)>],
    tied: &[Vec<usize>],
    pos: &[Option<(f64, f64)>],
) -> (f64, f64) {
    let c = at.0.to_radians().cos().max(1e-6);
    let (mut x, mut y) = (0.0, 0.0);
    for &a in std::iter::once(&anchor).chain(&tied[anchor]) {
        for &(o, _) in &adj[a] {
            if let Some(q) = pos[o] {
                x += (at.1 - q.1) * c;
                y += at.0 - q.0;
            }
        }
    }
    let base = if x.hypot(y) > 1e-12 {
        y.atan2(x)
    } else {
        // Nothing to lean away from: spread by bus index.
        anchor as f64 * GOLDEN_ANGLE
    };
    // 0, +30°, -30°, +60°, ...
    let step = nth.div_ceil(2) as f64 * if nth % 2 == 1 { 1.0 } else { -1.0 };
    let angle = base + step * 30f64.to_radians();
    (
        at.0 + d * angle.sin() / KM_PER_DEGREE,
        at.1 + d * angle.cos() / (KM_PER_DEGREE * c),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buses 1-3 placed on a right angle 1° apart at the equator, 138 kV;
    /// 4 hangs off 1, 5 is a 13.8 kV bus under 2, 6 ties to 1 and 2.
    fn case() -> Case {
        rs_power::parse_matpower(
            "function mpc = t
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
1 3 0 0 0 0 1 1 0 138 1 1.1 0.9;
2 1 0 0 0 0 1 1 0 138 1 1.1 0.9;
3 1 0 0 0 0 1 1 0 138 1 1.1 0.9;
4 1 0 0 0 0 1 1 0 138 1 1.1 0.9;
5 1 0 0 0 0 1 1 0 13.8 1 1.1 0.9;
6 1 0 0 0 0 1 1 0 138 1 1.1 0.9;
];
mpc.gen = [1 0 0 0 0 1 100 1 100 0];
mpc.branch = [
1 2 0 0.1 0 0 0 0 0 0 1 -360 360;
1 3 0 0.1 0 0 0 0 0 0 1 -360 360;
2 3 0 0.1 0 0 0 0 0 0 1 -360 360;
1 4 0 0.1 0 0 0 0 0 0 1 -360 360;
2 5 0 0.1 0 0 0 0 0 0 1 -360 360;
1 6 0 0.1 0 0 0 0 0 0 1 -360 360;
2 6 0 0.1 0 0 0 0 0 0 1 -360 360;
];
",
        )
        .unwrap()
    }

    fn placed() -> Vec<Option<(f64, f64)>> {
        vec![
            Some((0.0, 0.0)),
            Some((0.0, 1.0)),
            Some((1.0, 0.0)),
            None,
            None,
            None,
        ]
    }

    /// Lengths in miles; 1° at the equator is 69.09 mi.
    fn spans(spur: f64, tie: f64) -> Vec<Span> {
        [69.09, 69.09, 97.71, spur, 0.0, tie, tie]
            .into_iter()
            .map(|length| Span::Line { length })
            .collect()
    }

    #[test]
    fn lengths_place_a_spur_a_transformer_and_a_tie() {
        let mut p = placed();
        let method = estimate(&mut p, &case(), &spans(20.0, 50.0));
        assert_eq!(method, Method::Lengths);
        let km = 1.609_344;
        // The spur: 20 mi out of bus 1, away from 2 and 3 (south-west).
        let spur = p[3].unwrap();
        assert!(
            (distance(spur, (0.0, 0.0)) - 20.0 * km).abs() < 0.5,
            "{spur:?}"
        );
        assert!(spur.0 < 0.0 && spur.1 < 0.0, "{spur:?}");
        // The 13.8 kV bus sits exactly at bus 2, a different voltage.
        assert_eq!(p[4], Some((0.0, 1.0)));
        // The tie: 50 mi from both 1 and 2.
        let tie = p[5].unwrap();
        for end in [(0.0, 0.0), (0.0, 1.0)] {
            assert!((distance(tie, end) - 50.0 * km).abs() < 0.5, "{tie:?}");
        }
    }

    #[test]
    fn reactance_stands_in_for_missing_lengths() {
        let mut c = case();
        // The spur has twice the others' reactance, so twice 1°.
        c.branches[3].x = 0.2;
        c.branches[2].x = 0.1414;
        let mut p = placed();
        let method = estimate(&mut p, &c, &spans(0.0, 0.0));
        assert_eq!(method, Method::Reactance);
        let d = distance(p[3].unwrap(), (0.0, 0.0));
        assert!((d - 2.0 * 111.195).abs() < 2.0, "{d}");
    }

    #[test]
    fn too_few_placed_lines_fall_back_to_averages() {
        let mut p = vec![Some((0.0, 0.0)), Some((0.0, 1.0)), None, None, None, None];
        let method = estimate(&mut p, &case(), &spans(20.0, 50.0));
        assert_eq!(method, Method::Neighbours);
        // Bus 6 between 1 and 2; bus 4 on bus 1.
        assert_eq!(p[5], Some((0.0, 0.5)));
        assert_eq!(p[3], Some((0.0, 0.0)));
    }
}
