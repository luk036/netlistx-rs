//! netlistx-rs: Netlist (hypergraph) library for electronic design automation
//!
//! This library provides efficient data structures and algorithms for working with
//! netlists in EDA applications.

pub mod io;
pub mod netlist;
pub mod partitioning;
pub mod statistics;
pub mod trigonom;

// Re-exports for convenience
pub use netlist::{Netlist, NetlistBuilder, NetlistError};
pub use partitioning::{
    FiducciaMattheyses, KernighanLin, Partition, PartitionError, PartitionResult,
};
pub use statistics::NetlistStats;
