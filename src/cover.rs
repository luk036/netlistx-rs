use std::collections::HashMap;
use std::collections::HashSet;

use crate::netlist::Netlist;

/// Primal-dual approximation algorithm for covering problems.
///
/// Generic framework that works with any violate function that produces
/// sets of vertices, a weight function for vertices, and a solution set.
/// The `coverset` parameter is the current set of covered vertices used
/// to determine which sets are violated.
pub fn pd_cover<F, W>(
    mut violate: F,
    weight: &HashMap<String, W>,
    soln: &mut HashSet<String>,
    coverset: &HashSet<String>,
) -> (HashSet<String>, W)
where
    F: FnMut(&HashSet<String>) -> Vec<Vec<String>>,
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut gap: HashMap<String, W> = HashMap::new();
    let mut added_order: Vec<String> = Vec::new();

    let current_coverset: HashSet<String> = coverset.iter().cloned().collect();

    for violate_set in violate(&current_coverset) {
        if violate_set.is_empty() {
            continue;
        }

        let min_vtx = violate_set
            .iter()
            .min_by(|&v1, &v2| {
                let g1 = gap.get(v1).copied().unwrap_or(weight[v1]);
                let g2 = gap.get(v2).copied().unwrap_or(weight[v2]);
                g1.partial_cmp(&g2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .expect("violate_set should not be empty");

        let min_val = gap.get(&min_vtx).copied().unwrap_or(weight[&min_vtx]);

        if !soln.contains(&min_vtx) {
            soln.insert(min_vtx.clone());
            added_order.push(min_vtx.clone());
        }

        for vtx in &violate_set {
            let entry = gap.entry(vtx.clone()).or_insert(weight[vtx]);
            *entry = *entry - min_val;
        }
    }

    for vtx in added_order.iter().rev() {
        soln.remove(vtx);
        let violates = violate(soln);
        let any_violated = violates.iter().any(|s| !s.is_empty());
        if any_violated {
            soln.insert(vtx.clone());
        }
    }

    let final_primal_cost: W = soln
        .iter()
        .map(|vtx| weight[vtx])
        .fold(W::default(), |acc, w| acc + w);

    (soln.clone(), final_primal_cost)
}

/// Minimum weighted hypergraph vertex cover using primal-dual approximation.
///
/// Ported from C++ `min_hyper_vertex_cover()` in `cover.hpp`.
pub fn min_hyper_vertex_cover<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    coverset: &mut HashSet<String>,
) -> (HashSet<String>, W)
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let violate_fn = |current_soln: &HashSet<String>| -> Vec<Vec<String>> {
        let mut result = Vec::new();
        for net in &netlist.nets {
            let modules = netlist.get_net_modules(net);
            let covered = modules.iter().any(|m| current_soln.contains(m));
            if !covered {
                result.push(modules);
            }
        }
        result
    };

    let initial_coverset = coverset.clone();
    pd_cover(violate_fn, weight, coverset, &initial_coverset)
}

/// Convenience overload that creates an empty coverset.
pub fn min_hyper_vertex_cover_new<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
) -> (HashSet<String>, W)
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
        netlist.add_net("n0".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_edge("n0", "v1").unwrap();
        netlist.add_edge("n0", "v2").unwrap();
        netlist.add_edge("n1", "v0").unwrap();
        netlist.add_edge("n1", "v1").unwrap();
        netlist
    }

    fn default_weight(netlist: &Netlist) -> HashMap<String, i32> {
        let mut w = HashMap::new();
        for m in &netlist.modules {
            w.insert(m.clone(), 1);
        }
        w
    }

    fn assert_all_nets_covered(hyprgraph: &Netlist, covered: &HashSet<String>) {
        for net in &hyprgraph.nets {
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
        let weight: HashMap<String, i32> = HashMap::new();
        let mut coverset = HashSet::new();
        let (covered, cost) = min_hyper_vertex_cover(&hyprgraph, &weight, &mut coverset);
        assert!(covered.is_empty());
        assert_eq!(cost, 0);
    }

    #[test]
    fn test_min_hyper_vertex_cover_with_coverset() {
        let hyprgraph = create_simple_hypergraph();
        let weight = default_weight(&hyprgraph);
        let mut coverset: HashSet<String> = [("v0".to_string())].iter().cloned().collect();
        let (covered, _cost) = min_hyper_vertex_cover(&hyprgraph, &weight, &mut coverset);
        assert_all_nets_covered(&hyprgraph, &covered);
        assert!(covered.contains("v0"));
    }
}
