//! Graph Cover Algorithms (ported from Python `cover.py`)
//!
//! Implements primal-dual approximation algorithms with reverse-delete post-processing
//! for various covering problems in graphs:
//! - `min_vertex_cover`: minimum weighted vertex cover
//! - `min_cycle_cover`: minimum weighted set of vertices covering all cycles
//! - `min_odd_cycle_cover`: minimum weighted set of vertices covering all odd cycles

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::ops::Add;
use std::ops::Sub;

/// Phase 1 of the primal-dual algorithm: grow the dual variables (gaps) until
/// every violating set is hit, recording the vertices added to the solution.
fn primal_dual_selection<F, W>(
    violate: &mut F,
    weight: &HashMap<String, W>,
    soln: &mut HashSet<String>,
) -> Vec<String>
where
    F: FnMut(&HashSet<String>) -> Vec<Vec<String>>,
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let mut gap: HashMap<String, W> = HashMap::new();
    let mut added_order: Vec<String> = Vec::new();

    loop {
        let viol_sets = violate(soln);
        if viol_sets.is_empty() {
            break;
        }
        let set = viol_sets.into_iter().next().unwrap();
        if set.is_empty() {
            continue;
        }

        let min_vtx = set
            .iter()
            .min_by(|&v1, &v2| {
                let g1 = gap.get(v1).copied().unwrap_or(weight[v1]);
                let g2 = gap.get(v2).copied().unwrap_or(weight[v2]);
                g1.partial_cmp(&g2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .expect("set should not be empty");

        let min_val = gap.get(&min_vtx).copied().unwrap_or(weight[&min_vtx]);

        if !soln.contains(&min_vtx) {
            soln.insert(min_vtx.clone());
            added_order.push(min_vtx.clone());
        }

        for vtx in &set {
            let entry = gap.entry(vtx.clone()).or_insert(weight[vtx]);
            *entry = *entry - min_val;
        }
    }

    added_order
}

/// Total weight of a solution.
fn primal_cost<W>(weight: &HashMap<String, W>, soln: &HashSet<String>) -> W
where
    W: Copy + Add<Output = W> + Default,
{
    soln.iter()
        .map(|vtx| weight.get(vtx).copied().unwrap_or(W::default()))
        .fold(W::default(), |acc, w| acc + w)
}

/// Generic primal-dual approximation algorithm with reverse-delete post-processing.
///
/// Solves the weighted set cover problem via primal-dual:
///
/// $$ \min \sum_{v \in C} w(v) \quad \text{s.t.} \quad C \cap S \neq \varnothing \; \forall S \in \mathcal{V} $$
///
/// where $\mathcal{V}$ is the set of violating sets. The dual variables (gaps) are
/// increased uniformly for each element until a constraint becomes tight.
///
/// `violate` is a closure that takes the current solution set and returns violating
/// sets (each element in a violating set must have at least one element added to cover).
/// After all violations are resolved, redundant elements are removed via reverse-delete.
pub fn pd_cover<F, W>(
    mut violate: F,
    weight: &HashMap<String, W>,
    soln: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    F: FnMut(&HashSet<String>) -> Vec<Vec<String>>,
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let added_order = primal_dual_selection(&mut violate, weight, soln);

    // Phase 2: Reverse-Delete Post-Processing
    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        let viol_sets = violate(soln);
        let has_violation = viol_sets.iter().any(|s| !s.is_empty());
        if has_violation {
            soln.insert(vtx.clone());
        }
    }

    let final_primal_cost = primal_cost(weight, soln);
    (soln.clone(), final_primal_cost)
}

/// Like [`pd_cover`], but takes a cheap per-vertex redundancy predicate for the
/// reverse-delete phase instead of re-running the violator.
///
/// Removing `vtx` can only expose sets incident to `vtx`, so `redundant` can be
/// an O(deg) local test (e.g. a vertex cover checks only its neighbours).
pub fn pd_cover_with<F, R, W>(
    mut violate: F,
    weight: &HashMap<String, W>,
    soln: &mut HashSet<String>,
    redundant: R,
) -> (HashSet<String>, W)
where
    F: FnMut(&HashSet<String>) -> Vec<Vec<String>>,
    R: Fn(&str, &HashSet<String>) -> bool,
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let added_order = primal_dual_selection(&mut violate, weight, soln);

    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        if !redundant(vtx, soln) {
            soln.insert(vtx.clone());
        }
    }

    let final_primal_cost = primal_cost(weight, soln);
    (soln.clone(), final_primal_cost)
}

/// Minimum weighted vertex cover for a graph using primal-dual with reverse-delete.
///
/// A vertex cover is a set $C \subseteq V$ such that every edge $(u,v) \in E$
/// has at least one endpoint in $C$. Minimizes $\sum_{v \in C} w(v)$.
///
/// Ported from Python `min_vertex_cover()` in `cover.py`.
pub fn min_vertex_cover<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    coverset: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let node_index: HashMap<String, petgraph::graph::NodeIndex> =
        grph.node_indices().map(|i| (grph[i].clone(), i)).collect();

    let violate_fn = |soln: &HashSet<String>| -> Vec<Vec<String>> {
        let mut result = Vec::new();
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            if !soln.contains(u) && !soln.contains(v) {
                result.push(vec![u.clone(), v.clone()]);
            }
        }
        result
    };

    // Removing a vertex can only expose edges incident to it, so redundancy is
    // an O(deg) neighbour check instead of an O(E) rescan of every edge.
    let redundant = |vtx: &str, soln: &HashSet<String>| -> bool {
        match node_index.get(vtx) {
            Some(&idx) => grph.neighbors(idx).all(|n| soln.contains(&grph[n])),
            None => false,
        }
    };

    let mut soln = coverset.clone();
    pd_cover_with(violate_fn, weight, &mut soln, redundant)
}

/// Minimum weighted vertex cover (regular graph) — convenience version.
pub fn min_vertex_cover_new<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let mut coverset = HashSet::new();
    min_vertex_cover(grph, weight, &mut coverset)
}

/// Reconstruct a cycle from BFS parent-child info.
///
/// Ported from Python `_construct_cycle()` in `cover.py`.
fn construct_cycle(
    parent: &HashMap<String, Option<String>>,
    depth: &HashMap<String, usize>,
    start: &str,
    end: &str,
) -> Vec<String> {
    let (node_a, node_b) = if depth.get(start).unwrap_or(&0) < depth.get(end).unwrap_or(&0) {
        (start.to_string(), end.to_string())
    } else {
        (end.to_string(), start.to_string())
    };

    let mut left: VecDeque<String> = VecDeque::new();
    let mut right: VecDeque<String> = VecDeque::new();

    let mut da = *depth.get(&node_a).unwrap_or(&0);
    let mut a = node_a.clone();
    while da > *depth.get(&node_b).unwrap_or(&0) {
        left.push_back(a.clone());
        if let Some(Some(p)) = parent.get(&a) {
            a = p.clone();
            da = *depth.get(&a).unwrap_or(&0);
        } else {
            break;
        }
    }

    let mut b = node_b.clone();
    while a != b {
        left.push_back(a.clone());
        right.push_front(b.clone());
        if let Some(Some(p)) = parent.get(&a) {
            a = p.clone();
        } else {
            break;
        }
        if let Some(Some(p)) = parent.get(&b) {
            b = p.clone();
        } else {
            break;
        }
    }
    left.push_back(a.clone());
    left.extend(right);
    left.into()
}

/// Generic BFS cycle finder.
///
/// Uses BFS to detect cycles in a graph, yielding back edges.
/// Skips nodes in `coverset`.
///
/// Ported from Python `_generic_bfs_cycle()` in `cover.py`.
fn generic_bfs_cycle(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    coverset: &HashSet<String>,
) -> Vec<Vec<String>> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut parent: HashMap<String, Option<String>> = HashMap::new();
    let mut depth: HashMap<String, usize> = HashMap::new();

    for start_idx in grph.node_indices() {
        let source = grph[start_idx].clone();
        if coverset.contains(&source) || visited.contains(&source) {
            continue;
        }

        parent.clear();
        depth.clear();
        // Queue stores node indices so neighbours need no O(V) name lookup.
        let mut queue: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();

        parent.insert(source.clone(), None);
        depth.insert(source.clone(), 0);
        queue.push_back(start_idx);
        visited.insert(source);

        while let Some(current_idx) = queue.pop_front() {
            let current = grph[current_idx].clone();
            let current_depth = depth[&current];

            for neighbor_idx in grph.neighbors(current_idx) {
                let neighbor = grph[neighbor_idx].clone();
                if coverset.contains(&neighbor) {
                    continue;
                }
                if !depth.contains_key(&neighbor) {
                    parent.insert(neighbor.clone(), Some(current.clone()));
                    depth.insert(neighbor.clone(), current_depth + 1);
                    queue.push_back(neighbor_idx);
                    visited.insert(neighbor);
                } else if depth[&neighbor] != current_depth.saturating_sub(1) {
                    // Found a back edge (not the direct parent)
                    let is_direct_parent = parent
                        .get(&current)
                        .and_then(|p| p.as_ref())
                        .map(|p| p == &neighbor)
                        .unwrap_or(false);
                    if !is_direct_parent {
                        let cycle = construct_cycle(&parent, &depth, &current, &neighbor);
                        // Only find one cycle per BFS to avoid duplicates
                        return vec![cycle];
                    }
                }
            }
        }
    }

    Vec::new()
}

/// Minimum weighted set of vertices covering all cycles.
///
/// A cycle cover (feedback vertex set) is a set $C \subseteq V$ such that
/// $G[V \setminus C]$ is acyclic (a forest). Minimizes $\sum_{v \in C} w(v)$.
///
/// Ported from Python `min_cycle_cover()` in `cover.py`.
pub fn min_cycle_cover<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    coverset: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let current_coverset = coverset.clone();
    let violate_fn = |soln: &HashSet<String>| -> Vec<Vec<String>> {
        let cycle = generic_bfs_cycle(grph, soln);
        if cycle.is_empty() {
            vec![]
        } else {
            cycle // returns one cycle at a time
        }
    };

    let mut soln = current_coverset;
    pd_cover(violate_fn, weight, &mut soln)
}

/// Reconstruct the odd cycle closed by a same-colour edge.
///
/// Index-based counterpart of [`construct_cycle`], used by
/// [`min_odd_cycle_cover`]. Working on `NodeIndex` arrays avoids the O(V)
/// node-name lookup that `construct_cycle`'s `HashMap<String, _>` arguments
/// would otherwise require per node.
fn extract_odd_cycle(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    parent: &[Option<petgraph::graph::NodeIndex>],
    depth: &[usize],
    start: petgraph::graph::NodeIndex,
    end: petgraph::graph::NodeIndex,
) -> Vec<String> {
    let (node_a, node_b) = if depth[start.index()] < depth[end.index()] {
        (start, end)
    } else {
        (end, start)
    };

    let mut left: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();
    let mut right: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();

    let mut da = depth[node_a.index()];
    let mut a = node_a;
    while da > depth[node_b.index()] {
        left.push_back(a);
        if let Some(p) = parent[a.index()] {
            a = p;
            da = depth[a.index()];
        } else {
            break;
        }
    }

    let mut b = node_b;
    while a != b {
        left.push_back(a);
        right.push_front(b);
        if let Some(p) = parent[a.index()] {
            a = p;
        } else {
            break;
        }
        if let Some(p) = parent[b.index()] {
            b = p;
        } else {
            break;
        }
    }
    left.push_back(a);
    left.extend(right);
    left.into_iter().map(|idx| grph[idx].clone()).collect()
}

/// Minimum weighted set of vertices covering all odd cycles.
///
/// An odd cycle cover is a set $C \subseteq V$ such that $G[V \setminus C]$
/// contains no odd cycles (i.e., the remaining graph is bipartite).
///
/// Ported from Python `min_odd_cycle_cover()` in `cover.py`.
pub fn min_odd_cycle_cover<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    coverset: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let current_coverset = coverset.clone();
    let num_nodes = grph.node_count();
    let violate_fn = |soln: &HashSet<String>| -> Vec<Vec<String>> {
        // BFS with coloring to find odd cycles. Colors and parents are indexed
        // by node index (0/1 are the two colors, -1 marks an uncolored node).
        let mut color: Vec<i8> = vec![-1; num_nodes];
        let mut parent: Vec<Option<petgraph::graph::NodeIndex>> = vec![None; num_nodes];
        let mut depth: Vec<usize> = vec![0; num_nodes];

        for start in grph.node_indices() {
            if soln.contains(&grph[start]) || color[start.index()] >= 0 {
                continue;
            }

            color[start.index()] = 0;
            depth[start.index()] = 0;
            parent[start.index()] = None;

            let mut queue: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();
            queue.push_back(start);

            while let Some(current) = queue.pop_front() {
                let current_color = color[current.index()];
                for neighbor in grph.neighbors(current) {
                    if soln.contains(&grph[neighbor]) {
                        continue;
                    }
                    if color[neighbor.index()] < 0 {
                        color[neighbor.index()] = 1 - current_color;
                        depth[neighbor.index()] = depth[current.index()] + 1;
                        parent[neighbor.index()] = Some(current);
                        queue.push_back(neighbor);
                    } else if color[neighbor.index()] == current_color {
                        // Same color → odd cycle!
                        return vec![extract_odd_cycle(grph, &parent, &depth, current, neighbor)];
                    }
                }
            }
        }
        vec![]
    };

    let mut soln = current_coverset;
    pd_cover(violate_fn, weight, &mut soln)
}

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::UnGraph;

    fn make_graph(edges: &[(u32, u32)]) -> petgraph::Graph<String, (), petgraph::Undirected> {
        let mut grph = UnGraph::new_undirected();
        let mut indices = HashMap::new();
        for &(u, v) in edges {
            let key_u = format!("n{}", u);
            let key_v = format!("n{}", v);
            if !indices.contains_key(&key_u) {
                indices.insert(key_u.clone(), grph.add_node(key_u.clone()));
            }
            if !indices.contains_key(&key_v) {
                indices.insert(key_v.clone(), grph.add_node(key_v.clone()));
            }
            grph.add_edge(indices[&key_u], indices[&key_v], ());
        }
        grph
    }

    #[test]
    fn test_min_vertex_cover() {
        // Graph: 5 nodes, edges forming a K4-like structure
        let grph = make_graph(&[(0, 1), (0, 2), (1, 2), (1, 3), (2, 3), (2, 4), (3, 4)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
            ("n3".to_string(), 1),
            ("n4".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let mut coverset = HashSet::new();
        let (sol, _cost) = min_vertex_cover(&grph, &weight, &mut coverset);
        // Verify every edge is covered
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(
                sol.contains(u) || sol.contains(v),
                "Edge ({},{}) not covered",
                u,
                v
            );
        }
    }

    #[test]
    fn test_min_cycle_cover() {
        // Graph with cycles
        let grph = make_graph(&[(0, 1), (0, 2), (1, 2), (1, 3), (2, 3), (2, 4), (3, 4)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
            ("n3".to_string(), 1),
            ("n4".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let mut coverset = HashSet::new();
        let (sol, _cost) = min_cycle_cover(&grph, &weight, &mut coverset);
        // Verify graph is acyclic after removing sol
        let remaining: HashSet<String> = grph
            .node_indices()
            .map(|i| grph[i].clone())
            .filter(|n| !sol.contains(n))
            .collect();

        // Check no cycles in remaining graph
        let mut visited: HashSet<String> = HashSet::new();
        for node_idx in grph.node_indices() {
            let node = &grph[node_idx];
            if !remaining.contains(node) || visited.contains(node) {
                continue;
            }
            // Simple DFS cycle check
            let mut stack = vec![(node.clone(), None::<String>)];
            let mut local_visited = HashSet::new();
            while let Some((current, parent_opt)) = stack.pop() {
                if local_visited.contains(&current) {
                    panic!("Cycle still exists after removing cover");
                }
                local_visited.insert(current.clone());
                visited.insert(current.clone());
                let current_idx = grph.node_indices().find(|i| grph[*i] == current).unwrap();
                for neighbor_idx in grph.neighbors(current_idx) {
                    let neighbor = &grph[neighbor_idx];
                    if !remaining.contains(neighbor) {
                        continue;
                    }
                    if parent_opt.as_ref() == Some(neighbor) {
                        continue;
                    }
                    stack.push((neighbor.clone(), Some(current.clone())));
                }
            }
        }
    }

    #[test]
    fn test_min_odd_cycle_cover() {
        // Graph with both even and odd cycles
        let grph = make_graph(&[(0, 1), (0, 2), (1, 2), (1, 3), (2, 3), (2, 4), (3, 4)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
            ("n3".to_string(), 1),
            ("n4".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let mut coverset = HashSet::new();
        let (sol, _cost) = min_odd_cycle_cover(&grph, &weight, &mut coverset);

        // Verify remaining graph is bipartite (no odd cycles)
        let remaining: HashSet<String> = grph
            .node_indices()
            .map(|i| grph[i].clone())
            .filter(|n| !sol.contains(n))
            .collect();

        // BFS coloring to check bipartiteness
        let mut color: HashMap<String, Option<bool>> = HashMap::new();
        for node_idx in grph.node_indices() {
            let node = &grph[node_idx];
            if !remaining.contains(node) || color.contains_key(node) {
                continue;
            }
            let mut queue = VecDeque::new();
            color.insert(node.clone(), Some(true));
            queue.push_back(node.clone());
            while let Some(current) = queue.pop_front() {
                let current_idx = grph.node_indices().find(|i| grph[*i] == current).unwrap();
                for neighbor_idx in grph.neighbors(current_idx) {
                    let neighbor = &grph[neighbor_idx];
                    if !remaining.contains(neighbor) {
                        continue;
                    }
                    if !color.contains_key(neighbor) {
                        color.insert(neighbor.clone(), color[&current].map(|c| !c));
                        queue.push_back(neighbor.clone());
                    } else if color[&current] == color[neighbor] {
                        panic!("Odd cycle still exists after removing cover");
                    }
                }
            }
        }
    }

    #[test]
    fn test_min_vertex_cover_new() {
        let grph = make_graph(&[(0, 1), (1, 2)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 2),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let (sol, _cost) = min_vertex_cover_new(&grph, &weight);
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(sol.contains(u) || sol.contains(v));
        }
    }
}
