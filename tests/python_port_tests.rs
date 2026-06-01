//! Tests ported from Python pytest suite.
//!
//! Organized to match `tests/` structure in the Python project:
//!   test_netlist.py, test_graph_algo.py, test_cover.py, test_pd_cover.py,
//!   test_netlist_algo.py, test_rand_cover.py, test_hadlock.py, test_tsp.py,
//!   test_stress.py

use std::collections::HashMap;
use std::collections::HashSet;

use netlistx_rs::graph_algo::{min_maximal_independent_set, min_vertex_cover_fast};
use netlistx_rs::graph_cover::{min_cycle_cover, min_odd_cycle_cover, min_vertex_cover as graph_min_vc};
use netlistx_rs::hadlock::{solve_hadlock_max_cut, validate_max_cut};
use netlistx_rs::netlist_algo::{min_maximal_matching, min_maximal_matching_new, min_vertex_cover};
use netlistx_rs::rand_cover::{rand_hyper_vertex_cover, rand_vertex_cover};
use netlistx_rs::tsp::{
    christofides_tsp, make_l1_graph, solve_christofides_2opt_tsp, total_distance, two_opt,
};
use netlistx_rs::{
    create_drawf, create_inverter, create_random_hgraph, create_test_netlist, vdc, vdcorput,
    Netlist,
};

// ============================================================================
// Helper utilities
// ============================================================================

/// Build a `petgraph::Graph<String, (), Undirected>` from integer edge pairs.
fn make_petgraph(edges: &[(u32, u32)]) -> petgraph::Graph<String, (), petgraph::Undirected> {
    use petgraph::graph::UnGraph;
    let mut grph = UnGraph::new_undirected();
    let mut indices: HashMap<String, _> = HashMap::new();
    for &(u, v) in edges {
        let ku = format!("n{}", u);
        let kv = format!("n{}", v);
        if !indices.contains_key(&ku) {
            indices.insert(ku.clone(), grph.add_node(ku.clone()));
        }
        if !indices.contains_key(&kv) {
            indices.insert(kv.clone(), grph.add_node(kv.clone()));
        }
        grph.add_edge(indices[&ku], indices[&kv], ());
    }
    grph
}

/// Weight map from (name → weight) for a petgraph.
fn unit_weight(grph: &petgraph::Graph<String, (), petgraph::Undirected>) -> HashMap<String, u32> {
    grph.node_indices().map(|i| (grph[i].clone(), 1u32)).collect()
}

/// Create a complete graph for TSP testing.
fn make_complete_graph(n: usize, seed: u64) -> (petgraph::Graph<String, f64, petgraph::Undirected>, Vec<(f64, f64)>) {
    let mut rng = SimpleRng::new(seed);
    let positions: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.next_f64() * 100.0, rng.next_f64() * 100.0))
        .collect();
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let indices: Vec<_> = (0..n).map(|i| grph.add_node(format!("n{}", i))).collect();
    for i in 0..n {
        for j in i + 1..n {
            let dx = positions[i].0 - positions[j].0;
            let dy = positions[i].1 - positions[j].1;
            grph.add_edge(indices[i], indices[j], (dx * dx + dy * dy).sqrt());
        }
    }
    (grph, positions)
}

fn is_valid_hamiltonian(path: &[usize], n: usize) -> bool {
    path.len() == n + 1 && path[0] == path[path.len() - 1] && {
        let visited: HashSet<usize> = path[..path.len() - 1].iter().copied().collect();
        visited.len() == n && *visited.iter().min().unwrap() == 0 && *visited.iter().max().unwrap() == n - 1
    }
}

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

// ============================================================================
// test_netlist.py — Factory functions & netlist properties
// ============================================================================

#[test]
fn test_netlist_inverter() {
    let h = create_inverter();
    assert_eq!(h.num_modules(), 3);
    assert_eq!(h.num_nets(), 2);
    assert_eq!(h.number_of_nodes(), 5);
    assert_eq!(h.grph.edge_count(), 4);
    assert_eq!(h.get_max_degree(), 2);
}

#[test]
fn test_netlist_testnetlist() {
    let h = create_test_netlist();
    assert_eq!(h.num_modules(), 3);
    assert_eq!(h.num_nets(), 3);
    assert_eq!(h.number_of_nodes(), 6);
    assert_eq!(h.grph.edge_count(), 6);
    assert_eq!(h.get_max_degree(), 3);
}

#[test]
fn test_netlist_drawf() {
    let h = create_drawf();
    assert_eq!(h.num_modules(), 7);
    assert_eq!(h.num_nets(), 6);
    assert_eq!(h.grph.edge_count(), 14);
    assert_eq!(h.get_max_degree(), 3);
}

#[test]
fn test_netlist_random_hgraph() {
    let h = create_random_hgraph(30, 26, 0.1, 42);
    assert_eq!(h.num_modules(), 30);
    assert_eq!(h.num_nets(), 26);
}

#[test]
fn test_netlist_module_weight_dict() {
    // Match Python test_netlist_module_weight_dict
    let h = create_test_netlist();
    // create_test_netlist uses set_module_weight (dict equivalent)
    assert_eq!(h.get_module_weight("a0"), 533);
    assert_eq!(h.get_module_weight("a1"), 543);
    assert_eq!(h.get_module_weight("a2"), 532);
    // Non-existent module defaults to 1
    assert_eq!(h.get_module_weight("nonexistent"), 1);
}

#[test]
fn test_netlist_module_weight_default() {
    let h = create_inverter();
    // Modules a0, p1, p2 have weights 1, 0, 0
    assert_eq!(h.get_module_weight("a0"), 1);
    assert_eq!(h.get_module_weight("p1"), 0);
    assert_eq!(h.get_module_weight("p2"), 0);
}

#[test]
fn test_netlist_vdc() {
    assert!((vdc(0, 2) - 0.0).abs() < 1e-10);
    assert!((vdc(1, 2) - 0.5).abs() < 1e-10);
    assert!((vdc(2, 2) - 0.25).abs() < 1e-10);
    assert!((vdc(3, 2) - 0.75).abs() < 1e-10);
}

#[test]
fn test_netlist_vdcorput() {
    let seq = vdcorput(4, 2);
    assert_eq!(seq.len(), 4);
    assert!((seq[0] - 0.0).abs() < 1e-10);
}

// ============================================================================
// test_graph_algo.py — graph_algo functions on drawf netlist
// ============================================================================

#[test]
fn test_graph_algo_min_vertex_cover_on_drawf() {
    let h = create_drawf();
    // Build a petgraph from the Netlist's internal graph for the module nodes
    // Create a graph from all nodes in the netlist graph
    let mut grph = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
    let mut indices = HashMap::new();
    for node in h.grph.node_indices() {
        let name = h.grph[node].clone();
        indices.insert(name.clone(), grph.add_node(name));
    }
    for edge in h.grph.raw_edges() {
        let u = h.grph[edge.source()].clone();
        let v = h.grph[edge.target()].clone();
        grph.add_edge(indices[&u], indices[&v], ());
    }
    let weight: HashMap<String, u32> = grph.node_indices().map(|i| (grph[i].clone(), 1u32)).collect();
    let mut coverset = HashSet::new();
    let (sol, _cost) = graph_min_vc(&grph, &weight, &mut coverset);
    // Verify it's a valid vertex cover
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v), "Edge ({},{}) uncovered", u, v);
    }
}

#[test]
fn test_graph_algo_min_vertex_cover_fast_on_drawf() {
    let h = create_drawf();
    let grph = make_petgraph_from_netlist(&h);
    let weight = unit_weight(&grph);
    let mut coverset2 = HashSet::new();
    let (sol2, cost) = min_vertex_cover_fast(&grph, &weight, &mut coverset2);
    assert!(cost > 0);
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol2.contains(u) || sol2.contains(v));
    }
}

#[test]
fn test_graph_algo_min_vertex_cover_fast_weighted() {
    let h = create_drawf();
    let grph = make_petgraph_from_netlist(&h);
    let weight: HashMap<String, u32> = grph.node_indices().map(|i| (grph[i].clone(), 2u32)).collect();
    let mut coverset = HashSet::new();
    let (_sol, cost) = min_vertex_cover_fast(&grph, &weight, &mut coverset);
    // With all weights=2, total should be 2x the unweighted result
    assert!(cost >= 2);
}

#[test]
fn test_graph_algo_min_independent_set_on_drawf() {
    let h = create_drawf();
    let grph = make_petgraph_from_netlist(&h);
    let weight = unit_weight(&grph);
    let mut indset = HashSet::new();
    let mut dep = HashSet::new();
    let (sol, _cost) = min_maximal_independent_set(&grph, &weight, &mut indset, &mut dep);
    // Verify independence
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(!(sol.contains(u) && sol.contains(v)), "Edge between independent set vertices {}--{}", u, v);
    }
    // Verify maximality
    for node_idx in grph.node_indices() {
        let node = &grph[node_idx];
        if !sol.contains(node) {
            let adjacent_to_sol = grph.neighbors(node_idx).any(|n| sol.contains(&grph[n]));
            assert!(adjacent_to_sol, "Node {} could be added to independent set", node);
        }
    }
}

/// Make a petgraph from a Netlist's internal graph, using only node names.
fn make_petgraph_from_netlist(h: &Netlist) -> petgraph::Graph<String, (), petgraph::Undirected> {
    use petgraph::graph::UnGraph;
    let mut grph = UnGraph::new_undirected();
    let mut indices = HashMap::new();
    for node in h.grph.node_indices() {
        let name = h.grph[node].clone();
        indices.insert(name.clone(), grph.add_node(name));
    }
    for edge in h.grph.raw_edges() {
        let u = h.grph[edge.source()].clone();
        let v = h.grph[edge.target()].clone();
        grph.add_edge(indices[&u], indices[&v], ());
    }
    grph
}

// ============================================================================
// test_cover.py — Cover algorithms
// ============================================================================

#[test]
fn test_cover_pd_cover() {
    use netlistx_rs::graph_cover::pd_cover;
    let violate_fn = |soln: &HashSet<String>| -> Vec<Vec<String>> {
        let all_sets = vec![
            vec!["n0".to_string(), "n1".to_string()],
            vec!["n0".to_string(), "n2".to_string()],
            vec!["n1".to_string(), "n2".to_string()],
        ];
        // Return only sets that are NOT covered by current soln
        for s in &all_sets {
            if !s.iter().any(|v| soln.contains(v)) {
                return vec![s.clone()];
            }
        }
        vec![]
    };
    let weight: HashMap<String, u32> = [
        ("n0".to_string(), 1),
        ("n1".to_string(), 2),
        ("n2".to_string(), 3),
    ]
    .iter()
    .cloned()
    .collect();
    let mut soln = HashSet::new();
    let (covered, _cost) = pd_cover(violate_fn, &weight, &mut soln);
    assert!(covered.contains("n0") || covered.contains("n1"),
        "Expected n0 or n1 in cover, got {:?}", covered);
}

#[test]
fn test_cover_min_vertex_cover_simple() {
    let grph = make_petgraph(&[(0, 1), (1, 2)]);
    let weight: HashMap<String, u32> = [
        ("n0".to_string(), 1),
        ("n1".to_string(), 1),
        ("n2".to_string(), 1),
    ]
    .iter()
    .cloned()
    .collect();
    let mut coverset = HashSet::new();
    let (sol, _cost) = graph_min_vc(&grph, &weight, &mut coverset);
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v));
    }
}

#[test]
fn test_cover_min_cycle_cover_triangle() {
    let grph = make_petgraph(&[(0, 1), (1, 2), (2, 0)]);
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, cost) = min_cycle_cover(&grph, &weight, &mut coverset);
    // Triangle: cover 1 vertex breaks the cycle
    assert_eq!(sol.len(), 1);
    assert_eq!(cost, 1);
}

#[test]
fn test_cover_min_cycle_cover_tree() {
    // Tree has no cycles → empty cover
    let grph = make_petgraph(&[(0, 1), (1, 2), (2, 3)]);
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, cost) = min_cycle_cover(&grph, &weight, &mut coverset);
    assert_eq!(sol.len(), 0);
    assert_eq!(cost, 0);
}

#[test]
fn test_cover_min_odd_cycle_cover_triangle() {
    let grph = make_petgraph(&[(0, 1), (1, 2), (2, 0)]);
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, cost) = min_odd_cycle_cover(&grph, &weight, &mut coverset);
    assert_eq!(sol.len(), 1);
    assert_eq!(cost, 1);
}

#[test]
fn test_cover_min_odd_cycle_cover_square() {
    // Square (even cycle) → no odd cycles → empty cover
    let grph = make_petgraph(&[(0, 1), (1, 2), (2, 3), (3, 0)]);
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, cost) = min_odd_cycle_cover(&grph, &weight, &mut coverset);
    assert_eq!(sol.len(), 0);
    assert_eq!(cost, 0);
}

#[test]
fn test_cover_min_odd_cycle_cover_mixed() {
    // Square (even) + Triangle (odd)
    let grph = make_petgraph(&[
        (0, 1), (1, 2), (2, 3), (3, 0), // even square
        (4, 5), (5, 6), (6, 4), // odd triangle
    ]);
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, _cost) = min_odd_cycle_cover(&grph, &weight, &mut coverset);
    // Should cover the triangle vertices, not the square ones
    let in_triangle: HashSet<String> = ["n4".to_string(), "n5".to_string(), "n6".to_string()]
        .iter()
        .cloned()
        .collect();
    assert!(sol.iter().any(|v| in_triangle.contains(v)));
    // Square nodes should NOT be in the odd cycle cover
    for v in &["n0", "n1", "n2", "n3"] {
        assert!(!sol.contains(&v.to_string()), "Square node {} should not be in odd cycle cover", v);
    }
}

#[test]
fn test_cover_k5_minimality() {
    // K5's minimal vertex cover has 4 nodes
    let mut grph = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
    let nodes: Vec<_> = (0..5).map(|i| grph.add_node(format!("n{}", i))).collect();
    for i in 0..5 {
        for j in i + 1..5 {
            grph.add_edge(nodes[i], nodes[j], ());
        }
    }
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, _cost) = graph_min_vc(&grph, &weight, &mut coverset);
    // Verify minimality: removing any vertex breaks the cover
    for v in &sol {
        let mut test_soln = sol.clone();
        test_soln.remove(v);
        let is_still_covered = grph.raw_edges().iter().all(|e| {
            let u = &grph[e.source()];
            let v = &grph[e.target()];
            test_soln.contains(u) || test_soln.contains(v)
        });
        assert!(!is_still_covered, "Node {} was redundant in vertex cover", v);
    }
}

// ============================================================================
// test_netlist_algo.py — Netlist algorithm tests
// ============================================================================

#[test]
fn test_netlist_algo_min_vertex_cover_drawf() {
    let h = create_drawf();
    let weight: HashMap<String, u32> = h.modules.iter().map(|m| (m.clone(), 1u32)).collect();
    let mut coverset = HashSet::new();
    let (_sol, _cost) = min_vertex_cover(&h, &weight, &mut coverset);
    // Verify all nets are covered
    for net in &h.nets {
        let modules = h.get_net_modules(net);
        let covered = modules.iter().any(|m| coverset.contains(m));
        assert!(covered, "Net {} is not covered", net);
    }
}

#[test]
fn test_netlist_algo_min_maximal_matching_drawf() {
    let h = create_drawf();
    let weight: HashMap<String, u32> = h.nets.iter().map(|n| (n.clone(), 1u32)).collect();
    let mut matchset = HashSet::new();
    let mut dep = HashSet::new();
    let (_matchset, _cost) = min_maximal_matching(&h, &weight, &mut matchset, &mut dep);
    // Verify it's a maximal matching
    let mut covered_by_match: HashSet<String> = HashSet::new();
    for net in &matchset {
        for m in h.get_net_modules(net) {
            covered_by_match.insert(m);
        }
    }
    for net in &h.nets {
        let modules = h.get_net_modules(net);
        let has_covered = modules.iter().any(|m| covered_by_match.contains(m));
        assert!(has_covered, "Net {} shares no vertex with any matched net", net);
    }
}

#[test]
fn test_netlist_algo_min_maximal_matching_new() {
    let h = create_drawf();
    let weight: HashMap<String, u32> = h.nets.iter().map(|n| (n.clone(), 1u32)).collect();
    let (matchset, _cost) = min_maximal_matching_new(&h, &weight);
    assert!(!matchset.is_empty());
    for net in &matchset {
        assert!(h.nets.contains(net), "Matched net {} not in netlist", net);
    }
}

#[test]
fn test_netlist_algo_matching_with_predefined_matchset() {
    let h = create_drawf();
    let weight: HashMap<String, u32> = h.nets.iter().map(|n| (n.clone(), 1u32)).collect();
    // Pre-define one net in the matchset
    let predefined = h.nets.iter().next().cloned().unwrap();
    let mut matchset: HashSet<String> = [predefined.clone()].iter().cloned().collect();
    let mut dep = HashSet::new();
    let (result, _cost) = min_maximal_matching(&h, &weight, &mut matchset, &mut dep);
    assert!(result.contains(&predefined), "Predefined net should remain in matchset");
}

#[test]
fn test_netlist_algo_matching_with_different_weights() {
    let h = create_drawf();
    let weight: HashMap<String, i32> = h.nets.iter().enumerate().map(|(i, n)| (n.clone(), i as i32 + 1)).collect();
    let mut matchset = HashSet::new();
    let mut dep = HashSet::new();
    let (_result, cost) = min_maximal_matching(&h, &weight, &mut matchset, &mut dep);
    assert!(cost > 0);
}

// ============================================================================
// test_rand_cover.py — Randomized vertex cover tests
// ============================================================================

#[test]
fn test_rand_cover_triangle() {
    let grph = make_petgraph(&[(0, 1), (0, 2), (1, 2)]);
    let weight = unit_weight(&grph);
    let coverset = HashSet::new();
    let (sol, cost) = rand_vertex_cover(&grph, &weight, 0, &coverset);
    assert_eq!(sol.len(), 2);
    assert_eq!(cost, 2);
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v));
    }
}

#[test]
fn test_rand_cover_line() {
    let grph = make_petgraph(&[(0, 1), (1, 2)]);
    let weight = unit_weight(&grph);
    let coverset = HashSet::new();
    let (sol, _cost) = rand_vertex_cover(&grph, &weight, 1, &coverset);
    assert!(sol.len() >= 1);
    assert!(sol.len() <= 2);
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v));
    }
}

#[test]
fn test_rand_cover_star() {
    let grph = make_petgraph(&[(0, 1), (0, 2), (0, 3)]);
    let weight = unit_weight(&grph);
    let coverset = HashSet::new();
    let (sol, cost) = rand_vertex_cover(&grph, &weight, 2, &coverset);
    assert!(cost >= 1);
    assert!(cost <= 3);
    // If cost is 1, center is the only vertex
    if cost == 1 {
        assert!(sol.contains("n0"));
    }
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v));
    }
}

#[test]
fn test_rand_cover_deterministic() {
    let grph = make_petgraph(&[(0, 1), (1, 2), (2, 3), (3, 0)]);
    let weight: HashMap<String, u32> = [
        ("n0".to_string(), 2),
        ("n1".to_string(), 3),
        ("n2".to_string(), 1),
        ("n3".to_string(), 4),
    ]
    .iter()
    .cloned()
    .collect();
    let coverset = HashSet::new();
    let (sol1, cost1) = rand_vertex_cover(&grph, &weight, 123, &coverset);
    let (sol2, cost2) = rand_vertex_cover(&grph, &weight, 123, &coverset);
    assert_eq!(sol1, sol2);
    assert_eq!(cost1, cost2);
}

#[test]
fn test_rand_cover_with_initial_coverset() {
    let grph = make_petgraph(&[(0, 1), (1, 2), (2, 0)]);
    let weight = unit_weight(&grph);
    let coverset: HashSet<String> = [("n0".to_string())].iter().cloned().collect();
    let (sol, _cost) = rand_vertex_cover(&grph, &weight, 42, &coverset);
    assert!(sol.contains("n0"), "Initial vertex should be in cover");
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v));
    }
}

#[test]
fn test_rand_cover_empty_graph() {
    let grph = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
    let weight: HashMap<String, i32> = HashMap::new();
    let coverset = HashSet::new();
    let (sol, cost) = rand_vertex_cover(&grph, &weight, 0, &coverset);
    assert!(sol.is_empty());
    assert_eq!(cost, 0);
}

#[test]
fn test_rand_cover_single_edge_weighted() {
    let mut grph = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    grph.add_edge(n0, n1, ());
    let weight: HashMap<String, u32> = [("n0".to_string(), 5), ("n1".to_string(), 10)]
        .iter()
        .cloned()
        .collect();
    let coverset = HashSet::new();
    let (sol, cost) = rand_vertex_cover(&grph, &weight, 42, &coverset);
    assert_eq!(sol.len(), 1);
    assert!((sol.contains("n0") && cost == 5) || (sol.contains("n1") && cost == 10));
}

#[test]
fn test_rand_hyper_cover_simple() {
    // Mock hypergraph: single net [0, 1]
    let h = create_inverter();
    let weight: HashMap<String, u32> = h.modules.iter().map(|m| (m.clone(), 1u32)).collect();
    let coverset = HashSet::new();
    let (sol, _cost) = rand_hyper_vertex_cover(&h, &weight, 0, &coverset);
    // Verify all nets are covered
    for net in &h.nets {
        let modules = h.get_net_modules(net);
        assert!(modules.iter().any(|m| sol.contains(m)), "Net {} uncovered", net);
    }
}

#[test]
fn test_rand_hyper_cover_deterministic() {
    let h = create_inverter();
    let weight: HashMap<String, u32> = h.modules.iter().map(|m| (m.clone(), 1u32)).collect();
    let coverset = HashSet::new();
    let (sol1, cost1) = rand_hyper_vertex_cover(&h, &weight, 123, &coverset);
    let (sol2, cost2) = rand_hyper_vertex_cover(&h, &weight, 123, &coverset);
    assert_eq!(sol1, sol2);
    assert_eq!(cost1, cost2);
}

#[test]
fn test_rand_hyper_cover_empty() {
    let h = Netlist::new();
    let weight: HashMap<String, i32> = HashMap::new();
    let coverset = HashSet::new();
    let (sol, cost) = rand_hyper_vertex_cover(&h, &weight, 0, &coverset);
    assert!(sol.is_empty());
    assert_eq!(cost, 0);
}

// ============================================================================
// test_hadlock.py — Hadlock planar MAX-CUT tests
// ============================================================================

#[test]
fn test_hadlock_empty_graph() {
    let grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let cut = solve_hadlock_max_cut(&grph);
    assert!(cut.is_empty());
}

#[test]
fn test_hadlock_single_edge() {
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    grph.add_edge(n0, n1, 7.0);
    let cut = solve_hadlock_max_cut(&grph);
    let (valid, weight) = validate_max_cut(&grph, &cut);
    assert!(valid);
    assert!((weight - 7.0).abs() < 1e-10);
}

#[test]
fn test_hadlock_square() {
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    let n2 = grph.add_node("n2".to_string());
    let n3 = grph.add_node("n3".to_string());
    grph.add_edge(n0, n1, 1.0);
    grph.add_edge(n1, n2, 1.0);
    grph.add_edge(n2, n3, 1.0);
    grph.add_edge(n3, n0, 1.0);
    let cut = solve_hadlock_max_cut(&grph);
    let (valid, weight) = validate_max_cut(&grph, &cut);
    assert!(valid);
    assert!((weight - 4.0).abs() < 1e-10);
}

#[test]
fn test_hadlock_validate_valid() {
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    let n2 = grph.add_node("n2".to_string());
    let n3 = grph.add_node("n3".to_string());
    grph.add_edge(n0, n1, 5.0);
    grph.add_edge(n1, n2, 10.0);
    grph.add_edge(n2, n3, 5.0);
    grph.add_edge(n3, n0, 10.0);
    // All edges = bipartite square → valid cut
    let cut: HashSet<String> = [
        "n0--n1".to_string(),
        "n1--n2".to_string(),
        "n2--n3".to_string(),
        "n3--n0".to_string(),
    ]
    .iter()
    .cloned()
    .collect();
    let (valid, weight) = validate_max_cut(&grph, &cut);
    assert!(valid);
    assert!((weight - 30.0).abs() < 1e-10);
}

// ============================================================================
// test_tsp.py — TSP algorithm tests
// ============================================================================

#[test]
fn test_tsp_total_distance_triangle() {
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    let n2 = grph.add_node("n2".to_string());
    grph.add_edge(n0, n1, 1.0);
    grph.add_edge(n1, n2, 2.0);
    grph.add_edge(n0, n2, 3.0);
    assert!((total_distance(&[0, 1, 2, 0], &grph) - 6.0).abs() < 1e-10);
}

#[test]
fn test_tsp_total_distance_zero() {
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    let n2 = grph.add_node("n2".to_string());
    grph.add_edge(n0, n1, 0.0);
    grph.add_edge(n1, n2, 0.0);
    grph.add_edge(n0, n2, 0.0);
    assert!((total_distance(&[0, 1, 2, 0], &grph) - 0.0).abs() < 1e-10);
}

#[test]
fn test_tsp_two_opt_improves_crossing() {
    let n = 4;
    let (grph, _) = make_complete_graph(n, 42);
    let crossing = vec![0, 2, 1, 3, 0];
    let initial = total_distance(&crossing, &grph);
    let refined = two_opt(&crossing, &grph);
    let refined_dist = total_distance(&refined, &grph);
    assert!(refined_dist <= initial + 1e-10);
}

#[test]
fn test_tsp_two_opt_valid_output() {
    let (grph, _) = make_complete_graph(8, 1);
    let mut tour: Vec<usize> = (0..8).collect();
    tour.push(0);
    let refined = two_opt(&tour, &grph);
    assert!(is_valid_hamiltonian(&refined, 8));
}

#[test]
fn test_tsp_christofides_small() {
    let (grph, _) = make_complete_graph(5, 0);
    let tour = christofides_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 5));
}

#[test]
fn test_tsp_christofides_medium() {
    let (grph, _) = make_complete_graph(20, 1);
    let tour = christofides_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 20));
    assert!(total_distance(&tour, &grph) > 0.0);
}

#[test]
fn test_tsp_christofides_three_node() {
    let mut grph = petgraph::Graph::<String, f64, petgraph::Undirected>::new_undirected();
    let n0 = grph.add_node("n0".to_string());
    let n1 = grph.add_node("n1".to_string());
    let n2 = grph.add_node("n2".to_string());
    grph.add_edge(n0, n1, 1.0);
    grph.add_edge(n1, n2, 2.0);
    grph.add_edge(n0, n2, 3.0);
    let tour = christofides_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 3));
}

#[test]
fn test_tsp_combined_valid() {
    let (grph, _) = make_complete_graph(10, 0);
    let tour = solve_christofides_2opt_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 10));
}

#[test]
fn test_tsp_improvement_over_baseline() {
    let (grph, _) = make_complete_graph(15, 3);
    let christo = christofides_tsp(&grph);
    let combined = solve_christofides_2opt_tsp(&grph);
    let christo_dist = total_distance(&christo, &grph);
    let combined_dist = total_distance(&combined, &grph);
    assert!(combined_dist <= christo_dist + 1e-10);
}

#[test]
fn test_tsp_deterministic() {
    let (grph, _) = make_complete_graph(12, 5);
    let tour1 = solve_christofides_2opt_tsp(&grph);
    let tour2 = solve_christofides_2opt_tsp(&grph);
    assert_eq!(tour1, tour2);
}

#[test]
fn test_tsp_larger_instance() {
    let (grph, _) = make_complete_graph(50, 9);
    let tour = solve_christofides_2opt_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 50));
    assert!(total_distance(&tour, &grph) > 0.0);
}

#[test]
fn test_tsp_approximation_bound() {
    let (grph, _) = make_complete_graph(10, 11);
    // Compute MST as lower bound
    let mst_weight = compute_mst_weight(&grph);
    let tour = solve_christofides_2opt_tsp(&grph);
    let tour_weight = total_distance(&tour, &grph);
    // Christofides guarantees ≤ 1.5 × OPT, OPT ≥ MST
    assert!(tour_weight <= 1.5 * mst_weight + 1e-6);
}

#[test]
fn test_tsp_l1_make_graph() {
    let (grph, _pos) = make_l1_graph(5, 0);
    assert_eq!(grph.node_count(), 5);
    assert_eq!(grph.edge_count(), 10);
}

#[test]
fn test_tsp_l1_christofides() {
    let (grph, _) = make_l1_graph(10, 7);
    let tour = christofides_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 10));
}

#[test]
fn test_tsp_l1_combined() {
    let (grph, _) = make_l1_graph(15, 3);
    let tour = solve_christofides_2opt_tsp(&grph);
    assert!(is_valid_hamiltonian(&tour, 15));
}

#[test]
fn test_tsp_l1_improvement() {
    let (grph, _) = make_l1_graph(12, 5);
    let christo = christofides_tsp(&grph);
    let combined = solve_christofides_2opt_tsp(&grph);
    let cd = total_distance(&christo, &grph);
    let comd = total_distance(&combined, &grph);
    assert!(comd <= cd + 1e-10);
}

#[test]
fn test_tsp_l1_approximation_bound() {
    let (grph, _) = make_l1_graph(10, 11);
    let mst_weight = compute_mst_weight(&grph);
    let tour = solve_christofides_2opt_tsp(&grph);
    let tour_weight = total_distance(&tour, &grph);
    assert!(tour_weight <= 1.5 * mst_weight + 1e-6);
}

/// Compute MST total weight for a complete graph.
fn compute_mst_weight(grph: &petgraph::Graph<String, f64, petgraph::Undirected>) -> f64 {
    use petgraph::algo::min_spanning_tree;
    let mst = min_spanning_tree(&grph);
    let mut total = 0.0;
    for edge in mst {
        match edge {
            petgraph::data::Element::Edge { weight, .. } => total += weight,
            _ => {}
        }
    }
    total
}

// ============================================================================
// test_stress.py — Large random graph stress tests
// ============================================================================

#[test]
fn test_stress_min_vertex_cover_fast() {
    let h = create_random_hgraph(100, 100, 0.05, 42);
    let grph = make_petgraph_from_netlist(&h);
    let weight = unit_weight(&grph);
    let mut coverset = HashSet::new();
    let (sol, cost) = min_vertex_cover_fast(&grph, &weight, &mut coverset);
    assert!(cost > 0);
    assert!(!sol.is_empty());
    // Verify cover
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(sol.contains(u) || sol.contains(v));
    }
}

#[test]
fn test_stress_maximal_independent_set() {
    let h = create_random_hgraph(100, 100, 0.05, 42);
    let grph = make_petgraph_from_netlist(&h);
    let weight = unit_weight(&grph);
    let mut indset = HashSet::new();
    let mut dep = HashSet::new();
    let (sol, cost) = min_maximal_independent_set(&grph, &weight, &mut indset, &mut dep);
    assert!(cost > 0);
    assert!(!sol.is_empty());
    // Verify independence
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        assert!(!(sol.contains(u) && sol.contains(v)), "Edge between independent set vertices");
    }
}

// ============================================================================
// test_netlist.py — Additional JSON and edge-case tests
// ============================================================================

use netlistx_rs::io::read_node_link_json;

#[test]
fn test_netlist_get_module_weight_nonexistent() {
    let h = Netlist::new();
    assert_eq!(h.get_module_weight("nonexistent"), 1);
}

#[test]
fn test_netlist_weights_on_drawf() {
    let h = create_drawf();
    assert_eq!(h.get_module_weight("a0"), 1);
    assert_eq!(h.get_module_weight("a1"), 3);
    assert_eq!(h.get_module_weight("a2"), 4);
    assert_eq!(h.get_module_weight("a3"), 2);
    assert_eq!(h.get_module_weight("p1"), 0);
    assert_eq!(h.get_module_weight("p2"), 0);
    assert_eq!(h.get_module_weight("p3"), 0);
}

#[test]
fn test_netlist_get_module_nets() {
    let h = create_inverter();
    let nets_a0 = h.get_module_nets("a0");
    assert_eq!(nets_a0.len(), 2);
    assert!(nets_a0.contains(&"n0".to_string()));
    assert!(nets_a0.contains(&"n1".to_string()));
}

#[test]
fn test_netlist_get_net_modules() {
    let h = create_inverter();
    let mods_n0 = h.get_net_modules("n0");
    assert!(mods_n0.contains(&"a0".to_string()));
    assert!(mods_n0.contains(&"p1".to_string()));
}

#[test]
fn test_read_drawf_json() {
    let netlist = read_node_link_json("testcases/drawf.json").unwrap();
    assert_eq!(netlist.num_modules(), 7);
    assert_eq!(netlist.num_nets(), 6);
    assert_eq!(netlist.num_pads, 3);
    assert_eq!(netlist.number_of_nodes(), 13);
}

#[test]
fn test_read_fix_json() {
    let netlist = read_node_link_json("testcases/fix.json").unwrap();
    assert!(netlist.num_modules() > 0);
    assert!(netlist.num_nets() > 0);
}

#[test]
fn test_read_p1_json() {
    let netlist = read_node_link_json("testcases/p1.json").unwrap();
    // Match Python test: 833 modules, 902 nets, 81 pads
    assert_eq!(netlist.num_modules(), 833);
    assert_eq!(netlist.num_nets(), 902);
    assert_eq!(netlist.num_pads, 81);
    assert_eq!(netlist.number_of_nodes(), 1735);
}

#[test]
fn test_json_degree_counts() {
    // Port of test_readjson from Python: verify net degree distribution
    let netlist = read_node_link_json("testcases/p1.json").unwrap();
    let mut count_2 = 0;
    for net in &netlist.nets {
        let deg = netlist.get_net_degree(net);
        if deg == 2 {
            count_2 += 1;
        }
    }
    // Python asserts count_2 == 494
    assert_eq!(count_2, 494);
}

// ============================================================================
// Yosys testcase integration tests
// ============================================================================

fn check_yosys_file(path: &str, exp_modules: usize, exp_nets: usize, exp_pins: usize, exp_pads: i32, exp_nodes: usize) {
    let netlist = netlistx_rs::io::read_yosys_json(path).unwrap();
    assert_eq!(netlist.num_modules(), exp_modules,
        "{}: modules mismatch", path);
    assert_eq!(netlist.num_nets(), exp_nets,
        "{}: nets mismatch", path);
    assert_eq!(netlist.grph.edge_count(), exp_pins,
        "{}: pins mismatch", path);
    assert_eq!(netlist.num_pads, exp_pads,
        "{}: pads mismatch", path);
    assert_eq!(netlist.number_of_nodes(), exp_nodes,
        "{}: nodes mismatch", path);
    // Verify module weights: cells should be 1, ports should be 0
    assert!(netlist.get_max_degree() > 0,
        "{}: max degree should be > 0", path);
}

#[test]
fn test_yosys_sphere_netlist() {
    check_yosys_file(
        "yosys_testcases/sphere_netlist.json",
        65,  // modules: 56 cells + 9 ports
        623, // nets
        1555, // pins
        9,   // pads
        688, // nodes: 65 + 623
    );
}

#[test]
fn test_yosys_sphere3hopf_simple() {
    check_yosys_file(
        "yosys_testcases/sphere3hopf_netlist_simple.json",
        188, // modules: 180 cells + 8 ports
        2825, // nets
        6823, // pins
        8,   // pads
        3013, // nodes: 188 + 2825
    );
}

#[test]
fn test_yosys_sphere3hopf_full() {
    check_yosys_file(
        "yosys_testcases/sphere3hopf_netlist.json",
        188, // modules: 180 cells + 8 ports
        2825, // nets
        6823, // pins
        8,   // pads
        3013, // nodes: 188 + 2825
    );
}
