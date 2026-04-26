//! Netlist statistics and metrics
//!
//! This module provides utilities for computing various statistics and metrics
//! about netlists, useful for analysis and characterization.

use crate::netlist::Netlist;
use indexmap::IndexMap;

/// Statistics about a netlist, including module and net degree metrics.
///
/// This struct is created by [`NetlistStats::analyze`] and provides various
/// statistical measures about the netlist structure.
#[derive(Debug, Clone)]
pub struct NetlistStats {
    /// Total number of modules
    pub num_modules: usize,
    /// Total number of nets
    pub num_nets: usize,
    /// Number of pins (total connections)
    pub num_pins: usize,
    /// Average degree of modules
    pub avg_module_degree: f64,
    /// Maximum degree of modules
    pub max_module_degree: usize,
    /// Minimum degree of modules
    pub min_module_degree: usize,
    /// Average degree of nets
    pub avg_net_degree: f64,
    /// Maximum degree of nets
    pub max_net_degree: usize,
    /// Minimum degree of nets
    pub min_net_degree: usize,
    /// Degree distribution of modules
    pub module_degree_distribution: IndexMap<usize, usize>,
    /// Degree distribution of nets
    pub net_degree_distribution: IndexMap<usize, usize>,
}

impl NetlistStats {
    /// Compute statistics for a netlist
    pub fn analyze(netlist: &Netlist) -> Self {
        let mut module_degrees: Vec<usize> = Vec::with_capacity(netlist.num_modules());
        let mut net_degrees: Vec<usize> = Vec::with_capacity(netlist.num_nets());
        let mut num_pins = 0;

        // Compute module degrees
        for module in &netlist.modules {
            let degree = netlist.get_module_degree(module);
            module_degrees.push(degree);
        }

        // Compute net degrees
        for net in &netlist.nets {
            let degree = netlist.get_net_degree(net);
            net_degrees.push(degree);
            num_pins += degree;
        }

        // Compute average, max, min module degree
        let avg_module_degree = if !module_degrees.is_empty() {
            module_degrees.iter().sum::<usize>() as f64 / module_degrees.len() as f64
        } else {
            0.0
        };
        let max_module_degree = *module_degrees.iter().max().unwrap_or(&0);
        let min_module_degree = *module_degrees.iter().min().unwrap_or(&0);

        // Compute average, max, min net degree
        let avg_net_degree = if !net_degrees.is_empty() {
            net_degrees.iter().sum::<usize>() as f64 / net_degrees.len() as f64
        } else {
            0.0
        };
        let max_net_degree = *net_degrees.iter().max().unwrap_or(&0);
        let min_net_degree = *net_degrees.iter().min().unwrap_or(&0);

        // Compute degree distributions
        let mut module_degree_distribution = IndexMap::new();
        for degree in module_degrees {
            *module_degree_distribution.entry(degree).or_insert(0) += 1;
        }

        let mut net_degree_distribution = IndexMap::new();
        for degree in net_degrees {
            *net_degree_distribution.entry(degree).or_insert(0) += 1;
        }

        NetlistStats {
            num_modules: netlist.num_modules(),
            num_nets: netlist.num_nets(),
            num_pins,
            avg_module_degree,
            max_module_degree,
            min_module_degree,
            avg_net_degree,
            max_net_degree,
            min_net_degree,
            module_degree_distribution,
            net_degree_distribution,
        }
    }

    /// Get the average module degree
    pub fn avg_module_degree(&self) -> f64 {
        self.avg_module_degree
    }

    /// Get the maximum module degree
    pub fn max_module_degree(&self) -> usize {
        self.max_module_degree
    }

    /// Get the average net degree
    pub fn avg_net_degree(&self) -> f64 {
        self.avg_net_degree
    }

    /// Get the maximum net degree
    pub fn max_net_degree(&self) -> usize {
        self.max_net_degree
    }

    /// Get the pin-to-module ratio
    pub fn pin_module_ratio(&self) -> f64 {
        if self.num_modules > 0 {
            self.num_pins as f64 / self.num_modules as f64
        } else {
            0.0
        }
    }

    /// Get the pin-to-net ratio
    pub fn pin_net_ratio(&self) -> f64 {
        if self.num_nets > 0 {
            self.num_pins as f64 / self.num_nets as f64
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::Netlist;

    #[test]
    fn test_analyze_empty_netlist() {
        let netlist = Netlist::new();
        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.num_modules, 0);
        assert_eq!(stats.num_nets, 0);
        assert_eq!(stats.num_pins, 0);
    }

    #[test]
    fn test_analyze_simple_netlist() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.num_modules, 3);
        assert_eq!(stats.num_nets, 2);
        assert_eq!(stats.num_pins, 4);
        assert_eq!(stats.max_module_degree, 2);
        assert_eq!(stats.max_net_degree, 2);
    }

    #[test]
    fn test_stats_accessor_max_module_degree() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.max_module_degree(), 2);
    }

    #[test]
    fn test_stats_accessor_max_net_degree() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.max_net_degree(), 2);
    }

    #[test]
    fn test_stats_pin_module_ratio() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        let ratio = stats.pin_module_ratio();
        assert!((ratio - 4.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_stats_pin_net_ratio() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.pin_net_ratio(), 2.0);
    }

    #[test]
    fn test_stats_pin_module_ratio_empty() {
        let netlist = Netlist::new();
        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.pin_module_ratio(), 0.0);
    }

    #[test]
    fn test_stats_pin_net_ratio_empty() {
        let netlist = Netlist::new();
        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.pin_net_ratio(), 0.0);
    }

    #[test]
    fn test_stats_avg_module_degree() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        assert!((stats.avg_module_degree() - 1.333) < 0.001);
    }

    #[test]
    fn test_stats_min_degree() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let stats = NetlistStats::analyze(&netlist);
        assert_eq!(stats.min_module_degree, 1);
        assert_eq!(stats.min_net_degree, 2);
    }
}
