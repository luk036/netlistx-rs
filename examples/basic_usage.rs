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

    let n1 = netlist.add_net("net1".to_string()).unwrap();
    let n2 = netlist.add_net("net2".to_string()).unwrap();
    let clk = netlist.add_net("clk".to_string()).unwrap();

    netlist.add_edge(n1, 0).unwrap(); // net1-AND
    netlist.add_edge(n1, 1).unwrap(); // net1-OR
    netlist.add_edge(n2, 1).unwrap(); // net2-OR
    netlist.add_edge(n2, 2).unwrap(); // net2-NOT
    netlist.add_edge(clk, 3).unwrap(); // clk-FF

    println!("Created netlist:");
    println!("  Modules: {}", netlist.num_modules());
    println!("  Nets: {}", netlist.num_nets());
    println!("  Edges: {}", netlist.gr.edge_count());

    let stats = NetlistStats::analyze(&netlist);
    println!("\nStatistics:");
    println!("  Avg module degree: {:.2}", stats.avg_module_degree());
    println!("  Max module degree: {}", stats.max_module_degree());
    println!("  Avg net degree: {:.2}", stats.avg_net_degree());
    println!("  Pin count: {}", stats.num_pins);

    println!("\nModule degrees:");
    for i in netlist.module_indices() {
        println!(
            "  {}: {}",
            netlist.module_names[i],
            netlist.get_module_degree(i)
        );
    }
}
