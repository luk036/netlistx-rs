//! Integration tests for netlistx-rs

use netlistx_rs::{io::write_netlist, statistics::NetlistStats, Netlist, NetlistBuilder};
use tempfile::NamedTempFile;

#[test]
fn test_io_roundtrip() {
    // Create a netlist
    let mut builder = NetlistBuilder::new();
    for i in 0..5 {
        builder = builder.add_module(&format!("m{}", i));
    }
    for i in 0..3 {
        builder = builder.add_net(&format!("n{}", i));
    }
    builder = builder
        .add_edge("n0", "m0")
        .add_edge("n0", "m1")
        .add_edge("n1", "m1")
        .add_edge("n1", "m2")
        .add_edge("n2", "m2")
        .add_edge("n2", "m3");

    let original_netlist = builder.build().unwrap();

    let temp_file = NamedTempFile::new().unwrap();
    write_netlist(&original_netlist, temp_file.path()).unwrap();
    let content = std::fs::read_to_string(temp_file.path()).unwrap();
    assert!(content.contains("NET"));
    assert!(content.contains("MODULE"));
}

#[test]
fn test_netlist_statistics() {
    // Create a netlist
    let num_modules = 100;
    let num_nets = 50;

    let mut builder = NetlistBuilder::new();
    for i in 0..num_modules {
        builder = builder.add_module(&format!("m{}", i));
    }
    for i in 0..num_nets {
        builder = builder.add_net(&format!("n{}", i));
    }

    // Add edges in a pattern
    for i in 0..num_nets {
        let m1 = i % num_modules;
        let m2 = (i + 1) % num_modules;
        let m3 = (i + 2) % num_modules;
        builder = builder
            .add_edge(&format!("n{}", i), &format!("m{}", m1))
            .add_edge(&format!("n{}", i), &format!("m{}", m2))
            .add_edge(&format!("n{}", i), &format!("m{}", m3));
    }

    let netlist = builder.build().unwrap();

    // Compute statistics (should be fast)
    let start = std::time::Instant::now();
    let stats = NetlistStats::analyze(&netlist);
    let duration = start.elapsed();

    assert_eq!(stats.num_modules, num_modules);
    assert_eq!(stats.num_nets, num_nets);
    assert!(
        duration.as_millis() < 1000,
        "Statistics computation took too long"
    );
}

#[test]
fn test_empty_netlist_handling() {
    let netlist = Netlist::new();

    // Statistics on empty netlist
    let stats = NetlistStats::analyze(&netlist);
    assert_eq!(stats.num_modules, 0);
    assert_eq!(stats.num_nets, 0);
    assert_eq!(stats.num_pins, 0);
}

#[test]
fn test_single_module_netlist() {
    let mut netlist = Netlist::new();
    netlist.add_module("m1".to_string()).unwrap();

    assert_eq!(netlist.num_modules(), 1);
    assert_eq!(netlist.num_nets(), 0);
    assert_eq!(netlist.get_module_degree(0), 0);
}

#[test]
fn test_single_net_netlist() {
    let mut netlist = Netlist::new();
    netlist.add_net("n1".to_string()).unwrap();

    assert_eq!(netlist.num_modules(), 0);
    assert_eq!(netlist.num_nets(), 1);
    assert_eq!(netlist.get_net_degree(0), 0);
}
