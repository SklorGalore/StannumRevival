//! Agglomerative clustering of weighted points (paper Tables I and II):
//! repeatedly join the two closest clusters, place the result at their
//! weighted centre, and stop at the target count.
//!
//! The paper measures cluster distance as the weighted average distance
//! between members (its eq. 1), which costs O(N²) memory. Here the distance
//! is between weighted centres, and only clusters that are Delaunay
//! neighbours are compared: the nearest cluster is always a Delaunay
//! neighbour, so this finds the same closest pair at a fraction of the cost.

use crate::geo::dist;
use crate::graph::delaunay;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub xy: [f64; 2],
    pub weight: f64,
    /// Items join only items of the same group (fuel rules for plants).
    pub group: u32,
}

#[derive(Debug)]
pub struct Clusters {
    /// Cluster index (0..len) of each item.
    pub of_item: Vec<usize>,
    pub centres: Vec<[f64; 2]>,
    pub weights: Vec<f64>,
    pub groups: Vec<u32>,
}

#[derive(PartialEq)]
struct Pair(f64, usize, usize, u32, u32);
impl Eq for Pair {}
impl PartialOrd for Pair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Pair {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .total_cmp(&other.0)
            .then((self.1, self.2).cmp(&(other.1, other.2)))
    }
}

/// Cluster `items` down to `target` clusters, never letting a cluster's
/// weight pass `cap` (use infinity for no cap). Stops early when no allowed
/// join is left.
pub fn agglomerate(items: &[Item], target: usize, cap: f64) -> Clusters {
    let n = items.len();
    let mut centre: Vec<[f64; 2]> = items.iter().map(|i| i.xy).collect();
    let mut weight: Vec<f64> = items.iter().map(|i| i.weight.max(1e-9)).collect();
    let group: Vec<u32> = items.iter().map(|i| i.group).collect();
    let mut version = vec![0u32; n];
    let mut alive = vec![true; n];
    let mut parent: Vec<usize> = (0..n).collect();
    let mut count = n;

    fn root(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    while count > target {
        // Neighbours among the clusters still alive.
        let ids: Vec<usize> = (0..n).filter(|&i| alive[i]).collect();
        let pts: Vec<[f64; 2]> = ids.iter().map(|&i| centre[i]).collect();
        let local = delaunay(&pts);
        let mut nbrs: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut heap = BinaryHeap::new();
        for (li, list) in local.iter().enumerate() {
            let a = ids[li];
            for &lj in list {
                let b = ids[lj];
                nbrs[a].push(b);
                if a < b && group[a] == group[b] && weight[a] + weight[b] <= cap {
                    heap.push(Reverse(Pair(dist(centre[a], centre[b]), a, b, 0, 0)));
                }
            }
        }
        let before = count;
        while count > target {
            let Some(Reverse(Pair(_, a, b, va, vb))) = heap.pop() else {
                break;
            };
            if !alive[a] || !alive[b] || version[a] != va || version[b] != vb {
                continue;
            }
            if weight[a] + weight[b] > cap {
                continue;
            }
            // Join b into a.
            let w = weight[a] + weight[b];
            centre[a] = [
                (centre[a][0] * weight[a] + centre[b][0] * weight[b]) / w,
                (centre[a][1] * weight[a] + centre[b][1] * weight[b]) / w,
            ];
            weight[a] = w;
            alive[b] = false;
            parent[b] = a;
            version[a] += 1;
            count -= 1;
            let mut merged = std::mem::take(&mut nbrs[a]);
            merged.append(&mut nbrs[b]);
            let mut next: Vec<usize> = merged
                .into_iter()
                .map(|x| root(&mut parent, x))
                .filter(|&x| x != a && alive[x])
                .collect();
            next.sort_unstable();
            next.dedup();
            for &c in &next {
                if group[c] == group[a] && weight[a] + weight[c] <= cap {
                    let (x, y) = if a < c { (a, c) } else { (c, a) };
                    heap.push(Reverse(Pair(
                        dist(centre[a], centre[c]),
                        x,
                        y,
                        version[x],
                        version[y],
                    )));
                }
            }
            nbrs[a] = next;
        }
        if count == before {
            break; // nothing left that may join
        }
    }

    let mut index = vec![usize::MAX; n];
    let mut centres = Vec::new();
    let mut weights = Vec::new();
    let mut groups = Vec::new();
    for i in 0..n {
        if alive[i] {
            index[i] = centres.len();
            centres.push(centre[i]);
            weights.push(weight[i]);
            groups.push(group[i]);
        }
    }
    let of_item = (0..n).map(|i| index[root(&mut parent, i)]).collect();
    Clusters {
        of_item,
        centres,
        weights,
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(x: f64, y: f64, w: f64, g: u32) -> Item {
        Item {
            xy: [x, y],
            weight: w,
            group: g,
        }
    }

    #[test]
    fn joins_nearby_points_first() {
        let items = [
            item(0., 0., 1., 0),
            item(0.1, 0., 1., 0),
            item(10., 0., 1., 0),
            item(10.1, 0., 1., 0),
        ];
        let c = agglomerate(&items, 2, f64::INFINITY);
        assert_eq!(c.centres.len(), 2);
        assert_eq!(c.of_item[0], c.of_item[1]);
        assert_eq!(c.of_item[2], c.of_item[3]);
        assert_ne!(c.of_item[0], c.of_item[2]);
    }

    #[test]
    fn respects_cap_and_groups() {
        // Different groups never join.
        let c = agglomerate(
            &[item(0., 0., 5., 1), item(0.1, 0., 5., 2)],
            1,
            f64::INFINITY,
        );
        assert_eq!(c.centres.len(), 2);
        let capped = agglomerate(&[item(0., 0., 5., 0), item(1., 0., 5., 0)], 1, 8.0);
        assert_eq!(capped.centres.len(), 2);
    }
}
