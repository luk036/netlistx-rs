//! Basic netlist creation example
//!
//! Run with: cargo run --example basic_usage

use netlistx_rs::{Netlist, NetlistStats};

fn main() {
    println!("=== Basic Netlist Usage ===\n");

    let mut netlist = Netlist::new();

    netlist.add_module("AND".to_string()).unwrap();
    netlist.add_module("OR".to_string()).unwrap();
    netlist.add_module("NOT".to_string()).unwrap();
    netlist.add_module("FF".to_string()).unwrap();

    netlist.add_net("net1".to_string()).unwrap();
    netlist.add_net("net2".to_string()).unwrap();
    netlist.add_net("clk".to_string()).unwrap();

    netlist.add_edge("net1", "AND").unwrap();
    netlist.add_edge("net1", "OR").unwrap();
    netlist.add_edge("net2", "OR").unwrap();
    netlist.add_edge("net2", "NOT").unwrap();
    netlist.add_edge("clk", "FF").unwrap();

    println!("Created netlist:");
    println!("  Modules: {}", netlist.num_modules());
    println!("  Nets: {}", netlist.num_nets());
    println!("  Edges: {}", netlist.grph.edge_count());

    let stats = NetlistStats::analyze(&netlist);
    println!("\nStatistics:");
    println!("  Avg module degree: {:.2}", stats.avg_module_degree());
    println!("  Max module degree: {}", stats.max_module_degree());
    println!("  Avg net degree: {:.2}", stats.avg_net_degree());
    println!("  Pin count: {}", stats.num_pins);

    println!("\nModule degrees:");
    for module in &netlist.modules {
        println!("  {}: {}", module, netlist.get_module_degree(module));
    }
}
