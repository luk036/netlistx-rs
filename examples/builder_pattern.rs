//! Builder pattern example
//!
//! Run with: cargo run --example builder_pattern

use netlistx_rs::{NetlistBuilder, NetlistStats};

fn main() {
    println!("=== Builder Pattern Example ===\n");

    let netlist = NetlistBuilder::new()
        .add_module("cell_a")
        .add_module("cell_b")
        .add_module("cell_c")
        .add_module("cell_d")
        .add_net("net1")
        .add_net("net2")
        .add_net("net3")
        .add_edge("net1", "cell_a")
        .add_edge("net1", "cell_b")
        .add_edge("net2", "cell_b")
        .add_edge("net2", "cell_c")
        .add_edge("net3", "cell_c")
        .add_edge("net3", "cell_d")
        .with_pads(4)
        .with_cost_model(1)
        .build()
        .unwrap();

    println!("Created netlist with builder:");
    println!("  Modules: {}", netlist.num_modules());
    println!("  Nets: {}", netlist.num_nets());
    println!("  Pads: {}", netlist.num_pads);

    let stats = NetlistStats::analyze(&netlist);
    println!("  Pin-to-module ratio: {:.2}", stats.pin_module_ratio());
}
