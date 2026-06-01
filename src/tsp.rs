use std::collections::HashMap;
use std::collections::HashSet;
use petgraph::graph::NodeIndex;
use petgraph::Graph;

/// Solve Metric TSP using Christofides + 2-Opt refinement.
///
/// The graph must be a complete graph with edge weights stored as f64,
/// satisfying the triangle inequality (Metric TSP).
/// Returns a Hamiltonian cycle as a Vec of node indices (last == first).
pub fn solve_christofides_2opt_tsp(
    grph: &Graph<String, f64, petgraph::Undirected>,
) -> Vec<usize> {
    let initial = christofides_tsp(grph);
    two_opt(&initial, grph)
}

/// Christofides 3/2-approximation algorithm for Metric TSP.
///
/// 1. MST of the graph
/// 2. Odd-degree vertices in MST
/// 3. Minimum weight perfect matching on odd vertices
/// 4. Combine into Eulerian multigraph
/// 5. Eulerian circuit
/// 6. Shortcut to Hamiltonian cycle
pub fn christofides_tsp(
    grph: &Graph<String, f64, petgraph::Undirected>,
) -> Vec<usize> {
    let n = grph.node_count();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0, 0];
    }

    // 1. Minimum Spanning Tree
    let mst_edges = mst(grph);

    // 2. Odd-degree nodes in the MST
    let mut degree = vec![0u32; n];
    let mut multigraph_edges: Vec<(usize, usize)> = Vec::new();
    for &(u, v) in &mst_edges {
        degree[u] += 1;
        degree[v] += 1;
        multigraph_edges.push((u, v));
    }

    let odd_nodes: Vec<usize> = degree
        .iter()
        .enumerate()
        .filter(|(_, &d)| d % 2 != 0)
        .map(|(i, _)| i)
        .collect();

    // 3. Minimum weight perfect matching on odd vertices
    if odd_nodes.len() >= 2 {
        let matching = min_weight_perfect_matching_edge(grph, &odd_nodes);
        for &(a, b) in &matching {
            multigraph_edges.push((a, b));
        }
    }

    // 4-5. Eulerian circuit via Hierholzer
    let circuit = hierholzer_eulerian(n, &multigraph_edges);

    // 6. Shortcut to Hamiltonian
    shortcut_eulerian(&circuit)
}

/// Minimum Spanning Tree via simple Prim's algorithm.
fn mst(grph: &Graph<String, f64, petgraph::Undirected>) -> Vec<(usize, usize)> {
    let n = grph.node_count();
    if n <= 1 {
        return Vec::new();
    }

    let mut in_tree = vec![false; n];
    let mut key = vec![f64::INFINITY; n];
    let mut parent = vec![n; n];
    let mut result = Vec::new();

    key[0] = 0.0;

    for _ in 0..n {
        // Find minimum key vertex not in tree
        let mut u = n;
        let mut min_key = f64::INFINITY;
        for v in 0..n {
            if !in_tree[v] && key[v] < min_key {
                min_key = key[v];
                u = v;
            }
        }
        if u == n {
            break;
        }
        in_tree[u] = true;
        if parent[u] != n {
            result.push((parent[u], u));
        }

        // Update neighbors
        for edge_idx in grph.edge_indices() {
            let (a, b) = grph.edge_endpoints(edge_idx).unwrap();
            let ai: usize = a.index();
            let bi: usize = b.index();
            let w = grph[edge_idx];

            let (v, other) = if ai == u { (bi, ai) } else if bi == u { (ai, bi) } else { continue; };

            if !in_tree[v] && w < key[v] {
                key[v] = w;
                parent[v] = other;
            }
        }
    }

    result
}

/// Minimum weight perfect matching on a complete subgraph (odd vertices).
/// Uses DP over subsets (O(k * 2^k) for k odd vertices).
fn min_weight_perfect_matching_edge(
    grph: &Graph<String, f64, petgraph::Undirected>,
    odd_nodes: &[usize],
) -> Vec<(usize, usize)> {
    let k = odd_nodes.len();
    if k < 2 {
        return Vec::new();
    }

    // Build distance matrix between odd nodes
    let mut dist = vec![vec![f64::INFINITY; k]; k];
    for i in 0..k {
        dist[i][i] = 0.0;
        for j in (i + 1)..k {
            let d = euclidean_distance(grph, odd_nodes[i], odd_nodes[j]);
            dist[i][j] = d;
            dist[j][i] = d;
        }
    }

    // DP over subsets
    let size = 1 << k;
    let mut dp = vec![f64::INFINITY; size];
    let mut choice = vec![k; size];
    dp[0] = 0.0;

    for mask in 0..size {
        if dp[mask] == f64::INFINITY {
            continue;
        }
        let mut i = 0;
        while i < k && (mask & (1 << i)) != 0 {
            i += 1;
        }
        if i >= k {
            continue;
        }
        for (j, _) in dist.iter().enumerate().take(k).skip(i + 1) {
            if (mask & (1 << j)) == 0 {
                let new_mask = mask | (1 << i) | (1 << j);
                let new_cost = dp[mask] + dist[i][j];
                if new_cost < dp[new_mask] {
                    dp[new_mask] = new_cost;
                    choice[new_mask] = j;
                }
            }
        }
    }

    // Reconstruct
    let mut matching = Vec::new();
    let mut mask = size - 1;
    while mask != 0 {
        let mut i = 0;
        while i < k && (mask & (1 << i)) == 0 {
            i += 1;
        }
        if i >= k {
            break;
        }
        let j = choice[mask];
        if j < k {
            matching.push((odd_nodes[i], odd_nodes[j]));
            mask &= !(1 << i);
            mask &= !(1 << j);
        } else {
            break;
        }
    }

    matching
}

/// Get Euclidean distance between two nodes by looking up edge weight.
fn euclidean_distance(
    grph: &Graph<String, f64, petgraph::Undirected>,
    u: usize,
    v: usize,
) -> f64 {
    let ui = NodeIndex::new(u);
    let vi = NodeIndex::new(v);
    if let Some(edge_idx) = grph.find_edge(ui, vi) {
        grph[edge_idx]
    } else {
        // Fallback: should not happen for complete graphs
        f64::INFINITY
    }
}

/// Hierholzer's algorithm for Eulerian circuit in a multigraph.
///
/// `n` = number of vertices, `edges` = list of (u, v) edges.
/// Returns a list of vertex indices forming an Eulerian circuit.
fn hierholzer_eulerian(_n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    if edges.is_empty() {
        return Vec::new();
    }

    // Build adjacency list with edge tracking
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();

    for &(u, v) in edges {
        adj.entry(u).or_default().push(v);
        adj.entry(v).or_default().push(u);
        *edge_count.entry(if u < v { (u, v) } else { (v, u) }).or_insert(0) += 1;
    }

    // Find start vertex (the minimum vertex with edges, for determinism)
    let start = *adj.keys().min().unwrap_or(&0);

    let mut stack = vec![start];
    let mut circuit: Vec<usize> = Vec::new();

    // Clone edge counts for mutability
    let mut remaining: HashMap<(usize, usize), usize> = edge_count;

    while let Some(&v) = stack.last() {
        if let Some(neighbors) = adj.get(&v) {
            let unvisited = neighbors.iter().find(|&&u| {
                let key = if v < u { (v, u) } else { (u, v) };
                *remaining.get(&key).unwrap_or(&0) > 0
            });

            if let Some(&u) = unvisited {
                let key = if v < u { (v, u) } else { (u, v) };
                *remaining.get_mut(&key).unwrap_or(&mut 0) -= 1;
                stack.push(u);
            } else {
                circuit.push(v);
                stack.pop();
            }
        } else {
            circuit.push(v);
            stack.pop();
        }
    }

    circuit.reverse();
    circuit
}

/// Shortcut Eulerian circuit to Hamiltonian cycle by skipping repeated vertices.
fn shortcut_eulerian(circuit: &[usize]) -> Vec<usize> {
    let mut path: Vec<usize> = Vec::new();
    let mut visited: HashSet<usize> = HashSet::new();

    for &v in circuit {
        if !visited.contains(&v) {
            path.push(v);
            visited.insert(v);
        }
    }

    if !path.is_empty() {
        path.push(path[0]); // close the loop
    }

    path
}

/// 2-opt local search heuristic to refine a TSP tour.
pub fn two_opt(path: &[usize], grph: &Graph<String, f64, petgraph::Undirected>) -> Vec<usize> {
    let mut best_path = path.to_vec();
    let mut improved = true;

    while improved {
        improved = false;
        for i in 1..best_path.len().saturating_sub(2) {
            for j in (i + 1)..best_path.len() {
                if j - i == 1 {
                    continue;
                }
                // Reverse segment [i, j-1]
                let mut new_path = best_path[..i].to_vec();
                new_path.extend(best_path[i..j].iter().rev());
                new_path.extend_from_slice(&best_path[j..]);

                if total_distance(&new_path, grph) < total_distance(&best_path, grph) {
                    best_path = new_path;
                    improved = true;
                }
            }
        }
    }

    best_path
}

/// Calculate total distance of a Hamiltonian path/cycle.
pub fn total_distance(path: &[usize], grph: &Graph<String, f64, petgraph::Undirected>) -> f64 {
    let mut dist = 0.0;
    for i in 0..path.len().saturating_sub(1) {
        let u = NodeIndex::new(path[i]);
        let v = NodeIndex::new(path[i + 1]);
        if let Some(edge_idx) = grph.find_edge(u, v) {
            dist += grph[edge_idx];
        }
    }
    dist
}

/// Create a complete graph with random Euclidean (L2) edge weights.
pub fn make_l2_graph(n: usize, seed: u64) -> (Graph<String, f64, petgraph::Undirected>, Vec<(f64, f64)>) {
    let mut rng = SimpleRng::new(seed);
    let positions: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.next_f64() * 100.0, rng.next_f64() * 100.0))
        .collect();

    let mut grph = Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let indices: Vec<NodeIndex> = (0..n)
        .map(|i| grph.add_node(format!("n{}", i)))
        .collect();

    for i in 0..n {
        for j in (i + 1)..n {
            let dx = positions[i].0 - positions[j].0;
            let dy = positions[i].1 - positions[j].1;
            let dist = (dx * dx + dy * dy).sqrt();
            grph.add_edge(indices[i], indices[j], dist);
        }
    }

    (grph, positions)
}

/// Create a complete graph with random Manhattan (L1) edge weights.
pub fn make_l1_graph(n: usize, seed: u64) -> (Graph<String, f64, petgraph::Undirected>, Vec<(f64, f64)>) {
    let mut rng = SimpleRng::new(seed);
    let positions: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.next_f64() * 100.0, rng.next_f64() * 100.0))
        .collect();

    let mut grph = Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let indices: Vec<NodeIndex> = (0..n)
        .map(|i| grph.add_node(format!("n{}", i)))
        .collect();

    for i in 0..n {
        for j in (i + 1)..n {
            let dx = (positions[i].0 - positions[j].0).abs();
            let dy = (positions[i].1 - positions[j].1).abs();
            let dist = dx + dy;
            grph.add_edge(indices[i], indices[j], dist);
        }
    }

    (grph, positions)
}

/// Simple LCG random number generator.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 1 } else { seed } }
    }

    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mst_simple() {
        let mut grph = Graph::<String, f64, petgraph::Undirected>::new_undirected();
        let n0 = grph.add_node("n0".to_string());
        let n1 = grph.add_node("n1".to_string());
        let n2 = grph.add_node("n2".to_string());
        grph.add_edge(n0, n1, 1.0);
        grph.add_edge(n1, n2, 2.0);
        grph.add_edge(n0, n2, 3.0);

        let mst_edges = mst(&grph);
        assert_eq!(mst_edges.len(), 2);
    }

    #[test]
    fn test_christofides_small() {
        let n = 5;
        let (grph, _pos) = make_l2_graph(n, 42);
        let tour = christofides_tsp(&grph);

        assert_eq!(tour.len(), n + 1); // n nodes + return
        assert_eq!(tour[0], tour[tour.len() - 1]); // closed loop

        // Check all vertices visited exactly once (except start/end)
        let mut visited: HashSet<usize> = HashSet::new();
        for &v in &tour[..tour.len() - 1] {
            assert!(visited.insert(v), "Vertex {} visited twice", v);
        }
        assert_eq!(visited.len(), n);
    }

    #[test]
    fn test_christofides_2opt() {
        let n = 6;
        let (grph, _pos) = make_l2_graph(n, 42);
        let tour = solve_christofides_2opt_tsp(&grph);

        assert_eq!(tour.len(), n + 1);
        assert_eq!(tour[0], tour[tour.len() - 1]);

        let mut visited: HashSet<usize> = HashSet::new();
        for &v in &tour[..tour.len() - 1] {
            assert!(visited.insert(v), "Vertex {} visited twice", v);
        }
        assert_eq!(visited.len(), n);
    }

    #[test]
    fn test_total_distance() {
        let mut grph = Graph::<String, f64, petgraph::Undirected>::new_undirected();
        let n0 = grph.add_node("n0".to_string());
        let n1 = grph.add_node("n1".to_string());
        let n2 = grph.add_node("n2".to_string());
        grph.add_edge(n0, n1, 1.0);
        grph.add_edge(n1, n2, 2.0);
        grph.add_edge(n0, n2, 3.0);

        let dist = total_distance(&[0, 1, 2, 0], &grph);
        assert!((dist - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_two_opt_simple() {
        let n = 4;
        let (grph, _pos) = make_l2_graph(n, 42);
        let tour = two_opt(&[0, 1, 2, 3, 0], &grph);
        assert_eq!(tour.len(), 5);
        let d = total_distance(&tour, &grph);
        assert!(d > 0.0);
    }

    #[test]
    fn test_make_l2_graph() {
        let (grph, pos) = make_l2_graph(5, 42);
        assert_eq!(grph.node_count(), 5);
        assert_eq!(grph.edge_count(), 10); // complete graph
        assert_eq!(pos.len(), 5);
    }

    #[test]
    fn test_make_l1_graph() {
        let (grph, pos) = make_l1_graph(5, 42);
        assert_eq!(grph.node_count(), 5);
        assert_eq!(grph.edge_count(), 10);
        assert_eq!(pos.len(), 5);
    }
}
