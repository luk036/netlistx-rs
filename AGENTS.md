# AGENTS.md - Agentic Coding Guidelines for netlistx-rs

This file provides guidelines for AI coding agents operating in this repository.

---

## Project Overview

- **Project**: netlistx-rs - Netlist (hypergraph) library for EDA
- **Language**: Rust (edition 2021, minimum rust-version 1.70)
- **Repository**: https://github.com/luk036/netlistx-rs
- **License**: MIT OR Apache-2.0

---

## Build, Lint, and Test Commands

### Running Tests

```bash
# Run all tests (recommended for CI)
cargo test --all-features --workspace

# Run all tests with output
cargo test --all-features --workspace -- --nocapture

# Run a single test by name
cargo test test_name -- --nocapture

# Run doc tests only
cargo test --doc

# Run specific module tests (e.g., netlist module)
cargo test netlist:: -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Check formatting (no changes)
cargo fmt --all -- --check

# Run Clippy lints (all targets, all features)
cargo clippy --all-targets --all-features --workspace
```

### Building and Documentation

```bash
# Build in release mode
cargo build --release

# Build documentation (strict - treats warnings as errors)
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace

# Run with example binary
cargo run --release
```

---

## Code Style Guidelines

### General Principles

1. **Follow existing patterns**: Examine `src/netlist.rs` for canonical examples
2. **Use derive macros**: Prefer `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]` where applicable
3. **Document public APIs**: Use doc comments (`///`) for all public functions and types
4. **Write tests**: Add unit tests in `#[cfg(test)]` modules within each source file

### Imports and Dependencies

**External crates to use:**
- `petgraph` (0.8.2) - Graph data structures
- `indexmap` (2.2) - Ordered maps/sets with serde support
- `thiserror` (1.0) - Error type derivation

**Optional features (Cargo.toml):**
- `serde` - Serialization via serde
- `rayon` - Parallel processing
- `quickcheck` - Property-based testing

**Import style:**
```rust
// Group in order: std → external → crate
use std::collections::HashSet;

use indexmap::{IndexMap, IndexSet};
use petgraph::graph::NodeIndex;

use crate::netlist::{Netlist, NetlistError};
```

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Types | PascalCase | `Netlist`, `PartitionError` |
| Functions | snake_case | `add_module`, `get_module_degree` |
| Variables | snake_case | `netlist`, `module_indices` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_ITERATIONS` |
| Enum variants | PascalCase | `Partition::Side0` |
| Modules | snake_case | `mod netlist`, `mod partitioning` |
| Files | snake_case | `netlist.rs`, `partitioning.rs` |

### Error Handling

**Use thiserror for error types:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum NetlistError {
    #[error("Module not found: {0}")]
    ModuleNotFound(String),
    // ... more variants
}

pub type NetlistResult<T> = Result<T, NetlistError>;
```

**Always propagate errors:**
```rust
pub fn add_module(&mut self, module: String) -> NetlistResult<()> {
    if module.is_empty() {
        return Err(NetlistError::InvalidModuleName(module));
    }
    // ... rest of implementation
}
```

### Documentation

**Structure doc comments as:**
```rust
/// Description of what this type/function does.
///
/// # Arguments
///
/// * `arg1` - Description of first argument
/// * `arg2` - Description of second argument
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// Description of possible error conditions
///
/// # Examples
///
/// ```
/// use netlistx_rs::Netlist;
///
/// let netlist = Netlist::new();
/// assert_eq!(netlist.num_modules(), 0);
/// ```
```

### Struct Design

**Prefer public fields when reasonable:**
```rust
#[derive(Debug, Clone)]
pub struct Netlist {
    pub num_pads: i32,
    pub cost_model: i32,
    pub grph: petgraph::Graph<String, (), petgraph::Undirected>,
    pub modules: IndexSet<String>,
    // ...
}
```

**Use builder pattern for complex construction:**
```rust
pub struct NetlistBuilder {
    netlist: Netlist,
}

impl NetlistBuilder {
    pub fn new() -> Self { ... }
    pub fn add_module(mut self, module: &str) -> Self { ... }
    pub fn add_net(mut self, net: &str) -> Self { ... }
    pub fn add_edge(mut self, net: &str, module: &str) -> Self { ... }
    pub fn build(self) -> NetlistResult<Netlist> { ... }
}
```

### Testing Guidelines

**Test module structure:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_netlist() -> Netlist {
        // Helper to create test data
    }

    #[test]
    fn test_something() {
        let netlist = create_test_netlist();
        // assertions
    }
}
```

**For property-based testing:**
```rust
#[cfg(test)]
#[cfg(feature = "quickcheck")]
mod quickcheck_impls {
    use quickcheck::{Arbitrary, Gen};

    impl Arbitrary for Netlist {
        fn arbitrary(g: &mut Gen) -> Self {
            // ... implementation
        }
    }
}
```

---

## Project Structure

```
netlistx-rs/
├── src/
│   ├── lib.rs         # Main library entry, re-exports
│   ├── netlist.rs    # Core Netlist struct and builder
│   ├── partitioning.rs  # FM and KL partitioning
│   ├── statistics.rs # NetlistStats analysis
│   ├── io.rs         # File I/O
│   └── trigonom.rs   # Trigonom utilities
├── tests/
│   └── integration_tests.rs
├── Cargo.toml
├── CHANGELOG.md
└── CONTRIBUTING.md
```

---

## Feature Flags

- `serde` - Enable serialization/deserialization (implies `indexmap/serde`, `petgraph/serde`)
- `rayon` - Enable parallel processing with rayon
- `quickcheck` - Enable property-based testing

---

## CI/CD Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs:
1. `cargo test --all-features --workspace` - Tests
2. `cargo fmt --all --check` - Formatting
3. `cargo clippy --all-targets --all-features --workspace` - Lints
4. `cargo doc --no-deps --document-private-items --all-features --workspace` - Documentation

All checks must pass for PRs to be merged.

---

## Contributing Workflow

1. Clone the repository
2. Create a feature branch
3. Make changes following these guidelines
4. Run tests and Clippy locally
5. Update CHANGELOG.md under "Unreleased"
6. Submit a pull request

---

## Common Patterns

### Module Discovery
```rust
// In lib.rs
pub mod io;
pub mod netlist;
pub mod partitioning;
pub mod statistics;
pub mod trigonom;

// Re-exports for convenience
pub use netlist::{Netlist, NetlistBuilder, NetlistError};
```

### Iterating Over Collections
```rust
for module in &netlist.modules {
    let degree = netlist.get_module_degree(module);
    // ...
}
```

### Degree Computation
```rust
pub fn get_module_degree(&self, module: &str) -> usize {
    if let Some(&node_index) = self.module_indices.get(module) {
        self.grph.neighbors(node_index).count()
    } else {
        0
    }
}
```

---

## Quick Reference: Running a Single Test

```bash
# Test in netlist module named "test_add_module"
cargo test test_add_module -- --nocapture

# Test in partitioning module
cargo test partitioning:: -- --nocapture

# Test with specific pattern
cargo test add_edge -- --nocapture
```