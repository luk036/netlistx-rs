//! Partitioning example
//!
//! Run with: cargo run --example partitioning_example

use netlistx_rs::{FiducciaMattheyses, KernighanLin, NetlistBuilder, Partition};

fn main() {
    println!("=== Partitioning Example ===\n");

    let netlist = NetlistBuilder::new()
        .add_module("alu")
        .add_module("reg1")
        .add_module("reg2")
        .add_module("reg3")
        .add_module("mux1")
        .add_module("mux2")
        .add_module("decoder")
        .add_module("adder")
        .add_net("ctrl")
        .add_net("data1")
        .add_net("data2")
        .add_edge("ctrl", "alu")
        .add_edge("ctrl", "mux1")
        .add_edge("ctrl", "decoder")
        .add_edge("data1", "reg1")
        .add_edge("data1", "mux1")
        .add_edge("data1", "adder")
        .add_edge("data2", "reg2")
        .add_edge("data2", "mux2")
        .add_edge("data2", "adder")
        .add_edge("data1", "reg3")
        .add_edge("data2", "reg3")
        .add_edge("ctrl", "mux2")
        .build()
        .unwrap();

    println!(
        "Netlist: {} modules, {} nets\n",
        netlist.num_modules(),
        netlist.num_nets()
    );

    // FM Partitioning
    let fm = FiducciaMattheyses::new();
    let fm_result = fm.partition(&netlist, 0.5).unwrap();
    println!("FM Partitioning (balance=0.5):");
    println!("  Cut size: {}", fm_result.cut_size);

    let side0: Vec<_> = fm_result
        .assignment
        .iter()
        .filter(|(_, &p)| p == Partition::Side0)
        .map(|(k, _)| k.as_str())
        .collect();
    let side1: Vec<_> = fm_result
        .assignment
        .iter()
        .filter(|(_, &p)| p == Partition::Side1)
        .map(|(k, _)| k.as_str())
        .collect();
    println!("  Side 0: {:?}", side0);
    println!("  Side 1: {:?}", side1);

    // KL Partitioning
    let kl = KernighanLin::new();
    let kl_result = kl.partition(&netlist, 0.5).unwrap();
    println!("\nKL Partitioning (balance=0.5):");
    println!("  Cut size: {}", kl_result.cut_size);
}
