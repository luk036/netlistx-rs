/// Graph Algorithms (ported from Python `graph_algo.py`)
///
/// This module provides primal-dual approximation algorithms for graphs.
/// - `min_vertex_cover_fast`: minimum weighted vertex cover (without post-processing)
/// - `min_maximal_independent_set`: minimum weighted maximal independent set

use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Add;
use std::ops::Sub;

/// Minimum weighted vertex cover using primal-dual approximation (no post-processing).
///
/// For each uncovered edge, swaps so that vtx has the smaller gap, then adds vtx
/// to the cover. Updates gap[utx] -= gap[vtx] and sets gap[vtx] = 0.
///
/// Ported from Python `min_vertex_cover_fast()` in `graph_algo.py`.
pub fn min_vertex_cover_fast<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    coverset: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let mut gap: HashMap<String, W> = weight.clone();
    let mut total_dual_cost: W = W::default();
    let mut total_primal_cost: W = W::default();

    for edge in grph.raw_edges() {
        let mut u = &grph[edge.source()];
        let mut v = &grph[edge.target()];

        if coverset.contains(u) || coverset.contains(v) {
            continue;
        }

        let gu = *gap.get(u).unwrap_or(&weight[u]);
        let gv = *gap.get(v).unwrap_or(&weight[v]);

        if gu < gv {
            std::mem::swap(&mut u, &mut v);
        }
        // Now gap[u] >= gap[v], add v to cover
        let gv = *gap.get(v).unwrap_or(&weight[v]);
        coverset.insert(v.clone());
        total_dual_cost = total_dual_cost + gv;
        total_primal_cost = total_primal_cost + *weight.get(v).unwrap_or(&W::default());
        if let Some(g) = gap.get_mut(u) {
            *g = *g - gv;
        }
        gap.insert(v.clone(), W::default());
    }

    (coverset.clone(), total_primal_cost)
}

/// Minimum weighted maximal independent set using primal-dual approximation.
///
/// Finds a maximal set of vertices with no edges between them (independent set),
/// minimizing total weight. The algorithm is maximal (no vertex can be added
/// without breaking independence).
///
/// Ported from Python `min_maximal_independant_set()` in `graph_algo.py`.
pub fn min_maximal_independent_set<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    indset: &mut HashSet<String>,
    dep: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let mut gap: HashMap<String, W> = weight.clone();
    let mut total_primal_cost: W = W::default();
    let mut total_dual_cost: W = W::default();

    for node_idx in grph.node_indices() {
        let u = &grph[node_idx];

        if dep.contains(u) {
            continue;
        }
        if indset.contains(u) {
            continue;
        }

        // Find min gap vertex among u and its uncovered neighbors
        let mut min_val = *gap.get(u).unwrap_or(&weight[u]);
        let mut min_vtx = u.clone();

        for neighbor_idx in grph.neighbors(node_idx) {
            let v = &grph[neighbor_idx];
            if dep.contains(v) {
                continue;
            }
            let gv = *gap.get(v).unwrap_or(&weight[v]);
            if min_val > gv {
                min_val = gv;
                min_vtx = v.clone();
            }
        }

        // Add min_vtx to independent set
        indset.insert(min_vtx.clone());
        total_primal_cost = total_primal_cost + *weight.get(&min_vtx).unwrap_or(&W::default());
        total_dual_cost = total_dual_cost + min_val;

        // Cover min_vtx and all its neighbors
        coverset_dep(grph, &min_vtx, dep);

        // Update gap of neighbors of u
        if min_vtx == *u {
            continue;
        }
        for neighbor_idx in grph.neighbors(node_idx) {
            let v = &grph[neighbor_idx];
            if let Some(g) = gap.get_mut(v) {
                *g = *g - min_val;
            }
        }
    }

    (indset.clone(), total_primal_cost)
}

/// Cover a vertex and all its neighbors in the dependency set.
fn coverset_dep(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    vtx: &str,
    dep: &mut HashSet<String>,
) {
    dep.insert(vtx.to_string());
    for node_idx in grph.node_indices() {
        if grph[node_idx] == vtx {
            for neighbor_idx in grph.neighbors(node_idx) {
                dep.insert(grph[neighbor_idx].clone());
            }
            break;
        }
    }
}

/// Convenience version that creates empty indset and dep sets.
pub fn min_maximal_independent_set_new<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
) -> (HashSet<String>, W)
where
    W: Copy + Add<Output = W> + Sub<Output = W> + PartialOrd + Default,
{
    let mut indset = HashSet::new();
    let mut dep = HashSet::new();
    min_maximal_independent_set(grph, weight, &mut indset, &mut dep)
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

    fn make_weight(grph: &petgraph::Graph<String, (), petgraph::Undirected>, w: u32) -> HashMap<String, u32> {
        let mut weight = HashMap::new();
        for node_idx in grph.node_indices() {
            weight.insert(grph[node_idx].clone(), w);
        }
        weight
    }

    #[test]
    fn test_min_vertex_cover_fast_simple() {
        // Triangle graph: nodes 0,1,2 with edges (0,1), (0,2), (1,2)
        let grph = make_graph(&[(0, 1), (0, 2), (1, 2)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let mut coverset = HashSet::new();
        let (sol, cost) = min_vertex_cover_fast(&grph, &weight, &mut coverset);
        // A triangle's minimum vertex cover has size 2 (pick any 2 vertices)
        assert_eq!(sol.len(), 2);
        assert_eq!(cost, 2);
        // Verify it's a valid vertex cover
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(sol.contains(u) || sol.contains(v));
        }
    }

    #[test]
    fn test_min_vertex_cover_fast_weighted() {
        let grph = make_graph(&[(0, 1), (1, 2)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 2),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let mut coverset = HashSet::new();
        let (sol, cost) = min_vertex_cover_fast(&grph, &weight, &mut coverset);
        assert_eq!(cost, 2);
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(sol.contains(u) || sol.contains(v));
        }
    }

    #[test]
    fn test_min_maximal_independent_set_simple() {
        // Triangle: nodes 0,1,2 with edges (0,1), (0,2), (1,2)
        let grph = make_graph(&[(0, 1), (0, 2), (1, 2)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let mut indset = HashSet::new();
        let mut dep = HashSet::new();
        let (sol, cost) = min_maximal_independent_set(&grph, &weight, &mut indset, &mut dep);
        // Triangle's maximal independent set has size 1
        assert_eq!(sol.len(), 1);
        assert!(cost == 1);
        // Verify independence: no edge between any two vertices in the set
        let sol_nodes: Vec<_> = sol.iter().collect();
        for i in 0..sol_nodes.len() {
            for j in (i + 1)..sol_nodes.len() {
                let has_edge = grph.raw_edges().iter().any(|e| {
                    (grph[e.source()] == *sol_nodes[i] && grph[e.target()] == *sol_nodes[j])
                        || (grph[e.source()] == *sol_nodes[j] && grph[e.target()] == *sol_nodes[i])
                });
                assert!(!has_edge, "Edge between {} and {}", sol_nodes[i], sol_nodes[j]);
            }
        }
        // Verify maximal: no vertex can be added
        for node_idx in grph.node_indices() {
            let node = &grph[node_idx];
            if !sol.contains(node) {
                // Check if adding would break independence
                let adjacent_to_sol = grph.neighbors(node_idx).any(|n| sol.contains(&grph[n]));
                assert!(adjacent_to_sol, "Node {} could be added to independent set", node);
            }
        }
    }

    #[test]
    fn test_min_maximal_independent_set_new() {
        let grph = make_graph(&[(0, 1), (1, 2)]);
        let weight: HashMap<String, u32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 2),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let (sol, cost) = min_maximal_independent_set_new(&grph, &weight);
        assert_eq!(cost, 2);
        assert_eq!(sol.len(), 2);
    }
}
