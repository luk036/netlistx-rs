//! Factory functions for creating test `Netlist` instances.
//!
//! Ported from Python `netlist.py` factory functions:
//! - `create_inverter`, `create_inverter2`, `create_drawf`, `create_test_netlist`
//! - `create_random_hgraph` with van der Corput sequences

use crate::netlist::{Netlist, NetlistBuilder};

/// Van der Corput sequence (base 2 by default).
///
/// Ported from Python `vdc()` in `netlist.py`.
pub fn vdc(n: u32, base: u32) -> f64 {
    let mut n = n;
    let mut vdc_val = 0.0f64;
    let mut denom = 1.0f64;
    while n > 0 {
        denom *= base as f64;
        let remainder = (n % base) as f64;
        n /= base;
        vdc_val += remainder / denom;
    }
    vdc_val
}

/// Generate van der Corput sequence of length `n`.
///
/// Ported from Python `vdcorput()` in `netlist.py`.
pub fn vdcorput(n: u32, base: u32) -> Vec<f64> {
    (0..n).map(|i| vdc(i, base)).collect()
}

/// Simple LCG random number generator (no external crate dependency).
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 1 } else { seed } }
    }

    fn next_f64(&mut self) -> f64 {
        // LCG parameters from Numerical Recipes
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        // Extract top 53 bits for double precision
        (self.state >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Create a simple inverter netlist.
///
/// Ported from Python `create_inverter()` in `netlist.py`.
pub fn create_inverter() -> Netlist {
    let mut netlist = NetlistBuilder::new()
        .add_module("a0")
        .add_module("p1")
        .add_module("p2")
        .add_net("n0")
        .add_net("n1")
        .add_edge("n0", "p1")
        .add_edge("n0", "a0")
        .add_edge("n1", "a0")
        .add_edge("n1", "p2")
        .with_pads(2)
        .build()
        .expect("Failed to build inverter netlist");

    netlist.set_module_weight("a0", 1);
    netlist.set_module_weight("p1", 0);
    netlist.set_module_weight("p2", 0);

    netlist
}

/// Create an inverter netlist with numeric-style node IDs.
///
/// Ported from Python `create_inverter2()` in `netlist.py`.
pub fn create_inverter2() -> Netlist {
    let mut netlist = NetlistBuilder::new()
        .add_module("mod0")
        .add_module("mod1")
        .add_module("mod2")
        .add_net("net0")
        .add_net("net1")
        .add_edge("net0", "mod1")
        .add_edge("net0", "mod0")
        .add_edge("net1", "mod0")
        .add_edge("net1", "mod2")
        .with_pads(2)
        .build()
        .expect("Failed to build inverter2 netlist");

    netlist.set_module_weight("mod0", 1);
    netlist.set_module_weight("mod1", 0);
    netlist.set_module_weight("mod2", 0);

    netlist
}

/// Create a test netlist with 7 modules and 6 nets.
///
/// Ported from Python `create_drawf()` in `netlist.py`.
pub fn create_drawf() -> Netlist {
    let mut builder = NetlistBuilder::new();
    // Modules
    for m in &["a0", "a1", "a2", "a3", "p1", "p2", "p3"] {
        builder = builder.add_module(m);
    }
    // Nets
    for n in &["n0", "n1", "n2", "n3", "n4", "n5"] {
        builder = builder.add_net(n);
    }
    // Edges
    builder = builder
        .add_edge("n0", "p1").add_edge("n0", "a0").add_edge("n0", "a1")
        .add_edge("n1", "a0").add_edge("n1", "a2").add_edge("n1", "a3")
        .add_edge("n2", "a1").add_edge("n2", "a2").add_edge("n2", "a3")
        .add_edge("n3", "a2").add_edge("n3", "p2")
        .add_edge("n4", "a3").add_edge("n4", "p3")
        .add_edge("n5", "p2");

    let mut netlist = builder.with_pads(3).build().expect("Failed to build drawf netlist");

    netlist.set_module_weight("a0", 1);
    netlist.set_module_weight("a1", 3);
    netlist.set_module_weight("a2", 4);
    netlist.set_module_weight("a3", 2);
    netlist.set_module_weight("p1", 0);
    netlist.set_module_weight("p2", 0);
    netlist.set_module_weight("p3", 0);

    netlist
}

/// Create a test netlist with 3 modules and 3 nets.
///
/// Ported from Python `create_test_netlist()` in `netlist.py`.
pub fn create_test_netlist() -> Netlist {
    let mut builder = NetlistBuilder::new();
    for m in &["a0", "a1", "a2"] {
        builder = builder.add_module(m);
    }
    for n in &["a3", "a4", "a5"] {
        builder = builder.add_net(n);
    }
    builder = builder
        .add_edge("a3", "a0")
        .add_edge("a3", "a1")
        .add_edge("a4", "a0")
        .add_edge("a4", "a1")
        .add_edge("a4", "a2")
        .add_edge("a5", "a0");

    let mut netlist = builder.build().expect("Failed to build test netlist");

    netlist.set_module_weight("a0", 533);
    netlist.set_module_weight("a1", 543);
    netlist.set_module_weight("a2", 532);

    netlist
}

/// Create a random bipartite hypergraph for testing.
///
/// Uses van der Corput quasi-random sequences to distribute nodes
/// uniformly in 2D, then creates edges based on distance threshold.
#[allow(non_snake_case)]
pub fn create_random_hgraph(N: u32, M: u32, eta: f64, seed: u64) -> Netlist {
    let t = N + M;
    let xbase = 2;
    let ybase = 3;
    let x = vdcorput(t, xbase);
    let y = vdcorput(t, ybase);

    let mut rng = SimpleRng::new(seed);

    let mut builder = NetlistBuilder::new();
    for i in 0..N {
        builder = builder.add_module(&format!("m{}", i));
    }
    for j in 0..M {
        builder = builder.add_net(&format!("n{}", j));
    }

    // Connect nodes within distance threshold eta
    for i in 0..N {
        for j in 0..M {
            let dx = x[i as usize] - x[(N + j) as usize];
            let dy = y[i as usize] - y[(N + j) as usize];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < eta && rng.next_f64() > 0.5 {
                builder = builder.add_edge(&format!("n{}", j), &format!("m{}", i));
            }
        }
    }

    builder.build().expect("Failed to build random hypergraph")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vdc() {
        assert!((vdc(0, 2) - 0.0).abs() < 1e-10);
        assert!((vdc(1, 2) - 0.5).abs() < 1e-10);
        assert!((vdc(2, 2) - 0.25).abs() < 1e-10);
        assert!((vdc(3, 2) - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_vdcorput() {
        let seq = vdcorput(4, 2);
        assert_eq!(seq.len(), 4);
        assert!((seq[0] - 0.0).abs() < 1e-10);
        assert!((seq[1] - 0.5).abs() < 1e-10);
        assert!((seq[2] - 0.25).abs() < 1e-10);
        assert!((seq[3] - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_create_inverter() {
        let netlist = create_inverter();
        assert_eq!(netlist.num_modules(), 3);
        assert_eq!(netlist.num_nets(), 2);
        assert_eq!(netlist.num_pads, 2);
        assert_eq!(netlist.get_module_weight("a0"), 1);
        assert_eq!(netlist.get_module_weight("p1"), 0);
    }

    #[test]
    fn test_create_inverter2() {
        let netlist = create_inverter2();
        assert_eq!(netlist.num_modules(), 3);
        assert_eq!(netlist.num_nets(), 2);
        assert_eq!(netlist.num_pads, 2);
    }

    #[test]
    fn test_create_drawf() {
        let netlist = create_drawf();
        assert_eq!(netlist.num_modules(), 7);
        assert_eq!(netlist.num_nets(), 6);
        assert_eq!(netlist.num_pads, 3);
        assert_eq!(netlist.get_module_weight("a0"), 1);
        assert_eq!(netlist.get_module_weight("a1"), 3);
        assert_eq!(netlist.get_module_weight("a2"), 4);
        assert_eq!(netlist.get_module_weight("a3"), 2);
        assert_eq!(netlist.get_module_weight("p1"), 0);
    }

    #[test]
    fn test_create_test_netlist() {
        let netlist = create_test_netlist();
        assert_eq!(netlist.num_modules(), 3);
        assert_eq!(netlist.num_nets(), 3);
        assert_eq!(netlist.get_module_weight("a0"), 533);
        assert_eq!(netlist.get_module_weight("a1"), 543);
        assert_eq!(netlist.get_module_weight("a2"), 532);
    }

    #[test]
    fn test_create_random_hgraph() {
        let netlist = create_random_hgraph(10, 8, 0.3, 42);
        assert_eq!(netlist.num_modules(), 10);
        assert_eq!(netlist.num_nets(), 8);
    }

    #[test]
    fn test_create_random_hgraph_reproducible() {
        let n1 = create_random_hgraph(20, 15, 0.2, 123);
        let n2 = create_random_hgraph(20, 15, 0.2, 123);
        assert_eq!(n1.num_modules(), n2.num_modules());
        assert_eq!(n1.num_nets(), n2.num_nets());
    }
}
