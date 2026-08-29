use std::collections::HashSet;

use crate::netlist::Netlist;

/// Primal-dual approximation algorithm for covering problems.
///
/// Implements the primal-dual paradigm for set cover:
///
/// $$ \min \sum_{v \in C} w(v) \quad \text{s.t.} \quad C \cap S \neq \varnothing \; \forall S \in \mathcal{V} $$
///
/// where $\mathcal{V}$ is the set of violating sets and $w(v)$ are vertex weights.
///
/// Generic framework that works with any violate function that produces
/// sets of vertices, a weight function for vertices, and a solution set.
/// The `soln` parameter is the current solution set modified in-place.
pub fn pd_cover<F, W>(
    mut violate: F,
    weight: &[W],
    soln: &mut HashSet<usize>,
) -> (HashSet<usize>, W)
where
    F: FnMut(&HashSet<usize>) -> Vec<Vec<usize>>,
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut gap: Vec<W> = weight.to_vec();
    let mut added_order: Vec<usize> = Vec::new();

    for violate_set in violate(soln) {
        if violate_set.is_empty() {
            continue;
        }

        let min_vtx = violate_set
            .iter()
            .min_by(|&v1, &v2| {
                let g1 = gap[*v1];
                let g2 = gap[*v2];
                g1.partial_cmp(&g2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
            .expect("violate_set should not be empty");

        let min_val = gap[min_vtx];

        if !soln.contains(&min_vtx) {
            soln.insert(min_vtx);
            added_order.push(min_vtx);
        }

        for vtx in &violate_set {
            gap[*vtx] = gap[*vtx] - min_val;
        }
    }

    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        let violates = violate(soln);
        let any_violated = violates.iter().any(|s| !s.is_empty());
        if any_violated {
            soln.insert(*vtx);
        }
    }

    let final_primal_cost: W = soln
        .iter()
        .map(|vtx| weight[*vtx])
        .fold(W::default(), |acc, w| acc + w);

    (soln.clone(), final_primal_cost)
}

/// Minimum weighted hypergraph vertex cover using primal-dual approximation.
///
/// Ported from C++ `min_hyper_vertex_cover()` in `cover.hpp`.
pub fn min_hyper_vertex_cover<W>(
    netlist: &Netlist,
    weight: &[W],
    coverset: &mut HashSet<usize>,
) -> (HashSet<usize>, W)
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let violate_fn = |current_soln: &HashSet<usize>| -> Vec<Vec<usize>> {
        let mut result = Vec::new();
        for net in netlist.net_indices() {
            let modules = netlist.get_net_modules(net);
            let covered = modules.iter().any(|m| current_soln.contains(m));
            if !covered {
                result.push(modules);
            }
        }
        result
    };

    pd_cover(violate_fn, weight, coverset)
}

/// Convenience overload that creates an empty coverset.
pub fn min_hyper_vertex_cover_new<W>(netlist: &Netlist, weight: &[W]) -> (HashSet<usize>, W)
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut coverset = HashSet::new();
    min_hyper_vertex_cover(netlist, weight, &mut coverset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::Netlist;

    fn create_simple_hypergraph() -> Netlist {
        let mut netlist = Netlist::new();
        netlist.add_module("v0".to_string()).unwrap();
        netlist.add_module("v1".to_string()).unwrap();
        netlist.add_module("v2".to_string()).unwrap();
        let n0 = netlist.add_net("n0".to_string()).unwrap();
        let n1 = netlist.add_net("n1".to_string()).unwrap();
        netlist.add_edge(n0, 1).unwrap();
        netlist.add_edge(n0, 2).unwrap();
        netlist.add_edge(n1, 0).unwrap();
        netlist.add_edge(n1, 1).unwrap();
        netlist
    }

    #[inline]
    fn default_weight(netlist: &Netlist) -> Vec<i32> {
        vec![1; netlist.num_modules()]
    }

    fn assert_all_nets_covered(hyprgraph: &Netlist, covered: &HashSet<usize>) {
        for net in hyprgraph.net_indices() {
            let modules = hyprgraph.get_net_modules(net);
            let net_covered = modules.iter().any(|m| covered.contains(m));
            assert!(net_covered, "Net {} is not covered", net);
        }
    }

    #[test]
    fn test_min_hyper_vertex_cover() {
        let hyprgraph = create_simple_hypergraph();
        let weight = default_weight(&hyprgraph);

        let mut coverset = HashSet::new();
        let (covered, _cost) = min_hyper_vertex_cover(&hyprgraph, &weight, &mut coverset);
        assert_all_nets_covered(&hyprgraph, &covered);
        assert!(!covered.is_empty());
    }

    #[test]
    fn test_min_hyper_vertex_cover_new() {
        let hyprgraph = create_simple_hypergraph();
        let weight = default_weight(&hyprgraph);

        let (covered, _cost) = min_hyper_vertex_cover_new(&hyprgraph, &weight);
        assert_all_nets_covered(&hyprgraph, &covered);
        assert!(!covered.is_empty());
    }

    #[test]
    fn test_min_hyper_vertex_cover_empty() {
        let hyprgraph = Netlist::new();
        let weight: Vec<i32> = Vec::new();
        let mut coverset = HashSet::new();
        let (covered, cost) = min_hyper_vertex_cover(&hyprgraph, &weight, &mut coverset);
        assert!(covered.is_empty());
        assert_eq!(cost, 0);
    }

    #[test]
    fn test_min_hyper_vertex_cover_with_coverset() {
        let hyprgraph = create_simple_hypergraph();
        let weight = default_weight(&hyprgraph);
        let mut coverset: HashSet<usize> = [0].iter().copied().collect();
        let (covered, _cost) = min_hyper_vertex_cover(&hyprgraph, &weight, &mut coverset);
        assert_all_nets_covered(&hyprgraph, &covered);
        assert!(covered.contains(&0));
    }
}
