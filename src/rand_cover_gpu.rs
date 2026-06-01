//! GPU-Accelerated Randomized Vertex Cover using Pitt's Algorithm
//!
//! Ported from Python `rand_cover_gpu.py`.
//!
//! Runs multiple independent Pitt trials in parallel on the GPU via CUDA.
//! Each CUDA thread executes one complete Pitt trial (edge iteration + random
//! vertex selection). After all trials complete, the best (lowest cost) cover
//! is returned.
//!
//! Requires the `cuda` feature flag and CUDA toolkit installation.
//! Falls back to CPU multi-threaded execution via Rayon when CUDA is unavailable.

use std::collections::HashMap;
use std::collections::HashSet;

use rand::Rng;
use rand::SeedableRng;

#[allow(dead_code)]
const THREADS_PER_BLOCK: u32 = 64;

/// CUDA C kernel source for Pitt's randomized vertex cover algorithm.
///
/// Each thread runs one independent trial. Cover sets are stored as
/// bitmasks (uint32 words). Uses LCG for per-thread RNG.
#[allow(dead_code)]
const PITT_KERNEL_CUDA: &str = r#"
extern "C" __global__ void pitt_kernel(
    const int* edges, int num_edges,
    const float* weights, int num_vertices,
    unsigned int* covers, float* costs,
    unsigned long long* seeds, int num_trials
) {
    int tid = blockIdx.x * blockDim.x + threadIdx.x;
    if (tid >= num_trials) return;

    unsigned long long seed = seeds[tid];
    unsigned int* cover = &covers[tid * ((num_vertices + 31) / 32)];

    for (int i = 0; i < num_edges; i++) {
        int u = edges[i * 2];
        int v = edges[i * 2 + 1];

        int u_word = u >> 5;
        int v_word = v >> 5;
        unsigned int u_bit = 1u << (u & 31);
        unsigned int v_bit = 1u << (v & 31);

        if ((cover[u_word] & u_bit) == 0 && (cover[v_word] & v_bit) == 0) {
            seed = seed * 1103515245ull + 12345ull;
            float rand_val = (float)(seed & 0x7FFFFFFFull) / 2147483648.0f;

            float w_u = weights[u];
            float w_v = weights[v];
            float threshold = w_v / (w_u + w_v);

            if (rand_val < threshold) {
                cover[u_word] |= u_bit;
            } else {
                cover[v_word] |= v_bit;
            }
        }
    }

    float cost = 0.0f;
    for (int v = 0; v < num_vertices; v++) {
        int v_word = v >> 5;
        unsigned int v_bit = 1u << (v & 31);
        if (cover[v_word] & v_bit) {
            cost += weights[v];
        }
    }
    costs[tid] = cost;
}
"#;

/// Find a minimum weighted vertex cover using GPU-accelerated Pitt's algorithm.
///
/// Runs `num_trials` independent randomized Pitt trials in parallel on the
/// GPU and returns the cover with the lowest total weight.
///
/// When the `cuda` feature is enabled and a CUDA-capable GPU is available,
/// trials execute on the GPU. Otherwise falls back to CPU multi-threading
/// via Rayon (if enabled) or sequential execution.
pub fn rand_vertex_cover_gpu<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
    coverset: &HashSet<String>,
    num_trials: usize,
    seed: u64,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default + Send + Sync,
{
    // Handle empty graph
    if grph.edge_count() == 0 {
        let total_cost = coverset
            .iter()
            .map(|v| weight.get(v).copied().unwrap_or(W::default()))
            .fold(W::default(), |acc, w| acc + w);
        return (coverset.clone(), total_cost);
    }

    // Map vertices to consecutive integer indices
    let vertices: Vec<String> = grph.node_indices().map(|i| grph[i].clone()).collect();
    let n_vertices = vertices.len();
    let vertex_to_idx: HashMap<&str, usize> = vertices
        .iter()
        .enumerate()
        .map(|(i, v)| (v.as_str(), i))
        .collect();

    // Build edge array: flat [u0, v0, u1, v1, ...]
    let mut edges_flat: Vec<i32> = Vec::with_capacity(grph.edge_count() * 2);
    for edge in grph.raw_edges() {
        let u = &grph[edge.source()];
        let v = &grph[edge.target()];
        edges_flat.push(vertex_to_idx[u.as_str()] as i32);
        edges_flat.push(vertex_to_idx[v.as_str()] as i32);
    }

    // Build weight array
    let weights_flat: Vec<f32> = vertices
        .iter()
        .map(|v| {
            weight
                .get(v)
                .copied()
                .map(|w| w.into() as f32)
                .unwrap_or(1.0)
        })
        .collect();

    // Generate seeds for each trial
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let seeds: Vec<u64> = (0..num_trials).map(|_| rng.gen()).collect();

    let num_words = (n_vertices + 31) / 32;

    // Try GPU execution first
    #[cfg(feature = "cuda")]
    {
        match try_gpu_execution(
            &edges_flat,
            grph.edge_count(),
            &weights_flat,
            n_vertices,
            &seeds,
            num_trials,
            num_words,
            coverset,
            &vertex_to_idx,
            &vertices,
            weight,
        ) {
            Ok(result) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[rand_cover_gpu] ✅ GPU execution succeeded ({} trials)",
                    num_trials
                );
                return result;
            }
            Err(e) => {
                eprintln!(
                    "[rand_cover_gpu] ⚠️ CUDA failed: {} — falling back to CPU",
                    e
                );
            }
        }
    }

    // Fallback: CPU multi-threaded or sequential
    cpu_multi_trial(
        &edges_flat,
        grph.edge_count(),
        &weights_flat,
        n_vertices,
        &seeds,
        num_trials,
        num_words,
        coverset,
        &vertex_to_idx,
        &vertices,
        weight,
    )
}

/// Try to execute trials on GPU.
#[cfg(feature = "cuda")]
fn try_gpu_execution<W>(
    edges_flat: &[i32],
    num_edges: usize,
    weights_flat: &[f32],
    n_vertices: usize,
    seeds: &[u64],
    num_trials: usize,
    num_words: usize,
    coverset: &HashSet<String>,
    vertex_to_idx: &HashMap<&str, usize>,
    vertices: &[String],
    weight: &HashMap<String, W>,
) -> Result<(HashSet<String>, W), String>
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default,
{
    use cudarc::driver::safe::{CudaDevice, LaunchAsync, LaunchConfig};
    use cudarc::nvrtc::safe::compile_ptx;
    use std::panic::catch_unwind;

    let dev = match catch_unwind(|| CudaDevice::new(0)) {
        Ok(Ok(dev)) => dev,
        Ok(Err(e)) => return Err(format!("CUDA device error: {:?}", e)),
        Err(_) => {
            return Err(
                "CUDA driver not available (cudarc panicked loading shared library)".to_string(),
            )
        }
    };

    let ptx =
        compile_ptx(PITT_KERNEL_CUDA).map_err(|e| format!("PTX compilation error: {:?}", e))?;

    dev.load_ptx(ptx, "pitt_module", &["pitt_kernel"])
        .map_err(|e| format!("Module load error: {:?}", e))?;
    let func = dev
        .get_func("pitt_module", "pitt_kernel")
        .ok_or_else(|| "Function not found after loading".to_string())?;

    let d_edges = dev
        .htod_copy(edges_flat.to_vec())
        .map_err(|e| format!("Edge alloc error: {:?}", e))?;
    let d_weights = dev
        .htod_copy(weights_flat.to_vec())
        .map_err(|e| format!("Weight alloc error: {:?}", e))?;
    let d_seeds = dev
        .htod_copy(seeds.to_vec())
        .map_err(|e| format!("Seed alloc error: {:?}", e))?;

    let cover_bytes = num_trials * num_words;
    let mut covers_host = vec![0u32; cover_bytes];
    for v in coverset {
        if let Some(&vi) = vertex_to_idx.get(v.as_str()) {
            let word = vi >> 5;
            let bit = 1u32 << (vi & 31);
            for t in 0..num_trials {
                covers_host[t * num_words + word] |= bit;
            }
        }
    }
    let d_covers = dev
        .htod_copy(covers_host.clone())
        .map_err(|e| format!("Cover alloc error: {:?}", e))?;
    let mut d_costs = dev
        .alloc_zeros::<f32>(num_trials)
        .map_err(|e| format!("Cost alloc error: {:?}", e))?;

    let num_trials_i32 = num_trials as i32;
    let n_edges_i32 = num_edges as i32;
    let n_vert_i32 = n_vertices as i32;
    let grid_blocks = (num_trials as u32 + THREADS_PER_BLOCK - 1) / THREADS_PER_BLOCK;

    unsafe {
        func.launch(
            LaunchConfig::for_num_elems(grid_blocks * THREADS_PER_BLOCK),
            (
                &d_edges,
                n_edges_i32,
                &d_weights,
                n_vert_i32,
                &d_covers,
                &mut d_costs,
                &d_seeds,
                num_trials_i32,
            ),
        )
        .map_err(|e| format!("Kernel launch error: {:?}", e))?;
    }

    dev.synchronize()
        .map_err(|e| format!("Sync error: {:?}", e))?;

    // Copy results back
    let covers_result = dev
        .dtoh_sync_copy(&d_covers)
        .map_err(|e| format!("Cover copy error: {:?}", e))?;
    let costs_result = dev
        .dtoh_sync_copy(&d_costs)
        .map_err(|e| format!("Cost copy error: {:?}", e))?;

    let best_idx = costs_result
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let best_mask = &covers_result[best_idx * num_words..(best_idx + 1) * num_words];
    let mut soln: HashSet<String> = HashSet::new();
    for vi in 0..n_vertices {
        if (best_mask[vi >> 5] >> (vi & 31)) & 1 != 0 {
            soln.insert(vertices[vi].clone());
        }
    }

    let total_cost: W = soln
        .iter()
        .map(|v| weight.get(v).copied().unwrap_or(W::default()))
        .fold(W::default(), |acc, w| acc + w);

    Ok((soln, total_cost))
}

/// Run trials on CPU via Rayon (parallel) or sequentially.
#[allow(clippy::too_many_arguments)]
fn cpu_multi_trial<W>(
    edges_flat: &[i32],
    num_edges: usize,
    weights_flat: &[f32],
    n_vertices: usize,
    seeds: &[u64],
    num_trials: usize,
    num_words: usize,
    coverset: &HashSet<String>,
    vertex_to_idx: &HashMap<&str, usize>,
    vertices: &[String],
    weight: &HashMap<String, W>,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default + Send + Sync,
{
    // Pre-compute initial cover bitmask
    let mut initial_cover = vec![0u32; num_words];
    for v in coverset {
        if let Some(&vi) = vertex_to_idx.get(v.as_str()) {
            let word = vi >> 5;
            let bit = 1u32 << (vi & 31);
            initial_cover[word] |= bit;
        }
    }

    // Run trials
    #[cfg(feature = "rayon")]
    let results: Vec<(HashSet<String>, f32)> = {
        use rayon::prelude::*;
        (0..num_trials)
            .into_par_iter()
            .map(|t| {
                let mut cover = initial_cover.clone();
                let seed = seeds[t];
                run_single_trial(
                    &mut cover,
                    edges_flat,
                    num_edges,
                    weights_flat,
                    n_vertices,
                    seed,
                    num_words,
                );
                let cost = compute_cover_cost(&cover, weights_flat, n_vertices);
                let soln = extract_cover(&cover, vertices, n_vertices, num_words);
                (soln, cost)
            })
            .collect()
    };

    #[cfg(not(feature = "rayon"))]
    let results: Vec<(HashSet<String>, f32)> = {
        (0..num_trials)
            .map(|t| {
                let mut cover = initial_cover.clone();
                let seed = seeds[t];
                run_single_trial(
                    &mut cover,
                    edges_flat,
                    num_edges,
                    weights_flat,
                    n_vertices,
                    seed,
                    num_words,
                );
                let cost = compute_cover_cost(&cover, weights_flat, n_vertices);
                let soln = extract_cover(&cover, vertices, n_vertices, num_words);
                (soln, cost)
            })
            .collect()
    };

    // Find best trial
    let (best_soln, _best_cost) = results
        .into_iter()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_default();

    let total_cost: W = best_soln
        .iter()
        .map(|v| weight.get(v).copied().unwrap_or(W::default()))
        .fold(W::default(), |acc, w| acc + w);

    (best_soln, total_cost)
}

/// Run a single Pitt trial on CPU.
fn run_single_trial(
    cover: &mut [u32],
    edges_flat: &[i32],
    num_edges: usize,
    weights_flat: &[f32],
    _n_vertices: usize,
    mut seed: u64,
    _num_words: usize,
) {
    for i in 0..num_edges {
        let u = edges_flat[i * 2] as usize;
        let v = edges_flat[i * 2 + 1] as usize;

        let u_word = u >> 5;
        let v_word = v >> 5;
        let u_bit = 1u32 << (u & 31);
        let v_bit = 1u32 << (v & 31);

        if (cover[u_word] & u_bit) == 0 && (cover[v_word] & v_bit) == 0 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7FFF_FFFF;
            let rand_val = seed as f64 / 2147483648.0;

            let w_u = weights_flat[u];
            let w_v = weights_flat[v];
            let threshold = w_v / (w_u + w_v);

            if rand_val < threshold as f64 {
                cover[u_word] |= u_bit;
            } else {
                cover[v_word] |= v_bit;
            }
        }
    }
}

/// Compute total cost of a cover from bitmask.
fn compute_cover_cost(cover: &[u32], weights: &[f32], n_vertices: usize) -> f32 {
    let mut cost = 0.0;
    for v in 0..n_vertices.min(weights.len()) {
        if (cover[v >> 5] >> (v & 31)) & 1 != 0 {
            cost += weights[v];
        }
    }
    cost
}

/// Extract vertex names from bitmask.
fn extract_cover(
    cover: &[u32],
    vertices: &[String],
    n_vertices: usize,
    _num_words: usize,
) -> HashSet<String> {
    let mut soln = HashSet::new();
    for vi in 0..n_vertices.min(vertices.len()) {
        if (cover[vi >> 5] >> (vi & 31)) & 1 != 0 {
            soln.insert(vertices[vi].clone());
        }
    }
    soln
}

/// Convenience wrapper with num_trials defaulting to 1024.
pub fn rand_vertex_cover_gpu_default<W>(
    grph: &petgraph::Graph<String, (), petgraph::Undirected>,
    weight: &HashMap<String, W>,
) -> (HashSet<String>, W)
where
    W: Copy + Into<f64> + std::ops::Add<Output = W> + std::cmp::PartialOrd + Default + Send + Sync,
{
    let coverset = HashSet::new();
    rand_vertex_cover_gpu(grph, weight, &coverset, 1024, 42)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph(edges: &[(u32, u32)]) -> petgraph::Graph<String, (), petgraph::Undirected> {
        use petgraph::graph::UnGraph;
        let mut grph = UnGraph::new_undirected();
        let mut indices = std::collections::HashMap::new();
        for &(u, v) in edges {
            let ku = format!("n{}", u);
            let kv = format!("n{}", v);
            if !indices.contains_key(&ku) {
                indices.insert(ku.clone(), grph.add_node(ku.clone()));
            }
            if !indices.contains_key(&kv) {
                indices.insert(kv.clone(), grph.add_node(kv.clone()));
            }
            grph.add_edge(indices[&ku], indices[&kv], ());
        }
        grph
    }

    #[test]
    fn test_gpu_triangle() {
        let grph = make_graph(&[(0, 1), (0, 2), (1, 2)]);
        let weight: HashMap<String, i32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let coverset = HashSet::new();
        let (soln, cost) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 64, 42);
        // Pitt's algorithm should find a valid cover
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(
                soln.contains(u) || soln.contains(v),
                "Edge ({},{}) uncovered",
                u,
                v
            );
        }
        // Triangle optimal cover size = 2
        assert_eq!(soln.len(), 2);
        assert_eq!(cost, 2);
    }

    #[test]
    fn test_gpu_line() {
        let grph = make_graph(&[(0, 1), (1, 2)]);
        let weight: HashMap<String, i32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let coverset = HashSet::new();
        let (soln, _cost) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 64, 42);
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(soln.contains(u) || soln.contains(v));
        }
    }

    #[test]
    fn test_gpu_empty_graph() {
        let grph = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
        let weight: HashMap<String, i32> = HashMap::new();
        let coverset = HashSet::new();
        let (soln, cost) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 64, 42);
        assert!(soln.is_empty());
        assert_eq!(cost, 0);
    }

    #[test]
    fn test_gpu_weighted_edge() {
        let mut grph = petgraph::Graph::<String, (), petgraph::Undirected>::new_undirected();
        let n0 = grph.add_node("heavy".to_string());
        let n1 = grph.add_node("light".to_string());
        grph.add_edge(n0, n1, ());
        let weight: HashMap<String, i32> = [("heavy".to_string(), 100), ("light".to_string(), 1)]
            .iter()
            .cloned()
            .collect();
        let coverset = HashSet::new();
        let (soln, cost) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 128, 42);
        // Should prefer picking the light vertex
        assert!(soln.contains("light"), "Expected light vertex in cover");
        assert_eq!(cost, 1);
    }

    #[test]
    fn test_gpu_with_initial_coverset() {
        let grph = make_graph(&[(0, 1), (1, 2), (2, 0)]);
        let weight: HashMap<String, i32> = [
            ("n0".to_string(), 1),
            ("n1".to_string(), 1),
            ("n2".to_string(), 1),
        ]
        .iter()
        .cloned()
        .collect();
        let coverset: HashSet<String> = [("n0".to_string())].iter().cloned().collect();
        let (soln, _cost) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 64, 42);
        assert!(soln.contains("n0"), "Initial vertex should remain in cover");
        for edge in grph.raw_edges() {
            let u = &grph[edge.source()];
            let v = &grph[edge.target()];
            assert!(soln.contains(u) || soln.contains(v));
        }
    }

    #[test]
    fn test_gpu_deterministic_seed() {
        let grph = make_graph(&[(0, 1), (1, 2), (2, 3), (3, 0)]);
        let weight: HashMap<String, i32> = [
            ("n0".to_string(), 2),
            ("n1".to_string(), 3),
            ("n2".to_string(), 1),
            ("n3".to_string(), 4),
        ]
        .iter()
        .cloned()
        .collect();
        let coverset = HashSet::new();
        let (sol1, cost1) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 128, 123);
        let (sol2, cost2) = rand_vertex_cover_gpu(&grph, &weight, &coverset, 128, 123);
        assert_eq!(sol1, sol2, "Same seed should produce same result");
        assert_eq!(cost1, cost2, "Same seed should produce same cost");
    }
}
