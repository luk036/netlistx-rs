//! Benchmarks for netlistx-rs

use std::collections::HashMap;
use std::collections::HashSet;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use netlistx_rs::graph_algo::min_vertex_cover_fast;
use netlistx_rs::{statistics::NetlistStats, Netlist, NetlistBuilder};

/// Integer-keyed version — same `min_vertex_cover_fast`, same graph, different key type.
fn bench_vc_fast_line_int(c: &mut Criterion) {
    let grph = {
        let mut g = petgraph::Graph::<u32, (), petgraph::Undirected>::new_undirected();
        let n = (0..5u32).map(|i| g.add_node(i)).collect::<Vec<_>>();
        g.add_edge(n[0], n[1], ());
        g.add_edge(n[1], n[2], ());
        g.add_edge(n[2], n[3], ());
        g.add_edge(n[3], n[4], ());
        g
    };
    let weight: HashMap<u32, i32> = (0..5).map(|i| (i, 1)).collect();

    c.bench_function("min_vertex_cover_fast_line_int", |b| {
        b.iter(|| {
            let mut coverset = HashSet::new();
            let (sol, cost) =
                min_vertex_cover_fast(black_box(&grph), black_box(&weight), &mut coverset);
            black_box((sol, cost));
        });
    });
}

fn bench_vc_fast_line(c: &mut Criterion) {
    let grph = {
        let mut g = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
        let n0 = g.add_node("n0".into());
        let n1 = g.add_node("n1".into());
        let n2 = g.add_node("n2".into());
        let n3 = g.add_node("n3".into());
        let n4 = g.add_node("n4".into());
        g.add_edge(n0, n1, ());
        g.add_edge(n1, n2, ());
        g.add_edge(n2, n3, ());
        g.add_edge(n3, n4, ());
        g
    };
    let mut weight = HashMap::new();
    for i in 0..5 {
        weight.insert(format!("n{}", i), 1i32);
    }

    c.bench_function("min_vertex_cover_fast_line", |b| {
        b.iter(|| {
            let mut coverset = std::collections::HashSet::new();
            let (sol, cost) =
                min_vertex_cover_fast(black_box(&grph), black_box(&weight), &mut coverset);
            black_box((sol, cost));
        });
    });
}

fn create_netlist(num_modules: usize, num_nets: usize, connections_per_net: usize) -> Netlist {
    let mut builder = NetlistBuilder::new();

    for i in 0..num_modules {
        builder = builder.add_module(&format!("m{}", i));
    }

    for i in 0..num_nets {
        builder = builder.add_net(&format!("n{}", i));
    }

    for i in 0..num_nets {
        for j in 0..connections_per_net {
            let module_idx = (i * connections_per_net + j) % num_modules;
            builder = builder.add_edge(&format!("n{}", i), &format!("m{}", module_idx));
        }
    }

    builder.build().unwrap()
}

fn bench_netlist_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("netlist_creation");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut builder = NetlistBuilder::new();
                for i in 0..size {
                    builder = builder.add_module(&format!("m{}", i));
                }
                for i in 0..(size / 2) {
                    builder = builder.add_net(&format!("n{}", i));
                }
                builder.build().unwrap()
            });
        });
    }

    group.finish();
}

fn bench_statistics(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistics");

    for size in [10, 50, 100, 500].iter() {
        let netlist = create_netlist(*size, *size / 2, 3);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| NetlistStats::analyze(black_box(&netlist)));
        });
    }

    group.finish();
}

fn bench_degree_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("degree_calculation");

    for size in [10, 50, 100, 500].iter() {
        let netlist = create_netlist(*size, *size / 2, 3);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                for m in netlist.module_indices() {
                    black_box(netlist.get_module_degree(m));
                }
            });
        });
    }

    group.finish();
}

fn bench_edge_addition(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_addition");

    for size in [10, 50, 100, 500].iter() {
        let netlist = create_netlist(*size, *size / 2, 0);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut nl = netlist.clone();
                for i in 0..(size / 2) {
                    let net_idx = i;
                    let mod_idx = i % size;
                    let _ = nl.add_edge(net_idx, mod_idx);
                }
                black_box(nl)
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_netlist_creation,
    bench_statistics,
    bench_degree_calculation,
    bench_edge_addition,
    bench_vc_fast_line,
    bench_vc_fast_line_int,
);
criterion_main!(benches);
