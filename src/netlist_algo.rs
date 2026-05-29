use std::collections::HashMap;
use std::collections::HashSet;

use crate::netlist::Netlist;

/// Minimum weighted vertex cover for netlist hypergraphs using primal-dual paradigm.
///
/// Iterates over all nets, selecting the module with minimum gap (modified weight)
/// to cover each uncovered net.
///
/// Ported from C++ `min_vertex_cover()` in `netlist_algo.hpp`.
pub fn min_vertex_cover<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    coverset: &mut HashSet<String>,
) -> W
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut gap: HashMap<String, W> = weight.clone();
    let mut total_dual_cost: W = W::default();
    let mut total_primal_cost: W = W::default();

    for net in &netlist.nets {
        let modules = netlist.get_net_modules(net);
        let already_covered = modules.iter().any(|m| coverset.contains(m));
        if already_covered {
            continue;
        }

        let min_vtx = modules
            .iter()
            .min_by(|&v1, &v2| {
                let g1 = gap.get(v1).copied().unwrap_or(weight[v1]);
                let g2 = gap.get(v2).copied().unwrap_or(weight[v2]);
                g1.partial_cmp(&g2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .expect("net with no modules should not happen");

        let min_val = gap[&min_vtx];
        coverset.insert(min_vtx.clone());
        total_primal_cost = total_primal_cost + weight[&min_vtx];
        total_dual_cost = total_dual_cost + min_val;

        for vtx in &modules {
            if let Some(g) = gap.get_mut(vtx) {
                *g = *g - min_val;
            }
        }
    }

    total_primal_cost
}

/// Minimum weighted maximal matching for netlist hypergraphs.
///
/// Implements a primal-dual approximation algorithm. Selects nets greedily
/// avoiding conflicts (shared vertices), maintaining a dependency set.
///
/// Ported from C++ `min_maximal_matching()` in `netlist_algo.hpp` / `netlist_algo.cpp`.
pub fn min_maximal_matching<W>(
    netlist: &Netlist,
    weight: &HashMap<String, W>,
    matchset: &mut HashSet<String>,
    dep: &mut HashSet<String>,
) -> W
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut gap: HashMap<String, W> = weight.clone();
    let mut total_dual_cost: W = W::default();
    let mut total_primal_cost: W = W::default();

    for net in &netlist.nets {
        let modules_in_net = netlist.get_net_modules(net);

        let net_has_dep = modules_in_net.iter().any(|m| dep.contains(m));
        if net_has_dep {
            continue;
        }

        if matchset.contains(net) {
            cover_dep(netlist, net, dep);
            continue;
        }

        let mut min_val = gap[net];
        let mut min_net = net.clone();

        for m in &modules_in_net {
            for net2 in &netlist.get_module_nets(m) {
                if !dep.contains(net2) && gap.contains_key(net2) && gap[net2] < min_val {
                    min_val = gap[net2];
                    min_net = net2.clone();
                }
            }
        }

        cover_dep(netlist, &min_net, dep);
        matchset.insert(min_net.clone());
        total_primal_cost = total_primal_cost + weight[&min_net];
        total_dual_cost = total_dual_cost + min_val;

        if &min_net != net {
            if let Some(g) = gap.get_mut(net) {
                *g = *g - min_val;
            }
            for m in &modules_in_net {
                for net2 in &netlist.get_module_nets(m) {
                    if let Some(g) = gap.get_mut(net2) {
                        *g = *g - min_val;
                    }
                }
            }
        }
    }

    total_primal_cost
}

/// Convenience version that creates empty matchset and dep sets.
pub fn min_maximal_matching_new<W>(
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
    let mut matchset = HashSet::new();
    let mut dep = HashSet::new();
    let cost = min_maximal_matching(netlist, weight, &mut matchset, &mut dep);
    (matchset, cost)
}

fn cover_dep(netlist: &Netlist, net: &str, dep: &mut HashSet<String>) {
    for m in netlist.get_net_modules(net) {
        dep.insert(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::Netlist;

    fn create_dwarf_netlist() -> Netlist {
        let mut netlist = Netlist::new();
        for i in 0..7 {
            netlist.add_module(format!("mod{}", i)).unwrap();
        }
        for i in 0..6 {
            netlist.add_net(format!("net{}", i)).unwrap();
        }
        netlist.add_edge("net0", "mod0").unwrap();
        netlist.add_edge("net0", "mod1").unwrap();
        netlist.add_edge("net1", "mod0").unwrap();
        netlist.add_edge("net1", "mod2").unwrap();
        netlist.add_edge("net1", "mod3").unwrap();
        netlist.add_edge("net2", "mod1").unwrap();
        netlist.add_edge("net2", "mod2").unwrap();
        netlist.add_edge("net2", "mod3").unwrap();
        netlist.add_edge("net3", "mod2").unwrap();
        netlist.add_edge("net4", "mod3").unwrap();
        netlist.add_edge("net5", "mod0").unwrap();
        netlist
    }

    fn create_test_netlist() -> Netlist {
        let mut netlist = Netlist::new();
        netlist.add_module("mod0".to_string()).unwrap();
        netlist.add_module("mod1".to_string()).unwrap();
        netlist.add_module("mod2".to_string()).unwrap();
        netlist.add_net("net0".to_string()).unwrap();
        netlist.add_net("net1".to_string()).unwrap();
        netlist.add_net("net2".to_string()).unwrap();
        netlist.add_edge("net0", "mod0").unwrap();
        netlist.add_edge("net0", "mod1").unwrap();
        netlist.add_edge("net1", "mod0").unwrap();
        netlist.add_edge("net1", "mod2").unwrap();
        netlist.add_edge("net2", "mod1").unwrap();
        netlist
    }

    #[test]
    fn test_min_vertex_cover_dwarf() {
        let hyprgraph = create_dwarf_netlist();
        let mut weight = HashMap::new();
        for m in &hyprgraph.modules {
            weight.insert(m.clone(), 1u32);
        }
        let mut coverset = HashSet::new();
        let _cost = min_vertex_cover(&hyprgraph, &weight, &mut coverset);

        for net in &hyprgraph.nets {
            let modules = hyprgraph.get_net_modules(net);
            let net_covered = modules.iter().any(|m| coverset.contains(m));
            assert!(net_covered, "Net {} is not covered", net);
        }
    }

    #[test]
    fn test_min_maximal_matching_dwarf() {
        let hyprgraph = create_dwarf_netlist();
        let mut weight = HashMap::new();
        for n in &hyprgraph.nets {
            weight.insert(n.clone(), 1u32);
        }
        let mut matchset = HashSet::new();
        let mut dep = HashSet::new();
        let _cost = min_maximal_matching(&hyprgraph, &weight, &mut matchset, &mut dep);

        let mut net_covered_by_match = HashSet::new();
        for matched_net in &matchset {
            for m in hyprgraph.get_net_modules(matched_net) {
                net_covered_by_match.insert(m);
            }
        }
        for net in &hyprgraph.nets {
            let modules = hyprgraph.get_net_modules(net);
            let has_covered = modules.iter().any(|m| net_covered_by_match.contains(m));
            assert!(
                has_covered,
                "Net {} shares no vertex with any matched net",
                net
            );
        }
    }

    #[test]
    fn test_min_maximal_matching_consistency() {
        let hyprgraph = create_test_netlist();
        let mut weight = HashMap::new();
        for n in &hyprgraph.nets {
            weight.insert(n.clone(), 1u32);
        }
        let (matchset, _cost) = min_maximal_matching_new(&hyprgraph, &weight);

        assert!(!matchset.is_empty());
        for net in &matchset {
            assert!(
                hyprgraph.nets.contains(net),
                "Matched net {} is not in netlist",
                net
            );
        }
    }
}
