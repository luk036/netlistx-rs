//! Hadlock's algorithm for MAX-CUT on planar graphs.
//!
//! Given a planar graph $G = (V, E)$ with edge weights $w: E \to \mathbb{R}^+$,
//! find a partition $(S, V \setminus S)$ maximizing the total weight of cut edges
//!
//! $$ \max_{S \subseteq V} \sum_{\substack{(u,v) \in E \\ u \in S,\, v \notin S}} w(u,v) $$
//!
//! Hadlock's reduction: a set of primal edges is a cut iff the corresponding
//! dual edges form a $T$-join of the planar dual, where $T$ is the set of
//! *odd faces* (faces with an odd number of boundary edges). The minimum weight
//! $T$-join is computed as a minimum weight perfect matching on the complete
//! graph over $T$ whose weights are shortest-path distances in the dual.
//!
//! The graph is first decomposed into biconnected components (blocks): MAX-CUT
//! is additive over blocks, each block has a connected dual, and within a block
//! the odd-degree dual vertices coincide with the odd-length faces.

use crate::planar::planar_faces;
use mwmatching::{Matching, SENTINEL};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

type NetGraph = Graph<String, f64, Undirected>;

/// Solve MAX-CUT for a planar graph using Hadlock's algorithm.
///
/// Returns the set of edge keys `"u--v"` (with the endpoint names sorted) that
/// belong to the maximum cut.
///
/// # Panics
///
/// Panics if `grph` is not planar. Use [`try_solve_hadlock_max_cut`] for a
/// fallible variant.
pub fn solve_hadlock_max_cut(grph: &NetGraph) -> HashSet<String> {
    try_solve_hadlock_max_cut(grph).expect("Hadlock MAX-CUT requires a planar graph")
}

/// Fallible variant of [`solve_hadlock_max_cut`]; returns `None` when `grph`
/// is not planar.
pub fn try_solve_hadlock_max_cut(grph: &NetGraph) -> Option<HashSet<String>> {
    if grph.node_count() == 0 {
        return Some(HashSet::new());
    }
    let mut cut_edges: HashSet<String> = HashSet::new();
    for block in biconnected_components(grph) {
        let comp = subgraph(grph, &block);
        cut_edges.extend(solve_hadlock_component(&comp)?);
    }
    Some(cut_edges)
}

fn solve_hadlock_component(grph: &NetGraph) -> Option<HashSet<String>> {
    let faces = planar_faces(grph)?;
    if faces.is_empty() {
        return Some(HashSet::new());
    }

    let odd_faces: Vec<usize> = faces
        .iter()
        .enumerate()
        .filter(|(_, face)| face.len() % 2 == 1)
        .map(|(i, _)| i)
        .collect();

    if odd_faces.len() < 2 {
        return Some(all_edges(grph));
    }

    let weights = weight_map(grph);
    let dual = build_dual(&faces, &weights);

    let k = odd_faces.len();
    let mut dist = vec![vec![f64::INFINITY; k]; k];
    let mut predecessors: Vec<Vec<usize>> = Vec::with_capacity(k);
    let mut predecessor_edges: Vec<Vec<(NodeIndex, NodeIndex)>> = Vec::with_capacity(k);

    for (i, &src) in odd_faces.iter().enumerate() {
        let (d, prev, prev_edge) = dijkstra(&dual, src);
        for (j, &dst) in odd_faces.iter().enumerate() {
            dist[i][j] = d[dst];
        }
        predecessors.push(prev);
        predecessor_edges.push(prev_edge);
    }

    let matching = min_weight_perfect_matching(&dist, k);

    let mut excluded: HashSet<String> = HashSet::new();
    for &(i, j) in &matching {
        let src = odd_faces[i];
        let dst = odd_faces[j];
        let mut cur = dst;
        while cur != src && predecessors[i][cur] != usize::MAX {
            let (a, b) = predecessor_edges[i][cur];
            excluded.insert(edge_key(&grph[a], &grph[b]));
            cur = predecessors[i][cur];
        }
    }

    let mut result = all_edges(grph);
    for e in &excluded {
        result.remove(e);
    }
    Some(result)
}

fn all_edges(grph: &NetGraph) -> HashSet<String> {
    grph.edge_references()
        .map(|e| edge_key(&grph[e.source()], &grph[e.target()]))
        .collect()
}

fn edge_key(u: &str, v: &str) -> String {
    if u <= v {
        format!("{}--{}", u, v)
    } else {
        format!("{}--{}", v, u)
    }
}

fn normalize_edge_key(key: &str) -> String {
    match key.split_once("--") {
        Some((u, v)) => edge_key(u, v),
        None => key.to_string(),
    }
}

fn weight_map(grph: &NetGraph) -> HashMap<(NodeIndex, NodeIndex), f64> {
    let mut map: HashMap<(NodeIndex, NodeIndex), f64> = HashMap::new();
    for e in grph.edge_references() {
        let a = e.source();
        let b = e.target();
        let key = if a <= b { (a, b) } else { (b, a) };
        let w = *e.weight();
        map.entry(key)
            .and_modify(|existing| *existing = existing.min(w))
            .or_insert(w);
    }
    map
}

struct DualEdge {
    to: usize,
    w: f64,
    primal: (NodeIndex, NodeIndex),
}

fn build_dual(
    faces: &[Vec<NodeIndex>],
    weights: &HashMap<(NodeIndex, NodeIndex), f64>,
) -> Vec<Vec<DualEdge>> {
    let mut edge_faces: HashMap<(NodeIndex, NodeIndex), Vec<usize>> = HashMap::new();
    for (fi, face) in faces.iter().enumerate() {
        let m = face.len();
        for i in 0..m {
            let u = face[i];
            let v = face[(i + 1) % m];
            if u == v {
                continue;
            }
            let key = if u <= v { (u, v) } else { (v, u) };
            edge_faces.entry(key).or_default().push(fi);
        }
    }

    let mut dual: Vec<Vec<DualEdge>> = (0..faces.len()).map(|_| Vec::new()).collect();
    for (key, face_ids) in &edge_faces {
        if face_ids.len() < 2 {
            continue;
        }
        let w = *weights.get(key).unwrap_or(&1.0);
        for a in 0..face_ids.len() {
            for b in (a + 1)..face_ids.len() {
                let fi = face_ids[a];
                let fj = face_ids[b];
                if fi == fj {
                    continue;
                }
                add_dual_edge(&mut dual, fi, fj, w, *key);
                add_dual_edge(&mut dual, fj, fi, w, *key);
            }
        }
    }
    dual
}

fn add_dual_edge(
    dual: &mut [Vec<DualEdge>],
    from: usize,
    to: usize,
    w: f64,
    primal: (NodeIndex, NodeIndex),
) {
    if let Some(existing) = dual[from].iter_mut().find(|e| e.to == to) {
        if w < existing.w {
            existing.w = w;
            existing.primal = primal;
        }
    } else {
        dual[from].push(DualEdge { to, w, primal });
    }
}

#[derive(PartialEq)]
struct HeapItem(f64, usize);

impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}

fn dijkstra(
    dual: &[Vec<DualEdge>],
    src: usize,
) -> (Vec<f64>, Vec<usize>, Vec<(NodeIndex, NodeIndex)>) {
    let n = dual.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut prev_edge = vec![(NodeIndex::new(0), NodeIndex::new(0)); n];
    dist[src] = 0.0;
    let mut heap = BinaryHeap::new();
    heap.push(HeapItem(0.0, src));
    while let Some(HeapItem(d, u)) = heap.pop() {
        if d > dist[u] {
            continue;
        }
        for e in &dual[u] {
            let nd = d + e.w;
            if nd < dist[e.to] {
                dist[e.to] = nd;
                prev[e.to] = u;
                prev_edge[e.to] = e.primal;
                heap.push(HeapItem(nd, e.to));
            }
        }
    }
    (dist, prev, prev_edge)
}

/// Minimum weight perfect matching on a complete graph given by `dist`.
///
/// Uses Edmonds' blossom algorithm ($O(k^3)$) via the `mwmatching` crate. The
/// solver takes `i32` weights, so costs are shifted to `C - cost` (which turns
/// minimisation into maximisation while maximum cardinality forces a perfect
/// matching). Integral distances are used exactly; non-integral distances are
/// scaled into `[0, 1e6]`.
fn min_weight_perfect_matching(dist: &[Vec<f64>], k: usize) -> Vec<(usize, usize)> {
    if k < 2 || k % 2 != 0 {
        return Vec::new();
    }

    let mut max_d = 0.0f64;
    for (i, row) in dist.iter().enumerate() {
        for &d in row.iter().skip(i + 1) {
            if d.is_finite() && d > max_d {
                max_d = d;
            }
        }
    }
    if max_d <= 0.0 {
        return (0..k).step_by(2).map(|i| (i, i + 1)).collect();
    }

    let integral = max_d <= 1.0e8
        && dist.iter().all(|row| {
            row.iter()
                .all(|&d| !d.is_finite() || (d - d.round()).abs() < 1e-9)
        });
    let (scale, c_const) = if integral {
        (1.0f64, max_d.round() as i64 + 1)
    } else {
        let s = 1.0e6 / max_d;
        (s, (max_d * s).round() as i64 + 1)
    };

    let mut edges: Vec<(usize, usize, i32)> = Vec::with_capacity(k * (k - 1) / 2);
    for (i, row) in dist.iter().enumerate() {
        for (j, &d) in row.iter().enumerate().skip(i + 1) {
            if !d.is_finite() {
                continue;
            }
            let cost = (d * scale).round() as i64;
            let weight = (c_const - cost).clamp(0, i32::MAX as i64) as i32;
            edges.push((i, j, weight));
        }
    }
    if edges.is_empty() {
        return Vec::new();
    }

    let mates = Matching::new(edges).max_cardinality().solve();
    let mut matching = Vec::with_capacity(k / 2);
    for (i, &mate) in mates.iter().enumerate() {
        if i < mate && mate != SENTINEL {
            matching.push((i, mate));
        }
    }
    matching
}

struct Bcc<'a> {
    grph: &'a NetGraph,
    disc: Vec<usize>,
    low: Vec<usize>,
    stack: Vec<(NodeIndex, NodeIndex)>,
    blocks: Vec<Vec<(NodeIndex, NodeIndex)>>,
    timer: usize,
}

impl Bcc<'_> {
    fn dfs(&mut self, u: NodeIndex, parent: usize) {
        self.timer += 1;
        self.disc[u.index()] = self.timer;
        self.low[u.index()] = self.timer;
        let neighbors: Vec<NodeIndex> = self.grph.neighbors(u).collect();
        for v in neighbors {
            if v.index() == parent {
                continue;
            }
            if self.disc[v.index()] == usize::MAX {
                self.stack.push((u, v));
                self.dfs(v, u.index());
                self.low[u.index()] = self.low[u.index()].min(self.low[v.index()]);
                if self.low[v.index()] >= self.disc[u.index()] {
                    let mut block = Vec::new();
                    while let Some(top) = self.stack.pop() {
                        block.push(top);
                        if top == (u, v) {
                            break;
                        }
                    }
                    if !block.is_empty() {
                        self.blocks.push(block);
                    }
                }
            } else if self.disc[v.index()] < self.disc[u.index()] {
                self.stack.push((u, v));
                self.low[u.index()] = self.low[u.index()].min(self.disc[v.index()]);
            }
        }
    }
}

/// Biconnected components (blocks) as edge lists.
fn biconnected_components(grph: &NetGraph) -> Vec<Vec<(NodeIndex, NodeIndex)>> {
    let n = grph.node_count();
    let mut state = Bcc {
        grph,
        disc: vec![usize::MAX; n],
        low: vec![0; n],
        stack: Vec::new(),
        blocks: Vec::new(),
        timer: 0,
    };
    for root in grph.node_indices() {
        if state.disc[root.index()] != usize::MAX {
            continue;
        }
        state.dfs(root, usize::MAX);
        if !state.stack.is_empty() {
            let block: Vec<(NodeIndex, NodeIndex)> = state.stack.drain(..).collect();
            state.blocks.push(block);
        }
    }
    state.blocks
}

fn subgraph(grph: &NetGraph, edges: &[(NodeIndex, NodeIndex)]) -> NetGraph {
    let mut sub: NetGraph = Graph::new_undirected();
    let mut map: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    for &(u, v) in edges {
        let su = *map
            .entry(u)
            .or_insert_with(|| sub.add_node(grph[u].clone()));
        let sv = *map
            .entry(v)
            .or_insert_with(|| sub.add_node(grph[v].clone()));
        let w = grph.find_edge(u, v).map(|e| grph[e]).unwrap_or(1.0);
        sub.add_edge(su, sv, w);
    }
    sub
}

/// Validate that `cut_edges` forms a valid bipartite cut of `grph`.
///
/// Returns `(is_bipartite, total_cut_weight)`.
pub fn validate_max_cut(grph: &NetGraph, cut_edges: &HashSet<String>) -> (bool, f64) {
    let weights: HashMap<String, f64> = grph
        .edge_references()
        .map(|e| (edge_key(&grph[e.source()], &grph[e.target()]), *e.weight()))
        .collect();
    let normalized: HashSet<String> = cut_edges.iter().map(|k| normalize_edge_key(k)).collect();

    let mut cut: NetGraph = Graph::new_undirected();
    let mut map: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    for e in grph.edge_references() {
        let key = edge_key(&grph[e.source()], &grph[e.target()]);
        if !normalized.contains(&key) {
            continue;
        }
        let (a, b) = (e.source(), e.target());
        let ca = *map
            .entry(a)
            .or_insert_with(|| cut.add_node(grph[a].clone()));
        let cb = *map
            .entry(b)
            .or_insert_with(|| cut.add_node(grph[b].clone()));
        cut.add_edge(ca, cb, *e.weight());
    }

    let mut color: HashMap<NodeIndex, bool> = HashMap::new();
    let mut is_bipartite = true;
    'outer: for start in cut.node_indices() {
        if color.contains_key(&start) {
            continue;
        }
        color.insert(start, true);
        let mut queue = VecDeque::new();
        queue.push_back(start);
        while let Some(u) = queue.pop_front() {
            let cu = color[&u];
            for v in cut.neighbors(u) {
                match color.get(&v) {
                    Some(&cv) => {
                        if cv == cu {
                            is_bipartite = false;
                            break 'outer;
                        }
                    }
                    None => {
                        color.insert(v, !cu);
                        queue.push_back(v);
                    }
                }
            }
        }
    }

    let cut_weight: f64 = cut_edges
        .iter()
        .map(|key| {
            weights
                .get(&normalize_edge_key(key))
                .copied()
                .unwrap_or(1.0)
        })
        .sum();

    (is_bipartite, cut_weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_with(edges: &[(&str, &str, f64)]) -> NetGraph {
        let mut grph: NetGraph = Graph::new_undirected();
        let mut map: HashMap<String, NodeIndex> = HashMap::new();
        for &(u, v, w) in edges {
            let nu = *map
                .entry(u.to_string())
                .or_insert_with(|| grph.add_node(u.to_string()));
            let nv = *map
                .entry(v.to_string())
                .or_insert_with(|| grph.add_node(v.to_string()));
            grph.add_edge(nu, nv, w);
        }
        grph
    }

    fn triangle() -> NetGraph {
        graph_with(&[("a", "b", 5.0), ("b", "c", 10.0), ("c", "a", 3.0)])
    }

    #[test]
    fn triangle_excludes_min_weight_edge() {
        let grph = triangle();
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 15.0).abs() < 1e-9);
        assert_eq!(cut.len(), 2);
    }

    #[test]
    fn square_is_bipartite() {
        let grph = graph_with(&[
            ("a", "b", 1.0),
            ("b", "c", 1.0),
            ("c", "d", 1.0),
            ("d", "a", 1.0),
        ]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert_eq!(cut.len(), 4);
        assert!((weight - 4.0).abs() < 1e-9);
    }

    #[test]
    fn square_with_diagonal_excludes_the_diagonal() {
        let grph = graph_with(&[
            ("a", "b", 5.0),
            ("b", "c", 10.0),
            ("c", "d", 5.0),
            ("d", "a", 10.0),
            ("a", "c", 2.0),
        ]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 30.0).abs() < 1e-9);
        assert!(!cut.contains("a--c"));
    }

    #[test]
    fn grid_is_bipartite() {
        let mut grph: NetGraph = Graph::new_undirected();
        let nodes: Vec<Vec<NodeIndex>> = (0..3)
            .map(|r| {
                (0..3)
                    .map(|c| grph.add_node(format!("r{}c{}", r, c)))
                    .collect()
            })
            .collect();
        for r in 0..3 {
            for c in 0..3 {
                if c < 2 {
                    grph.add_edge(nodes[r][c], nodes[r][c + 1], 1.0);
                }
                if r < 2 {
                    grph.add_edge(nodes[r][c], nodes[r + 1][c], 1.0);
                }
            }
        }
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert_eq!(cut.len(), grph.edge_count());
        assert!((weight - 12.0).abs() < 1e-9);
    }

    #[test]
    fn empty_graph_has_empty_cut() {
        let grph: NetGraph = Graph::new_undirected();
        assert!(solve_hadlock_max_cut(&grph).is_empty());
    }

    #[test]
    fn single_edge_is_entirely_in_the_cut() {
        let grph = graph_with(&[("a", "b", 7.0)]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 7.0).abs() < 1e-9);
    }

    #[test]
    fn default_weight_one_triangle() {
        let grph = graph_with(&[("a", "b", 1.0), ("b", "c", 1.0), ("c", "a", 1.0)]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 2.0).abs() < 1e-9);
    }

    #[test]
    fn wheel_w4_max_cut_is_six() {
        let grph = graph_with(&[
            ("h", "r1", 1.0),
            ("h", "r2", 1.0),
            ("h", "r3", 1.0),
            ("h", "r4", 1.0),
            ("r1", "r2", 1.0),
            ("r2", "r3", 1.0),
            ("r3", "r4", 1.0),
            ("r4", "r1", 1.0),
        ]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 6.0).abs() < 1e-9);
    }

    #[test]
    fn triangular_prism_max_cut_is_seven() {
        let grph = graph_with(&[
            ("0", "1", 1.0),
            ("1", "2", 1.0),
            ("2", "0", 1.0),
            ("3", "4", 1.0),
            ("4", "5", 1.0),
            ("5", "3", 1.0),
            ("0", "3", 1.0),
            ("1", "4", 1.0),
            ("2", "5", 1.0),
        ]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 7.0).abs() < 1e-9);
    }

    #[test]
    fn bridge_connected_triangles() {
        let grph = graph_with(&[
            ("a", "b", 2.0),
            ("b", "c", 3.0),
            ("c", "a", 4.0),
            ("c", "d", 1.0),
            ("d", "e", 5.0),
            ("e", "f", 6.0),
            ("f", "d", 7.0),
        ]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        // bridge 1 + triangle cuts (9 - 2 = 7) + (18 - 5 = 13) = 21
        assert!((weight - 21.0).abs() < 1e-9);
    }

    #[test]
    fn two_disjoint_triangles() {
        let grph = graph_with(&[
            ("a", "b", 2.0),
            ("b", "c", 3.0),
            ("c", "a", 4.0),
            ("d", "e", 5.0),
            ("e", "f", 6.0),
            ("f", "d", 7.0),
        ]);
        let cut = solve_hadlock_max_cut(&grph);
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 20.0).abs() < 1e-9);
    }

    #[test]
    fn k5_is_rejected() {
        let mut grph: NetGraph = Graph::new_undirected();
        let nodes: Vec<NodeIndex> = (0..5).map(|i| grph.add_node(format!("n{}", i))).collect();
        for i in 0..5 {
            for j in (i + 1)..5 {
                grph.add_edge(nodes[i], nodes[j], 1.0);
            }
        }
        assert!(try_solve_hadlock_max_cut(&grph).is_none());
    }

    #[test]
    fn k3_3_is_rejected() {
        let mut grph: NetGraph = Graph::new_undirected();
        let nodes: Vec<NodeIndex> = (0..6).map(|i| grph.add_node(format!("n{}", i))).collect();
        for i in 0..3 {
            for j in 3..6 {
                grph.add_edge(nodes[i], nodes[j], 1.0);
            }
        }
        assert!(try_solve_hadlock_max_cut(&grph).is_none());
    }

    #[test]
    #[should_panic(expected = "planar")]
    fn non_planar_panics() {
        let mut grph: NetGraph = Graph::new_undirected();
        let nodes: Vec<NodeIndex> = (0..5).map(|i| grph.add_node(format!("n{}", i))).collect();
        for i in 0..5 {
            for j in (i + 1)..5 {
                grph.add_edge(nodes[i], nodes[j], 1.0);
            }
        }
        solve_hadlock_max_cut(&grph);
    }

    #[test]
    fn biconnected_components_of_triangle() {
        let grph = graph_with(&[("a", "b", 1.0), ("b", "c", 1.0), ("c", "a", 1.0)]);
        let blocks = biconnected_components(&grph);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].len(), 3);
    }

    #[test]
    fn biconnected_components_of_bowtie() {
        let grph = graph_with(&[
            ("a", "b", 1.0),
            ("b", "c", 1.0),
            ("c", "a", 1.0),
            ("c", "d", 1.0),
            ("d", "e", 1.0),
            ("e", "c", 1.0),
        ]);
        let blocks = biconnected_components(&grph);
        assert_eq!(blocks.len(), 2);
        assert!(blocks.iter().all(|b| b.len() == 3));
    }

    #[test]
    fn biconnected_components_of_bridge() {
        let grph = graph_with(&[("a", "b", 1.0)]);
        let blocks = biconnected_components(&grph);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].len(), 1);
    }

    #[test]
    fn biconnected_components_empty() {
        let grph: NetGraph = Graph::new_undirected();
        assert!(biconnected_components(&grph).is_empty());
    }

    #[test]
    fn mwpm_pairs_everything_once() {
        let dist = vec![
            vec![0.0, 1.0, 2.0, 3.0],
            vec![1.0, 0.0, 4.0, 5.0],
            vec![2.0, 4.0, 0.0, 6.0],
            vec![3.0, 5.0, 6.0, 0.0],
        ];
        let matching = min_weight_perfect_matching(&dist, 4);
        assert_eq!(matching.len(), 2);
        let mut used = [false; 4];
        for &(i, j) in &matching {
            assert!(!used[i] && !used[j]);
            used[i] = true;
            used[j] = true;
        }
    }

    #[test]
    fn mwpm_rejects_odd_counts() {
        let dist = vec![
            vec![0.0, 1.0, 2.0],
            vec![1.0, 0.0, 3.0],
            vec![2.0, 3.0, 0.0],
        ];
        assert!(min_weight_perfect_matching(&dist, 3).is_empty());
    }

    #[test]
    fn edge_key_is_sorted() {
        assert_eq!(edge_key("a", "b"), "a--b");
        assert_eq!(edge_key("b", "a"), "a--b");
        assert_eq!(edge_key("x", "x"), "x--x");
    }

    #[test]
    fn validate_detects_odd_cycle() {
        let grph = triangle();
        let mut cut = HashSet::new();
        cut.insert("a--b".to_string());
        cut.insert("b--c".to_string());
        cut.insert("a--c".to_string());
        let (valid, _) = validate_max_cut(&grph, &cut);
        assert!(!valid);
    }

    #[test]
    fn validate_accepts_a_valid_cut() {
        let grph = triangle();
        let mut cut = HashSet::new();
        cut.insert("a--b".to_string());
        cut.insert("b--c".to_string());
        let (valid, weight) = validate_max_cut(&grph, &cut);
        assert!(valid);
        assert!((weight - 15.0).abs() < 1e-9);
    }

    #[test]
    fn grid_with_diagonal_matches_reference() {
        // Cross-validated against the Python `netlistx` reference on the same
        // unit-weight construction. The 10x10 case has ~160 odd faces, which the
        // previous exponential bitmask matching could never handle.
        for (m, n, expected) in [(3usize, 3usize, 24.0), (6, 6, 84.0), (10, 10, 220.0)] {
            let mut grph: NetGraph = Graph::new_undirected();
            let mut nodes: HashMap<(usize, usize), NodeIndex> = HashMap::new();
            for r in 0..=m {
                for c in 0..=n {
                    nodes.insert((r, c), grph.add_node(format!("r{}c{}", r, c)));
                }
            }
            for r in 0..=m {
                for c in 0..=n {
                    if c < n {
                        grph.add_edge(nodes[&(r, c)], nodes[&(r, c + 1)], 1.0);
                    }
                    if r < m {
                        grph.add_edge(nodes[&(r, c)], nodes[&(r + 1, c)], 1.0);
                    }
                    if r < m && c < n {
                        grph.add_edge(nodes[&(r, c)], nodes[&(r + 1, c + 1)], 1.0);
                    }
                }
            }
            let cut = solve_hadlock_max_cut(&grph);
            let (valid, weight) = validate_max_cut(&grph, &cut);
            assert!(valid, "m={} n={} produced a non-bipartite cut", m, n);
            assert!(
                (weight - expected).abs() < 1e-9,
                "m={} n={} expected {} got {}",
                m,
                n,
                expected,
                weight
            );
        }
    }
}
