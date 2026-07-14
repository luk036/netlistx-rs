use std::collections::HashSet;

use crate::netlist::Netlist;

/// Minimum weighted vertex cover for hypergraphs using primal-dual paradigm.
///
/// Modules are identified by `usize` indices (0..num_modules), matching C++/Python.
/// Weights are stored in a slice indexed by module index.
/// The cover set contains module indices.
pub fn min_vertex_cover<W>(
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
    let mut gap: Vec<W> = weight.to_vec();
    let mut total_dual_cost: W = W::default();
    let mut total_primal_cost: W = W::default();

    for net in netlist.net_indices() {
        let modules = netlist.get_net_modules(net);
        let already_covered = modules.iter().any(|m| coverset.contains(m));
        if already_covered {
            continue;
        }

        let min_vtx = *modules
            .iter()
            .min_by(|&v1, &v2| {
                let g1 = gap[*v1];
                let g2 = gap[*v2];
                g1.partial_cmp(&g2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("net with no modules should not happen");

        let min_val = gap[min_vtx];
        coverset.insert(min_vtx);
        total_primal_cost = total_primal_cost + weight[min_vtx];
        total_dual_cost = total_dual_cost + min_val;

        for &vtx in &modules {
            let g = &mut gap[vtx];
            *g = if *g > min_val {
                *g - min_val
            } else {
                W::default()
            };
        }
    }

    (coverset.clone(), total_primal_cost)
}

/// Minimum weighted maximal matching for hypergraphs.
///
/// Nets are identified by `usize` indices (0..num_nets).
/// Weights are stored in a slice indexed by net index.
pub fn min_maximal_matching<W>(
    netlist: &Netlist,
    weight: &[W],
    matchset: &mut HashSet<usize>,
    dep: &mut HashSet<usize>,
) -> (HashSet<usize>, W)
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut gap: Vec<W> = weight.to_vec();
    let mut total_dual_cost: W = W::default();
    let mut total_primal_cost: W = W::default();

    for net in netlist.net_indices() {
        let modules_in_net = netlist.get_net_modules(net);

        let net_has_dep = modules_in_net.iter().any(|m| dep.contains(m));
        if net_has_dep {
            continue;
        }

        if matchset.contains(&net) {
            cover_dep(netlist, net, dep);
            continue;
        }

        let mut min_val = gap[net];
        let mut min_net = net;

        for &m in &modules_in_net {
            for &net2 in &netlist.get_module_nets(m) {
                let g_val = gap[net2];
                if !dep.contains(&net2) && g_val < min_val {
                    min_val = g_val;
                    min_net = net2;
                }
            }
        }

        cover_dep(netlist, min_net, dep);
        matchset.insert(min_net);
        total_primal_cost = total_primal_cost + weight[min_net];
        total_dual_cost = total_dual_cost + min_val;

        if min_net != net {
            gap[net] = if gap[net] > min_val {
                gap[net] - min_val
            } else {
                W::default()
            };
            for &m in &modules_in_net {
                for &net2 in &netlist.get_module_nets(m) {
                    gap[net2] = if gap[net2] > min_val {
                        gap[net2] - min_val
                    } else {
                        W::default()
                    };
                }
            }
        }
    }

    (matchset.clone(), total_primal_cost)
}

/// Convenience version that creates empty matchset and dep sets.
pub fn min_maximal_matching_new<W>(netlist: &Netlist, weight: &[W]) -> (HashSet<usize>, W)
where
    W: Copy
        + std::ops::Add<Output = W>
        + std::ops::Sub<Output = W>
        + std::cmp::PartialOrd
        + Default,
{
    let mut matchset = HashSet::new();
    let mut dep = HashSet::new();
    min_maximal_matching(netlist, weight, &mut matchset, &mut dep)
}

fn cover_dep(netlist: &Netlist, net: usize, dep: &mut HashSet<usize>) {
    for m in netlist.get_net_modules(net) {
        dep.insert(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::Netlist;

    fn create_dwarf_netlist() -> Netlist {
        let mut nl = Netlist::new();
        for i in 0..7 {
            nl.add_module(format!("mod{}", i)).unwrap();
        }
        for i in 0..6 {
            nl.add_net(format!("net{}", i)).unwrap();
        }
        nl.add_edge(0, 0).unwrap(); // net0-mod0
        nl.add_edge(0, 1).unwrap();
        nl.add_edge(1, 0).unwrap();
        nl.add_edge(1, 2).unwrap();
        nl.add_edge(1, 3).unwrap();
        nl.add_edge(2, 1).unwrap();
        nl.add_edge(2, 2).unwrap();
        nl.add_edge(2, 3).unwrap();
        nl.add_edge(3, 2).unwrap();
        nl.add_edge(4, 3).unwrap();
        nl.add_edge(5, 0).unwrap();
        nl
    }

    fn create_test_netlist() -> Netlist {
        let mut nl = Netlist::new();
        nl.add_module("mod0".to_string()).unwrap();
        nl.add_module("mod1".to_string()).unwrap();
        nl.add_module("mod2".to_string()).unwrap();
        nl.add_net("net0".to_string()).unwrap();
        nl.add_net("net1".to_string()).unwrap();
        nl.add_net("net2".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        nl.add_edge(0, 1).unwrap();
        nl.add_edge(1, 0).unwrap();
        nl.add_edge(1, 2).unwrap();
        nl.add_edge(2, 1).unwrap();
        nl
    }

    #[test]
    fn test_min_vertex_cover_dwarf() {
        let h = create_dwarf_netlist();
        let weight: Vec<u32> = (0..h.num_modules).map(|_| 1u32).collect();
        let mut coverset = HashSet::new();
        let _cost = min_vertex_cover(&h, &weight, &mut coverset);

        for net in h.net_indices() {
            let modules = h.get_net_modules(net);
            let covered = modules.iter().any(|m| coverset.contains(m));
            assert!(covered, "Net {} is not covered", net);
        }
    }

    #[test]
    fn test_min_maximal_matching_dwarf() {
        let h = create_dwarf_netlist();
        let weight: Vec<u32> = (0..h.num_nets).map(|_| 1u32).collect();
        let mut matchset = HashSet::new();
        let mut dep = HashSet::new();
        let _cost = min_maximal_matching(&h, &weight, &mut matchset, &mut dep);

        let mut covered_by_match: HashSet<usize> = HashSet::new();
        for &matched_net in &matchset {
            for m in h.get_net_modules(matched_net) {
                covered_by_match.insert(m);
            }
        }
        for net in h.net_indices() {
            let modules = h.get_net_modules(net);
            let has_covered = modules.iter().any(|m| covered_by_match.contains(m));
            assert!(
                has_covered,
                "Net {} shares no vertex with any matched net",
                net
            );
        }
    }

    #[test]
    fn test_min_maximal_matching_consistency() {
        let h = create_test_netlist();
        let weight: Vec<u32> = (0..h.num_nets).map(|_| 1u32).collect();
        let (matchset, _cost) = min_maximal_matching_new(&h, &weight);

        assert!(!matchset.is_empty());
        for &net in &matchset {
            assert!(net < h.num_nets, "Matched net {} is not in netlist", net);
        }
    }
}
