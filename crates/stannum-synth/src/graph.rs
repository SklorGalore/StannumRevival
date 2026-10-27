//! Graph tools on substation points: Delaunay neighbours, the Euclidean
//! minimum spanning tree, hop distances along the triangulation, and
//! connectivity (components and biconnected blocks).

use crate::geo::dist;
use std::collections::{HashMap, VecDeque};

pub struct UnionFind {
    parent: Vec<usize>,
    size: Vec<usize>,
    pub components: usize,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            size: vec![1; n],
            components: n,
        }
    }

    pub fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Join the sets of a and b; false when they were already joined.
    pub fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return false;
        }
        if self.size[a] < self.size[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b] = a;
        self.size[a] += self.size[b];
        self.components -= 1;
        true
    }
}

/// Delaunay neighbours of each point (sorted, no duplicates). Points that
/// coincide with another are joined to it; fewer than three points, or all
/// on one line, form a chain.
pub fn delaunay(xy: &[[f64; 2]]) -> Vec<Vec<usize>> {
    let n = xy.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    if n < 2 {
        return adj;
    }
    let points: Vec<delaunator::Point> = xy
        .iter()
        .map(|p| delaunator::Point { x: p[0], y: p[1] })
        .collect();
    let t = delaunator::triangulate(&points);
    if t.triangles.is_empty() {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            xy[a][0]
                .total_cmp(&xy[b][0])
                .then(xy[a][1].total_cmp(&xy[b][1]))
        });
        for w in order.windows(2) {
            adj[w[0]].push(w[1]);
            adj[w[1]].push(w[0]);
        }
    } else {
        for tri in t.triangles.chunks(3) {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                adj[a].push(b);
                adj[b].push(a);
            }
        }
        // Duplicate points are left out of the triangulation: join each to
        // the point it repeats, or else to its nearest point.
        let mut first: HashMap<(u64, u64), usize> = HashMap::new();
        for (i, p) in xy.iter().enumerate() {
            first.entry((p[0].to_bits(), p[1].to_bits())).or_insert(i);
        }
        for i in 0..n {
            if !adj[i].is_empty() {
                continue;
            }
            let twin = first[&(xy[i][0].to_bits(), xy[i][1].to_bits())];
            let j = if twin != i && !adj[twin].is_empty() {
                twin
            } else {
                (0..n)
                    .filter(|&j| j != i && !adj[j].is_empty())
                    .min_by(|&a, &b| dist(xy[i], xy[a]).total_cmp(&dist(xy[i], xy[b])))
                    .unwrap_or(if i == 0 { 1 } else { 0 })
            };
            adj[i].push(j);
            adj[j].push(i);
        }
    }
    for list in &mut adj {
        list.sort_unstable();
        list.dedup();
    }
    adj
}

/// Edges (a < b) of an adjacency list.
pub fn edges(adj: &[Vec<usize>]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (a, list) in adj.iter().enumerate() {
        for &b in list {
            if a < b {
                out.push((a, b));
            }
        }
    }
    out
}

/// The Euclidean minimum spanning tree, which always lies on the Delaunay
/// triangulation (Kruskal over its edges).
pub fn euclidean_mst(xy: &[[f64; 2]], adj: &[Vec<usize>]) -> Vec<(usize, usize)> {
    let mut e = edges(adj);
    e.sort_by(|&(a, b), &(c, d)| {
        dist(xy[a], xy[b])
            .total_cmp(&dist(xy[c], xy[d]))
            .then((a, b).cmp(&(c, d)))
    });
    let mut uf = UnionFind::new(xy.len());
    e.into_iter().filter(|&(a, b)| uf.union(a, b)).collect()
}

/// Every pair (a < b) within `max_hops` steps along `adj`, with its hop
/// count.
pub fn pairs_within(adj: &[Vec<usize>], max_hops: u8) -> Vec<(u32, u32, u8)> {
    let n = adj.len();
    let mut out = Vec::new();
    let mut depth = vec![u8::MAX; n];
    let mut seen = Vec::new();
    let mut queue = VecDeque::new();
    for s in 0..n {
        depth[s] = 0;
        seen.push(s);
        queue.push_back(s);
        while let Some(u) = queue.pop_front() {
            if depth[u] == max_hops {
                continue;
            }
            for &v in &adj[u] {
                if depth[v] == u8::MAX {
                    depth[v] = depth[u] + 1;
                    seen.push(v);
                    queue.push_back(v);
                    if v > s {
                        out.push((s as u32, v as u32, depth[v]));
                    }
                }
            }
        }
        for &v in &seen {
            depth[v] = u8::MAX;
        }
        seen.clear();
    }
    out
}

/// Hops between a and b along `adj`, searching at most `limit` steps.
pub fn hops(adj: &[Vec<usize>], a: usize, b: usize, limit: u8) -> Option<u8> {
    if a == b {
        return Some(0);
    }
    let mut depth: HashMap<usize, u8> = HashMap::from([(a, 0)]);
    let mut queue = VecDeque::from([a]);
    while let Some(u) = queue.pop_front() {
        let d = depth[&u];
        if d == limit {
            continue;
        }
        for &v in &adj[u] {
            if let std::collections::hash_map::Entry::Vacant(e) = depth.entry(v) {
                if v == b {
                    return Some(d + 1);
                }
                e.insert(d + 1);
                queue.push_back(v);
            }
        }
    }
    None
}

/// Biconnected blocks of a multigraph on `n` vertices: for each vertex, the
/// blocks it belongs to (none when it has no edges, several when it is a
/// cut vertex). Two vertices stay connected after any single other vertex
/// is removed exactly when they share a block.
pub fn blocks(n: usize, edge_list: &[(usize, usize)]) -> Vec<Vec<u32>> {
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (id, &(a, b)) in edge_list.iter().enumerate() {
        if a != b {
            adj[a].push((b, id));
            adj[b].push((a, id));
        }
    }
    let mut disc = vec![usize::MAX; n];
    let mut low = vec![0usize; n];
    let mut member: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut timer = 0;
    let mut block = 0u32;
    let mut edge_stack: Vec<(usize, usize)> = Vec::new();
    // (vertex, edge used to reach it, next neighbour index)
    let mut stack: Vec<(usize, usize, usize)> = Vec::new();
    for root in 0..n {
        if disc[root] != usize::MAX || adj[root].is_empty() {
            continue;
        }
        disc[root] = timer;
        low[root] = timer;
        timer += 1;
        stack.push((root, usize::MAX, 0));
        while let Some(top) = stack.len().checked_sub(1) {
            let (u, via, next) = stack[top];
            if next < adj[u].len() {
                stack[top].2 += 1;
                let (v, id) = adj[u][next];
                if id == via {
                    continue;
                }
                if disc[v] == usize::MAX {
                    disc[v] = timer;
                    low[v] = timer;
                    timer += 1;
                    edge_stack.push((u, v));
                    stack.push((v, id, 0));
                } else if disc[v] < disc[u] {
                    low[u] = low[u].min(disc[v]);
                    edge_stack.push((u, v));
                }
            } else {
                stack.pop();
                if let Some(&(p, _, _)) = stack.last() {
                    low[p] = low[p].min(low[u]);
                    if low[u] >= disc[p] {
                        // p separates u's subtree: pop that block.
                        while let Some((a, b)) = edge_stack.pop() {
                            for x in [a, b] {
                                if member[x].last() != Some(&block) {
                                    member[x].push(block);
                                }
                            }
                            if (a, b) == (p, u) {
                                break;
                            }
                        }
                        block += 1;
                    }
                }
            }
        }
    }
    member
}

/// Whether a and b share a block (see [`blocks`]).
pub fn share_block(member: &[Vec<u32>], a: usize, b: usize) -> bool {
    let (x, y) = (&member[a], &member[b]);
    x.iter().any(|blk| y.contains(blk))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_with_centre() {
        let xy = [[0., 0.], [1., 0.], [1., 1.], [0., 1.], [0.5, 0.45]];
        let adj = delaunay(&xy);
        assert_eq!(edges(&adj).len(), 8);
        let mst = euclidean_mst(&xy, &adj);
        assert_eq!(mst.len(), 4);
        assert!(mst.iter().all(|&(a, b)| a == 4 || b == 4));
        let pairs = pairs_within(&adj, 3);
        assert_eq!(pairs.len(), 10);
        assert_eq!(hops(&adj, 0, 2, 3), Some(2));
    }

    #[test]
    fn duplicates_and_lines_stay_connected() {
        let adj = delaunay(&[[0., 0.], [1., 0.], [1., 0.], [0., 1.]]);
        assert!(adj.iter().all(|l| !l.is_empty()));
        let line = delaunay(&[[0., 0.], [2., 0.], [1., 0.]]);
        assert_eq!(line[2], vec![0, 1]);
    }

    #[test]
    fn blocks_find_cut_vertices() {
        // Two triangles joined at vertex 2, plus a pendant 5 on 4.
        let e = [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 2), (4, 5)];
        let m = blocks(7, &e);
        assert_eq!(m[2].len(), 2);
        assert_eq!(m[4].len(), 2);
        assert!(m[6].is_empty());
        assert!(share_block(&m, 0, 1));
        assert!(!share_block(&m, 0, 3));
        assert!(!share_block(&m, 3, 5));
        // A parallel pair is one block.
        let p = blocks(2, &[(0, 1), (0, 1)]);
        assert!(share_block(&p, 0, 1));
    }
}
