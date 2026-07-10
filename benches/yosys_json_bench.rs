use std::hint::black_box;
use std::io::Write;
use std::path::Path;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use netlistx_rs::io::{read_yosys_json, read_yosys_json_sax};

/// Generate a synthetic Yosys JSON file with the given number of cells and ports.
fn create_yosys_json_file(num_cells: usize, num_ports: usize) -> tempfile::NamedTempFile {
    let num_nets = (num_cells * 4 + num_ports) as u32;

    let mut cells = serde_json::Map::new();
    for i in 0..num_cells {
        let mut conn = serde_json::Map::new();
        let a_nets: Vec<u32> =
            (0..3).map(|j| ((i * 3 + j) as u32) % num_nets).collect();
        let y_net = ((i * 3 + 3) as u32) % num_nets;
        conn.insert(
            "A".into(),
            serde_json::Value::Array(
                a_nets.iter().map(|&n| serde_json::Value::Number(n.into())).collect(),
            ),
        );
        conn.insert(
            "Y".into(),
            serde_json::Value::Array(vec![serde_json::Value::Number(y_net.into())]),
        );

        let mut cell = serde_json::Map::new();
        cell.insert("type".into(), serde_json::Value::String("$and".into()));
        cell.insert("connections".into(), serde_json::Value::Object(conn));
        cells.insert(format!("cell_{}", i), serde_json::Value::Object(cell));
    }

    let mut ports = serde_json::Map::new();
    for i in 0..num_ports {
        let mut port = serde_json::Map::new();
        port.insert("direction".into(), serde_json::Value::String("input".into()));
        port.insert(
            "bits".into(),
            serde_json::Value::Array(vec![serde_json::Value::Number((i as u32).into())]),
        );
        ports.insert(format!("port_{}", i), serde_json::Value::Object(port));
    }

    let data = serde_json::json!({
        "creator": "Yosys 0.9 (benchmark)",
        "modules": {
            "top": {
                "cells": cells,
                "ports": ports,
            }
        }
    });

    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{}", serde_json::to_string(&data).unwrap()).unwrap();
    tmp
}

fn bench_yosys_json_dom_vs_sax(c: &mut Criterion) {
    // --- Synthetic files at controlled sizes ---
    let small = create_yosys_json_file(100, 10);
    let med = create_yosys_json_file(500, 50);
    let large = create_yosys_json_file(2000, 200);

    let mut group = c.benchmark_group("yosys_json_parse_synthetic");

    for (label, path) in [("small_100", small.path()), ("med_500", med.path()), ("large_2k", large.path())] {
        group.bench_with_input(BenchmarkId::new("dom", label), path, |b, p| {
            b.iter(|| black_box(read_yosys_json(p).unwrap()));
        });
        group.bench_with_input(BenchmarkId::new("sax", label), path, |b, p| {
            b.iter(|| black_box(read_yosys_json_sax(p).unwrap()));
        });
    }

    group.finish();

    // --- Real Yosys netlist file ---
    let real_path = Path::new("yosys_testcases/sphere_netlist.json");
    if real_path.exists() && read_yosys_json(real_path).is_ok() && read_yosys_json_sax(real_path).is_ok() {
        let mut group = c.benchmark_group("yosys_json_parse_real");
        group.bench_with_input(BenchmarkId::new("dom", "sphere"), real_path, |b, p| {
            b.iter(|| black_box(read_yosys_json(p).unwrap()));
        });
        group.bench_with_input(BenchmarkId::new("sax", "sphere"), real_path, |b, p| {
            b.iter(|| black_box(read_yosys_json_sax(p).unwrap()));
        });
        group.finish();
    }
}

// Verify correctness: DOM and SAX produce identical results
#[test]
fn test_sax_matches_dom_across_sizes() {
    for (n_cells, n_ports) in [(0, 2), (10, 3), (100, 20)] {
        let tmp = create_yosys_json_file(n_cells, n_ports);
        let dom = read_yosys_json(tmp.path()).unwrap();
        let sax = read_yosys_json_sax(tmp.path()).unwrap();
        assert_eq!(dom.num_modules(), sax.num_modules());
        assert_eq!(dom.num_nets(), sax.num_nets());
        assert_eq!(dom.num_pads, sax.num_pads);
        assert_eq!(dom.number_of_nodes(), sax.number_of_nodes());
        assert_eq!(dom.grph.edge_count(), sax.grph.edge_count());
    }
}

criterion_group!(benches, bench_yosys_json_dom_vs_sax);
criterion_main!(benches);
