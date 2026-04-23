use criterion::{black_box, criterion_group, criterion_main, Criterion};
use netlistx_rs::netlist::{Netlist, NetlistBuilder};
use netlistx_rs::partitioning::FiducciaMattheyses;
use netlistx_rs::statistics::NetlistStats;

fn create_large_netlist(num_modules: usize, nets_per_module: usize) -> Netlist {
    let mut builder = NetlistBuilder::new();

    for i in 0..num_modules {
        builder = builder.add_module(&format!("m{}", i));
    }

    for i in 0..(num_modules / 2) {
        builder = builder.add_net(&format!("n{}", i));
    }

    for i in 0..num_modules {
        for j in 0..nets_per_module {
            let net_idx = j % (num_modules / 2);
            builder = builder.add_edge(&format!("n{}", net_idx), &format!("m{}", i));
        }
    }

    builder.build().unwrap()
}

fn bench_netlist_creation(c: &mut Criterion) {
    c.bench_function("netlist_creation_small", |b| {
        b.iter(|| {
            let netlist = create_large_netlist(black_box(50), black_box(3));
            black_box(netlist.num_modules());
        });
    });

    c.bench_function("netlist_creation_large", |b| {
        b.iter(|| {
            let netlist = create_large_netlist(black_box(200), black_box(5));
            black_box(netlist.num_modules());
        });
    });
}

fn bench_statistics(c: &mut Criterion) {
    let netlist = create_large_netlist(100, 4);

    c.bench_function("statistics_analyze", |b| {
        b.iter(|| {
            let stats = NetlistStats::analyze(black_box(&netlist));
            black_box(stats.num_pins);
        });
    });

    c.bench_function("module_degree_computation", |b| {
        let module = "m50";
        b.iter(|| {
            let degree = netlist.get_module_degree(black_box(module));
            black_box(degree);
        });
    });
}

fn bench_partitioning(c: &mut Criterion) {
    let netlist = create_large_netlist(100, 4);
    let fm = FiducciaMattheyses::new();

    c.bench_function("partitioning_fm_100_modules", |b| {
        b.iter(|| {
            let result = fm.partition(black_box(&netlist), black_box(0.5));
            black_box(result.unwrap().cut_size);
        });
    });

    let large_netlist = create_large_netlist(500, 4);

    c.bench_function("partitioning_fm_500_modules", |b| {
        b.iter(|| {
            let result = fm.partition(black_box(&large_netlist), black_box(0.5));
            black_box(result.unwrap().cut_size);
        });
    });
}

criterion_group!(
    benches,
    bench_netlist_creation,
    bench_statistics,
    bench_partitioning
);
criterion_main!(benches);
