//! Benchmarks for netlistx-rs

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use netlistx_rs::{
    partitioning::{FiducciaMattheyses, KernighanLin},
    statistics::NetlistStats,
    Netlist, NetlistBuilder,
};

fn create_netlist(num_modules: usize, num_nets: usize, connections_per_net: usize) -> Netlist {
    let mut builder = NetlistBuilder::new();

    // Add modules
    for i in 0..num_modules {
        builder = builder.add_module(&format!("m{}", i));
    }

    // Add nets
    for i in 0..num_nets {
        builder = builder.add_net(&format!("n{}", i));
    }

    // Add connections
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

fn bench_partitioning_fm(c: &mut Criterion) {
    let mut group = c.benchmark_group("partitioning_fm");

    for size in [10, 50, 100].iter() {
        let netlist = create_netlist(*size, *size / 2, 3);
        let fm = FiducciaMattheyses::new();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| fm.partition(black_box(&netlist), 0.5));
        });
    }

    group.finish();
}

fn bench_partitioning_kl(c: &mut Criterion) {
    let mut group = c.benchmark_group("partitioning_kl");

    for size in [10, 50, 100].iter() {
        let netlist = create_netlist(*size, *size / 2, 3);
        let kl = KernighanLin::new();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| kl.partition(black_box(&netlist), 0.5));
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
                for module in &netlist.modules {
                    black_box(netlist.get_module_degree(module));
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
                    let _ = nl.add_edge(&format!("n{}", i), &format!("m{}", i % size));
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
    bench_partitioning_fm,
    bench_partitioning_kl,
    bench_degree_calculation,
    bench_edge_addition
);
criterion_main!(benches);
