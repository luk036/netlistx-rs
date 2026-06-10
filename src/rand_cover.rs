use std::collections::HashMap;
use std::collections::HashSet;

use rand::SeedableRng;

use crate::netlist::Netlist;

/// Single trial of Pitt's randomized hypergraph vertex cover.
///
/// For each uncovered net, selects one endpoint with probability inversely
/// proportional to its weight. Then applies reverse-delete post-processing.
///
/// Ported from C++ `rand_hyper_vertex_cover_trial()` in `rand_cover.hpp`.
pub fn rand_hyper_vertex_cover_trial<W, R>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    coverset: &HashSet<String>,
    rng: &mut R,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default,
    R: rand::Rng,
{
    let mut soln: HashSet<String> = coverset.iter().cloned().collect();
    let mut added_order: Vec<String> = Vec::new();

    for net in &netlist.nets {
        let modules = netlist.get_net_modules(net);
        if modules.is_empty() {
            continue;
        }

        let already_covered = modules.iter().any(|m| soln.contains(m));
        if already_covered {
            continue;
        }

        let total_inv: f64 = modules.iter().map(|m| 1.0 / weight[m].into()).sum();

        if total_inv <= 0.0 {
            continue;
        }

        let r: f64 = rng.gen();
        let mut cumulative = 0.0;
        let mut chosen = &modules[0];

        for m in &modules {
            cumulative += 1.0 / weight[m].into() / total_inv;
            if r < cumulative {
                chosen = m;
                break;
            }
        }

        soln.insert(chosen.clone());
        added_order.push(chosen.clone());
    }

    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        let mut valid = true;
        for net in &netlist.nets {
            let modules = netlist.get_net_modules(net);
            if modules.is_empty() {
                continue;
            }
            let net_covered = modules.iter().any(|m| soln.contains(m));
            if !net_covered {
                valid = false;
                break;
            }
        }
        if !valid {
            soln.insert(vtx.clone());
        }
    }

    let total_cost: W = soln
        .iter()
        .map(|v| weight[v])
        .fold(W::default(), |acc, w| acc + w);

    (soln, total_cost)
}

/// Single-trial hypergraph vertex cover with optional seed.
///
/// Ported from C++ `rand_hyper_vertex_cover()` in `rand_cover.hpp`.
pub fn rand_hyper_vertex_cover<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    seed: u64,
    coverset: &HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default,
{
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    rand_hyper_vertex_cover_trial(netlist, weight, coverset, &mut rng)
}

/// Multi-threaded hypergraph vertex cover using `rayon`.
///
/// Runs `num_trials` independent trials in parallel and returns the best cover.
///
/// Ported from C++ `rand_hyper_vertex_cover_mt()` in `rand_cover.hpp`.
///
/// Requires the `rayon` feature.
#[cfg(feature = "rayon")]
pub fn rand_hyper_vertex_cover_mt<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    num_trials: usize,
    seed: u64,
    coverset: &HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default + Send + Sync,
{
    use rayon::prelude::*;

    let results: Vec<(HashSet<String>, W)> = (0..num_trials)
        .into_par_iter()
        .map(|t| {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed + t as u64);
            rand_hyper_vertex_cover_trial(netlist, weight, coverset, &mut rng)
        })
        .collect();

    results
        .into_iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((HashSet::new(), W::default()))
}

/// Single-threaded fallback for multi-trial vertex cover.
///
/// Runs `num_trials` independent trials sequentially and returns the best cover.
pub fn rand_hyper_vertex_cover_mt_seq<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    num_trials: usize,
    seed: u64,
    coverset: &HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default,
{
    let mut best: Option<(HashSet<String>, W)> = None;

    for t in 0..num_trials {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed + t as u64);
        let result = rand_hyper_vertex_cover_trial(netlist, weight, coverset, &mut rng);
        match &best {
            Some((_, best_cost)) if result.1 >= *best_cost => {}
            _ => best = Some(result),
        }
    }

    best.unwrap_or_else(|| (HashSet::new(), W::default()))
}

/// Pitt's randomized algorithm for minimum weighted vertex cover on a regular graph.
///
/// For each uncovered edge (u, v), selects u with probability w(v)/(w(u)+w(v))
/// and v with probability w(u)/(w(u)+w(v)). Then applies reverse-delete
/// post-processing to remove redundant vertices.
///
/// Ported from Python `rand_vertex_cover()` in `rand_cover.py`.
pub fn rand_vertex_cover_trial<W, R>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    coverset: &HashSet<String>,
    rng: &mut R,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default,
    R: rand::Rng,
{
    let mut soln: HashSet<String> = coverset.iter().cloned().collect();
    let mut added_order: Vec<String> = Vec::new();

    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        if soln.contains(u) || soln.contains(v) {
            continue;
        }
        let w_u: f64 = (*weight.get(u).unwrap_or(&W::default())).into();
        let w_v: f64 = (*weight.get(v).unwrap_or(&W::default())).into();
        let threshold = w_v / (w_u + w_v);
        let chosen = if rng.gen::<f64>() < threshold { u } else { v };
        soln.insert(chosen.clone());
        added_order.push(chosen.clone());
    }

    // Reverse-delete post-processing
    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        let mut valid = true;
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            if !soln.contains(u) && !soln.contains(v) {
                valid = false;
                break;
            }
        }
        if !valid {
            soln.insert(vtx.clone());
        }
    }

    let total_cost: W = soln
        .iter()
        .map(|v| weight.get(v).copied().unwrap_or(W::default()))
        .fold(W::default(), |acc, w| acc + w);

    (soln, total_cost)
}

/// Convenience wrapper for `rand_vertex_cover_trial` with seedable RNG.
pub fn rand_vertex_cover<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    seed: u64,
    coverset: &HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default,
{
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    rand_vertex_cover_trial(grph, weight, coverset, &mut rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::UnGraph;

    fn create_weighted_graph() -> (Netlist, HashMap<String, i32>) {
        let mut netlist = Netlist::new();
        netlist.add_module("v0".to_string()).unwrap();
        netlist.add_module("v1".to_string()).unwrap();
        netlist.add_module("v2".to_string()).unwrap();
        netlist.add_net("n0".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_edge("n0", "v1").unwrap();
        netlist.add_edge("n0", "v2").unwrap();
        netlist.add_edge("n1", "v0").unwrap();
        netlist.add_edge("n1", "v1").unwrap();

        let mut weight = HashMap::new();
        weight.insert("v0".to_string(), 1);
        weight.insert("v1".to_string(), 1);
        weight.insert("v2".to_string(), 1);
        (netlist, weight)
    }

    #[test]
    fn test_rand_hyper_vertex_cover_simple() {
        let (hyprgraph, weight) = create_weighted_graph();
        let coverset = HashSet::new();
        let (soln, cost) = rand_hyper_vertex_cover(&hyprgraph, &weight, 42, &coverset);
        for net in &hyprgraph.nets {
            let modules = hyprgraph.get_net_modules(net);
            let covered = modules.iter().any(|m| soln.contains(m));
            assert!(covered, "Net {} is not covered", net);
        }
        assert!(cost >= 1);
    }

    #[test]
    fn test_rand_hyper_vertex_cover_empty() {
        let hyprgraph = Netlist::new();
        let weight: HashMap<String, i32> = HashMap::new();
        let coverset = HashSet::new();
        let (soln, cost) = rand_hyper_vertex_cover(&hyprgraph, &weight, 0, &coverset);
        assert!(soln.is_empty());
        assert_eq!(cost, 0);
    }

    #[test]
    fn test_rand_hyper_vertex_cover_deterministic() {
        let mut netlist = Netlist::new();
        netlist.add_module("v0".to_string()).unwrap();
        netlist.add_module("v1".to_string()).unwrap();
        netlist.add_module("v2".to_string()).unwrap();
        netlist.add_net("n0".to_string()).unwrap();
        netlist.add_edge("n0", "v0").unwrap();
        netlist.add_edge("n0", "v1").unwrap();
        netlist.add_edge("n0", "v2").unwrap();

        let mut weight = HashMap::new();
        weight.insert("v0".to_string(), 1);
        weight.insert("v1".to_string(), 1);
        weight.insert("v2".to_string(), 1);

        let coverset = HashSet::new();
        let (soln1, cost1) = rand_hyper_vertex_cover(&netlist, &weight, 99, &coverset);
        let (soln2, cost2) = rand_hyper_vertex_cover(&netlist, &weight, 99, &coverset);
        assert_eq!(soln1, soln2);
        assert_eq!(cost1, cost2);
    }

    #[test]
    fn test_rand_hyper_vertex_cover_mt_seq_simple() {
        let (hyprgraph, weight) = create_weighted_graph();
        let coverset = HashSet::new();
        let (soln, cost) = rand_hyper_vertex_cover_mt_seq(&hyprgraph, &weight, 16, 42, &coverset);
        for net in &hyprgraph.nets {
            let modules = hyprgraph.get_net_modules(net);
            let covered = modules.iter().any(|m| soln.contains(m));
            assert!(covered, "Net {} is not covered", net);
        }
        assert!(cost >= 1);
    }

    #[test]
    fn test_rand_hyper_vertex_cover_mt_seq_weighted() {
        let mut netlist = Netlist::new();
        netlist.add_module("v0".to_string()).unwrap();
        netlist.add_module("v1".to_string()).unwrap();
        netlist.add_net("n0".to_string()).unwrap();
        netlist.add_edge("n0", "v0").unwrap();
        netlist.add_edge("n0", "v1").unwrap();

        let mut weight = HashMap::new();
        weight.insert("v0".to_string(), 100);
        weight.insert("v1".to_string(), 1);

        let coverset = HashSet::new();
        let (soln, cost) = rand_hyper_vertex_cover_mt_seq(&netlist, &weight, 128, 7, &coverset);
        assert_eq!(soln.len(), 1);
        assert!(soln.contains("v1"));
        assert_eq!(cost, 1);
    }

    #[test]
    fn test_rand_vertex_cover_simple() {
        let mut grph = UnGraph::new_undirected();
        let n0 = grph.add_node("v0".to_string());
        let n1 = grph.add_node("v1".to_string());
        let n2 = grph.add_node("v2".to_string());
        grph.add_edge(n0, n1, ());
        grph.add_edge(n1, n2, ());
        grph.add_edge(n0, n2, ());

        let mut weight = HashMap::new();
        weight.insert("v0".to_string(), 1i32);
        weight.insert("v1".to_string(), 1);
        weight.insert("v2".to_string(), 1);

        let coverset = HashSet::new();
        let (soln, _cost) = rand_vertex_cover(&grph, &weight, 42, &coverset);
        // Verify cover
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(soln.contains(u) || soln.contains(v));
        }
    }

    #[test]
    fn test_rand_vertex_cover_weighted() {
        let mut grph = UnGraph::new_undirected();
        let n0 = grph.add_node("v0".to_string());
        let n1 = grph.add_node("v1".to_string());
        grph.add_edge(n0, n1, ());

        let mut weight = HashMap::new();
        weight.insert("v0".to_string(), 100i32);
        weight.insert("v1".to_string(), 1);

        let coverset = HashSet::new();
        let (soln, cost) = rand_vertex_cover(&grph, &weight, 42, &coverset);
        assert!(soln.contains("v1"));
        assert_eq!(cost, 1);
    }

    #[test]
    fn test_rand_hyper_vertex_cover_empty_net() {
        let mut hyprgraph = Netlist::new();
        hyprgraph.add_module("m1".to_string()).unwrap();
        hyprgraph.add_net("isolated".to_string()).unwrap();
        let mut weight = HashMap::new();
        weight.insert("m1".to_string(), 1i32);
        let coverset = HashSet::new();
        let (soln, cost) = rand_hyper_vertex_cover(&hyprgraph, &weight, 42, &coverset);
        assert_eq!(cost, 0);
        assert!(soln.is_empty());
    }

    #[test]
    fn test_rand_vertex_cover_empty_graph() {
        let grph = UnGraph::<String, ()>::new_undirected();
        let weight: HashMap<String, i32> = HashMap::new();
        let coverset = HashSet::new();
        let (soln, cost) = rand_vertex_cover(&grph, &weight, 0, &coverset);
        assert!(soln.is_empty());
        assert_eq!(cost, 0);
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn test_rand_hyper_vertex_cover_mt_simple() {
        let (hyprgraph, weight) = create_weighted_graph();
        let coverset = HashSet::new();
        let (soln, cost) = rand_hyper_vertex_cover_mt(&hyprgraph, &weight, 8, 42, &coverset);
        for net in &hyprgraph.nets {
            let modules = hyprgraph.get_net_modules(net);
            let covered = modules.iter().any(|m| soln.contains(m));
            assert!(covered, "Net {} is not covered", net);
        }
        assert!(cost >= 1);
    }
}
