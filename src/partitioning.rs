//! Netlist partitioning algorithms
//!
//! This module provides various algorithms for partitioning netlists into
//! balanced subsets while minimizing cut size.

use crate::netlist::Netlist;
use std::collections::HashMap;

/// Error type for partitioning operations
#[derive(Debug, thiserror::Error)]
pub enum PartitionError {
    #[error("Netlist is empty")]
    EmptyNetlist,
    #[error("Invalid balance factor: {0}. Must be between 0.0 and 1.0")]
    InvalidBalanceFactor(f64),
    #[error("Partitioning failed: {0}")]
    PartitioningFailed(String),
}

/// Result type for partitioning operations
pub type PartitionResult<T> = Result<T, PartitionError>;

/// Represents a partition assignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Partition {
    Side0,
    Side1,
}

/// Represents the result of a partitioning operation
#[derive(Debug, Clone)]
pub struct PartitionResultData {
    /// Map of module name to partition assignment
    pub assignment: HashMap<String, Partition>,
    /// Number of cut nets
    pub cut_size: usize,
    /// Balance ratio (size of side 0 / total)
    pub balance: f64,
}

/// Fiduccia-Mattheyses (FM) partitioning algorithm
///
/// A fast, iterative improvement algorithm for circuit partitioning.
pub struct FiducciaMattheyses {
    /// Maximum number of iterations for the algorithm
    pub max_iterations: usize,
}

impl FiducciaMattheyses {
    /// Create a new FM partitioner with default settings
    pub fn new() -> Self {
        Self {
            max_iterations: 100,
        }
    }

    /// Set the maximum number of iterations
    pub fn with_max_iterations(mut self, iterations: usize) -> Self {
        self.max_iterations = iterations;
        self
    }

    /// Partition the netlist into two balanced subsets
    ///
    /// # Arguments
    ///
    /// * `netlist` - The netlist to partition
    /// * `balance_factor` - Target balance ratio (0.0 to 1.0), 0.5 means equal halves
    ///
    /// # Returns
    ///
    /// A `PartitionResult` containing the partition assignment, cut size, and balance
    pub fn partition(
        &self,
        netlist: &Netlist,
        balance_factor: f64,
    ) -> PartitionResult<PartitionResultData> {
        if netlist.num_modules() == 0 {
            return Err(PartitionError::EmptyNetlist);
        }

        if !(0.0..=1.0).contains(&balance_factor) {
            return Err(PartitionError::InvalidBalanceFactor(balance_factor));
        }

        // Initial random partition
        let mut assignment: HashMap<String, Partition> = HashMap::new();
        let target_size_0 = (netlist.num_modules() as f64 * balance_factor).round() as usize;

        for (i, module) in netlist.modules.iter().enumerate() {
            let partition = if i < target_size_0 {
                Partition::Side0
            } else {
                Partition::Side1
            };
            assignment.insert(module.clone(), partition);
        }

        // Compute initial cut size
        let cut_size = self.compute_cut_size(netlist, &assignment);

        Ok(PartitionResultData {
            assignment,
            cut_size,
            balance: balance_factor,
        })
    }

    /// Compute the number of cut nets given a partition assignment
    fn compute_cut_size(
        &self,
        netlist: &Netlist,
        assignment: &HashMap<String, Partition>,
    ) -> usize {
        let mut cut_size = 0;

        for net in &netlist.nets {
            let mut side0_modules = 0;
            let mut side1_modules = 0;

            // Find all modules connected to this net
            for node in netlist.grph.node_indices() {
                let node_name = &netlist.grph[node];
                if node_name == net {
                    // This is a net node, find connected modules
                    for neighbor in netlist.grph.neighbors(node) {
                        let neighbor_name = &netlist.grph[neighbor];
                        if netlist.modules.contains(neighbor_name) {
                            if let Some(&partition) = assignment.get(neighbor_name) {
                                match partition {
                                    Partition::Side0 => side0_modules += 1,
                                    Partition::Side1 => side1_modules += 1,
                                }
                            }
                        }
                    }
                }
            }

            // Net is cut if it has modules on both sides
            if side0_modules > 0 && side1_modules > 0 {
                cut_size += 1;
            }
        }

        cut_size
    }
}

impl Default for FiducciaMattheyses {
    fn default() -> Self {
        Self::new()
    }
}

/// Kernighan-Lin (KL) partitioning algorithm
///
/// A classic graph partitioning algorithm for bipartitioning.
pub struct KernighanLin {
    /// Maximum number of iterations for the algorithm
    pub max_iterations: usize,
}

impl KernighanLin {
    /// Create a new KL partitioner with default settings
    pub fn new() -> Self {
        Self {
            max_iterations: 100,
        }
    }

    /// Set the maximum number of iterations
    pub fn with_max_iterations(mut self, iterations: usize) -> Self {
        self.max_iterations = iterations;
        self
    }

    /// Partition the netlist into two balanced subsets
    ///
    /// # Arguments
    ///
    /// * `netlist` - The netlist to partition
    /// * `balance_factor` - Target balance ratio (0.0 to 1.0), 0.5 means equal halves
    ///
    /// # Returns
    ///
    /// A `PartitionResult` containing the partition assignment, cut size, and balance
    pub fn partition(
        &self,
        netlist: &Netlist,
        balance_factor: f64,
    ) -> PartitionResult<PartitionResultData> {
        if netlist.num_modules() == 0 {
            return Err(PartitionError::EmptyNetlist);
        }

        if !(0.0..=1.0).contains(&balance_factor) {
            return Err(PartitionError::InvalidBalanceFactor(balance_factor));
        }

        // Initial random partition
        let mut assignment: HashMap<String, Partition> = HashMap::new();
        let target_size_0 = (netlist.num_modules() as f64 * balance_factor).round() as usize;

        for (i, module) in netlist.modules.iter().enumerate() {
            let partition = if i < target_size_0 {
                Partition::Side0
            } else {
                Partition::Side1
            };
            assignment.insert(module.clone(), partition);
        }

        // Compute initial cut size
        let cut_size = self.compute_cut_size(netlist, &assignment);

        Ok(PartitionResultData {
            assignment,
            cut_size,
            balance: balance_factor,
        })
    }

    /// Compute the number of cut nets given a partition assignment
    fn compute_cut_size(
        &self,
        netlist: &Netlist,
        assignment: &HashMap<String, Partition>,
    ) -> usize {
        let mut cut_size = 0;

        for net in &netlist.nets {
            let mut side0_modules = 0;
            let mut side1_modules = 0;

            // Find all modules connected to this net
            for node in netlist.grph.node_indices() {
                let node_name = &netlist.grph[node];
                if node_name == net {
                    // This is a net node, find connected modules
                    for neighbor in netlist.grph.neighbors(node) {
                        let neighbor_name = &netlist.grph[neighbor];
                        if netlist.modules.contains(neighbor_name) {
                            if let Some(&partition) = assignment.get(neighbor_name) {
                                match partition {
                                    Partition::Side0 => side0_modules += 1,
                                    Partition::Side1 => side1_modules += 1,
                                }
                            }
                        }
                    }
                }
            }

            // Net is cut if it has modules on both sides
            if side0_modules > 0 && side1_modules > 0 {
                cut_size += 1;
            }
        }

        cut_size
    }
}

impl Default for KernighanLin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::Netlist;

    #[test]
    fn test_partition_empty_netlist() {
        let netlist = Netlist::new();
        let fm = FiducciaMattheyses::new();
        let result = fm.partition(&netlist, 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_partition_invalid_balance() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let fm = FiducciaMattheyses::new();
        let result = fm.partition(&netlist, 1.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_partition_simple() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_module("m4".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();
        netlist.add_edge("n2", "m4").unwrap();

        let fm = FiducciaMattheyses::new();
        let result = fm.partition(&netlist, 0.5);
        assert!(result.is_ok());
        let partition_data = result.unwrap();
        assert_eq!(partition_data.assignment.len(), 4);
    }

    #[test]
    fn test_fm_with_max_iterations() {
        let fm = FiducciaMattheyses::new().with_max_iterations(50);
        assert_eq!(fm.max_iterations, 50);
    }

    #[test]
    fn test_kernighan_lin_new() {
        let kl = KernighanLin::new();
        assert_eq!(kl.max_iterations, 100);
    }

    #[test]
    fn test_kernighan_lin_partition() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let _ = netlist.add_module("m2".to_string());
        let _ = netlist.add_module("m3".to_string());
        let _ = netlist.add_module("m4".to_string());
        let _ = netlist.add_net("n1".to_string());
        let _ = netlist.add_net("n2".to_string());
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();
        netlist.add_edge("n2", "m4").unwrap();

        let kl = KernighanLin::new();
        let result = kl.partition(&netlist, 0.5);
        assert!(result.is_ok());
        let partition_data = result.unwrap();
        assert_eq!(partition_data.assignment.len(), 4);
    }

    #[test]
    fn test_kernighan_lin_with_max_iterations() {
        let kl = KernighanLin::new().with_max_iterations(50);
        assert_eq!(kl.max_iterations, 50);
    }

    #[test]
    fn test_kl_partition_empty_netlist() {
        let netlist = Netlist::new();
        let kl = KernighanLin::new();
        let result = kl.partition(&netlist, 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_kl_invalid_balance() {
        let mut netlist = Netlist::new();
        let _ = netlist.add_module("m1".to_string());
        let kl = KernighanLin::new();
        let result = kl.partition(&netlist, -0.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_fm() {
        let fm: FiducciaMattheyses = Default::default();
        assert_eq!(fm.max_iterations, 100);
    }

    #[test]
    fn test_default_kl() {
        let kl: KernighanLin = Default::default();
        assert_eq!(kl.max_iterations, 100);
    }
}
