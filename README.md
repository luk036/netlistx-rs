# 🥅 netlistx-rs

[![Crates.io](https://img.shields.io/crates/v/netlistx-rs.svg)](https://crates.io/crates/netlistx-rs)
[![Docs.rs](https://docs.rs/netlistx-rs/badge.svg)](https://docs.rs/netlistx-rs)
[![CI](https://github.com/luk036/netlistx-rs/workflows/CI/badge.svg)](https://github.com/luk036/netlistx-rs/actions)
[![codecov](https://codecov.io/gh/luk036/netlistx-rs/graph/badge.svg?token=H7oT1T5LV5)](https://codecov.io/gh/luk036/netlistx-rs)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-APACHE)

**netlistx-rs** is a Rust library for working with netlists (hypergraphs) in electronic design automation (EDA). It provides efficient data structures and algorithms for representing, analyzing, and partitioning circuit netlists.

## 📖 What is a Netlist?

A **netlist** is a representation of an electronic circuit that describes the connections between components. In EDA, netlists are typically modeled as hypergraphs where:
- **Modules** (or cells) represent the circuit components
- **Nets** represent the wires that connect multiple modules together
- Each net can connect multiple modules, making it a hyperedge

## ✨ Features

- **Efficient Data Structures**: Fast graph-based representation using `petgraph`
- **Hypergraph Support**: True hypergraph semantics for multi-terminal nets
- **Partitioning Algorithms**: Kernighan-Lin and Fiduccia-Mattheyses partitioning
- **Connectivity Analysis**: Compute connectivity metrics and analyze circuit structure
- **Statistics**: Comprehensive netlist statistics and metrics
- **Serde Support**: Optional serialization/deserialization via `serde` feature
- **Parallel Processing**: Optional parallel operations via `rayon` feature

## 🚀 Installation

### Cargo

Add `netlistx-rs` to your `Cargo.toml`:

```toml
[dependencies]
netlistx-rs = "0.1"
```

For optional features:

```toml
[dependencies]
netlistx-rs = { version = "0.1", features = ["serde", "rayon"] }
```

## 💡 Usage Examples

### Basic Netlist Creation

```rust
use netlistx_rs::netlist::Netlist;

// Create a new netlist
let mut netlist = Netlist::new();

// Add modules
netlist.add_module("cell_a".to_string());
netlist.add_module("cell_b".to_string());
netlist.add_module("cell_c".to_string());

// Add nets
netlist.add_net("net1".to_string());
netlist.add_net("net2".to_string());

// Connect modules to nets
netlist.add_edge("net1", "cell_a").unwrap();
netlist.add_edge("net1", "cell_b").unwrap();
netlist.add_edge("net2", "cell_b").unwrap();
netlist.add_edge("net2", "cell_c").unwrap();

println!("Modules: {}", netlist.num_modules);
println!("Nets: {}", netlist.num_nets);
```

### Using the Builder Pattern

```rust
use netlistx_rs::netlist::NetlistBuilder;

let netlist = NetlistBuilder::new()
    .add_module("cell_a")
    .add_module("cell_b")
    .add_module("cell_c")
    .add_net("net1")
    .add_net("net2")
    .add_edge("net1", "cell_a")
    .add_edge("net1", "cell_b")
    .add_edge("net2", "cell_b")
    .add_edge("net2", "cell_c")
    .build()
    .unwrap();
```

### Netlist Partitioning

```rust
use netlistx_rs::partitioning::FiducciaMattheyses;

let partitioner = FiducciaMattheyses::new();
let result = partitioner.partition(&netlist, 0.5); // 50% balance
```

### Statistics and Metrics

```rust
use netlistx_rs::statistics::NetlistStats;

let stats = NetlistStats::analyze(&netlist);
println!("Average net degree: {}", stats.avg_net_degree());
println!("Maximum module degree: {}", stats.max_module_degree());
```

## 📚 API Documentation

Full API documentation is available at [docs.rs/netlistx-rs](https://docs.rs/netlistx-rs).

### Main Modules

- **`netlist`**: Core netlist data structure and operations
- **`partitioning`**: Circuit partitioning algorithms
- **`statistics`**: Netlist statistics and metrics
- **`io`**: File I/O for various netlist formats

## 🔧 Optional Features

- **`serde`**: Enable serialization/deserialization support
- **`rayon`**: Enable parallel processing for large netlists

## 🗺️ Roadmap

- [ ] Additional file format support (Verilog, ISPD, LEF/DEF)
- [ ] More partitioning algorithms (Multilevel partitioning)
- [ ] Placement algorithms
- [ ] Timing analysis utilities
- [ ] Power analysis tools
- [ ] Python bindings via PyO3

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## 📄 License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
