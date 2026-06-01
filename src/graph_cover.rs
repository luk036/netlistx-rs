/// Graph Cover Algorithms (ported from Python `cover.py`)
///
/// Implements primal-dual approximation algorithms with reverse-delete post-processing
/// for various covering problems in graphs:
/// - `min_vertex_cover`: minimum weighted vertex cover
/// - `min_cycle_cover`: minimum weighted set of vertices covering all cycles
/// - `min_odd_cycle_cover`: minimum weighted set of vertices covering all odd cycles

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::ops::Add;
use std::ops::Sub;

/// Generic primal-dual approximation algorithm with reverse-delete post-processing.
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
    let mut gap: HashMap<String, W> = weight.clone();
    let mut added_order: Vec<String> = Vec::new();
    let mut total_dual_cost: W = W::default();

    // Phase 1: Primal-Dual Selection
    loop {
        let viol_sets = violate(soln);
        if viol_sets.is_empty() {
            break;
        }
        // Take the first violating set
        let set = viol_sets.into_iter().next().unwrap();
        if set.is_empty() {
            continue;
        }

        // Find element with minimum gap in this violating set
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

        total_dual_cost = total_dual_cost + min_val;
        for vtx in &set {
            let entry = gap.entry(vtx.clone()).or_insert(weight[vtx]);
            *entry = *entry - min_val;
        }
    }

    // Phase 2: Reverse-Delete Post-Processing
    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        let viol_sets = violate(soln);
        let has_violation = viol_sets.iter().any(|s| !s.is_empty());
        if has_violation {
            soln.insert(vtx.clone());
        }
    }

    let final_primal_cost: W = soln
        .iter()
        .map(|vtx| weight.get(vtx).copied().unwrap_or(W::default()))
        .fold(W::default(), |acc, w| acc + w);

    (soln.clone(), final_primal_cost)
}

/// Minimum weighted vertex cover for a graph using primal-dual with reverse-delete.
///
/// A vertex cover is a set of vertices where every edge has at least one endpoint
/// in the set.
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
    let current_coverset = coverset.clone();
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

    let mut soln = current_coverset;
    pd_cover(violate_fn, weight, &mut soln)
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
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();

    for node_idx in grph.node_indices() {
        let source = &grph[node_idx];
        if coverset.contains(source) || visited.contains(source) {
            continue;
        }

        // BFS
        let mut parent: HashMap<String, Option<String>> = HashMap::new();
        let mut depth: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        parent.insert(source.clone(), None);
        depth.insert(source.clone(), 0);
        queue.push_back(source.clone());
        visited.insert(source.clone());

        while let Some(current) = queue.pop_front() {
            let current_depth = *depth.get(&current).unwrap_or(&0);
            let current_idx = grph
                .node_indices()
                .find(|i| grph[*i] == current)
                .expect("node not found");

            for neighbor_idx in grph.neighbors(current_idx) {
                let neighbor = &grph[neighbor_idx];
                if coverset.contains(neighbor) {
                    continue;
                }
                if !depth.contains_key(neighbor) {
                    parent.insert(neighbor.clone(), Some(current.clone()));
                    depth.insert(neighbor.clone(), current_depth + 1);
                    queue.push_back(neighbor.clone());
                    visited.insert(neighbor.clone());
                } else if depth[neighbor] != current_depth - 1 {
                    // Found a back edge (not the direct parent)
                    let is_direct_parent = parent
                        .get(&current)
                        .and_then(|p| p.as_ref())
                        .map(|p| p == neighbor)
                        .unwrap_or(false);
                    if !is_direct_parent {
                        let cycle = construct_cycle(&parent, &depth, &current, neighbor);
                        cycles.push(cycle);
                        // Only find one cycle per BFS to avoid duplicates
                        return cycles;
                    }
                }
            }
        }
    }

    cycles
}

/// Minimum weighted set of vertices covering all cycles.
///
/// A cycle cover is a set of vertices such that removing them from the graph
/// eliminates all cycles (i.e., the remaining graph is a forest).
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

/// Minimum weighted set of vertices covering all odd cycles.
///
/// An odd cycle cover is a set of vertices such that removing them from the
/// graph eliminates all odd-length cycles (making the graph bipartite).
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
    let violate_fn = |soln: &HashSet<String>| -> Vec<Vec<String>> {
        // BFS with coloring to find odd cycles
        let mut color: HashMap<String, Option<bool>> = HashMap::new();
        for node_idx in grph.node_indices() {
            let source = &grph[node_idx];
            if soln.contains(source) || color.contains_key(source) {
                continue;
            }

            let mut parent: HashMap<String, Option<String>> = HashMap::new();
            let mut depth: HashMap<String, usize> = HashMap::new();
            let mut queue: VecDeque<String> = VecDeque::new();

            color.insert(source.clone(), Some(true));
            depth.insert(source.clone(), 0);
            parent.insert(source.clone(), None);
            queue.push_back(source.clone());

            while let Some(current) = queue.pop_front() {
                let current_color = *color.get(&current).unwrap_or(&Some(false));
                let current_idx = grph
                    .node_indices()
                    .find(|i| grph[*i] == current)
                    .expect("node not found");

                for neighbor_idx in grph.neighbors(current_idx) {
                    let neighbor = &grph[neighbor_idx];
                    if soln.contains(neighbor) {
                        continue;
                    }
                    if !color.contains_key(neighbor) {
                        color.insert(neighbor.clone(), current_color.map(|c| !c));
                        depth.insert(neighbor.clone(), depth[&current] + 1);
                        parent.insert(neighbor.clone(), Some(current.clone()));
                        queue.push_back(neighbor.clone());
                    } else if color[&current] == color[neighbor] {
                        // Same color → odd cycle!
                        // Find the cycle
                        let cycle = construct_cycle(&parent, &depth, &current, neighbor);
                        return vec![cycle];
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
            assert!(sol.contains(u) || sol.contains(v),
                    "Edge ({},{}) not covered", u, v);
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
        let remaining: HashSet<String> = grph.node_indices()
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
                let current_idx = grph.node_indices()
                    .find(|i| grph[*i] == current).unwrap();
                for neighbor_idx in grph.neighbors(current_idx) {
                    let neighbor = &grph[neighbor_idx];
                    if !remaining.contains(neighbor) {
                        continue;
                    }
                    if parent_opt.as_ref().map_or(false, |p| p == neighbor) {
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
        let remaining: HashSet<String> = grph.node_indices()
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
                let current_idx = grph.node_indices()
                    .find(|i| grph[*i] == current).unwrap();
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
