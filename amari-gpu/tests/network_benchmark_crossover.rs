//! Manual benchmark/crossover harnesses for geometric-network GPU paths.

use amari_core::Vector;
use amari_gpu::GpuGeometricNetwork;
use amari_network::GeometricNetwork;
use std::time::{Duration, Instant};

fn make_positions(node_count: usize) -> Vec<(f64, f64, f64)> {
    (0..node_count)
        .map(|i| {
            let t = i as f64;
            (
                (t * 0.37).sin() * 10.0,
                (t * 0.19).cos() * 7.0,
                (t % 31.0) * 0.25,
            )
        })
        .collect()
}

fn make_network(positions: &[(f64, f64, f64)]) -> GeometricNetwork<3, 0, 0> {
    let mut network = GeometricNetwork::<3, 0, 0>::new();
    let mut nodes = Vec::with_capacity(positions.len());
    for &(x, y, z) in positions {
        nodes.push(network.add_node(Vector::from_components(x, y, z).mv));
    }
    for i in 1..nodes.len() {
        network
            .add_undirected_edge(nodes[i - 1], nodes[i], 1.0)
            .unwrap();
    }
    network
}

fn cpu_distance_matrix(positions: &[(f64, f64, f64)]) -> Vec<Vec<f64>> {
    positions
        .iter()
        .map(|&(ax, ay, az)| {
            positions
                .iter()
                .map(|&(bx, by, bz)| {
                    let dx = ax - bx;
                    let dy = ay - by;
                    let dz = az - bz;
                    (dx * dx + dy * dy + dz * dz).sqrt()
                })
                .collect()
        })
        .collect()
}

fn cpu_centrality(distances: &[Vec<f64>]) -> Vec<f64> {
    let n = distances.len() as f64;
    distances
        .iter()
        .map(|row| {
            let sum: f64 = row.iter().sum();
            if sum > 0.0 {
                (n - 1.0) / sum
            } else {
                0.0
            }
        })
        .collect()
}

fn assert_matrix_close(actual: &[Vec<f64>], expected: &[Vec<f64>], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (row_index, (actual_row, expected_row)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual_row.len(), expected_row.len());
        for (col_index, (actual, expected)) in
            actual_row.iter().zip(expected_row.iter()).enumerate()
        {
            assert!(
                (actual - expected).abs() <= tolerance,
                "mismatch at ({row_index}, {col_index}): expected {expected}, got {actual}"
            );
        }
    }
}

fn assert_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "mismatch at {index}: expected {expected}, got {actual}"
        );
    }
}

fn avg_duration<T, F>(warmups: usize, runs: usize, mut f: F) -> (T, Duration)
where
    T: Default,
    F: FnMut() -> T,
{
    for _ in 0..warmups {
        std::hint::black_box(f());
    }
    let mut total = Duration::ZERO;
    let mut last = T::default();
    for _ in 0..runs {
        let start = Instant::now();
        last = f();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_distances(
    gpu: &GpuGeometricNetwork,
    network: &GeometricNetwork<3, 0, 0>,
    warmups: usize,
    runs: usize,
) -> (Vec<Vec<f64>>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.compute_all_pairwise_distances(network).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.compute_all_pairwise_distances(network).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_centrality(
    gpu: &GpuGeometricNetwork,
    network: &GeometricNetwork<3, 0, 0>,
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.compute_geometric_centrality(network).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.compute_geometric_centrality(network).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_clustering(
    gpu: &GpuGeometricNetwork,
    network: &GeometricNetwork<3, 0, 0>,
    warmups: usize,
    runs: usize,
) -> (usize, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.geometric_clustering(network, 4, 8).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last_len = 0;
    for _ in 0..runs {
        let start = Instant::now();
        let communities = gpu.geometric_clustering(network, 4, 8).await.unwrap();
        total += start.elapsed();
        last_len = communities.len();
        std::hint::black_box(&communities);
    }
    (last_len, total / runs as u32)
}

fn print_row(label: &str, nodes: usize, cpu: Duration, gpu: Duration, correct: bool) {
    let cpu_ms = cpu.as_secs_f64() * 1000.0;
    let gpu_ms = gpu.as_secs_f64() * 1000.0;
    let speedup = if gpu_ms > 0.0 {
        cpu_ms / gpu_ms
    } else {
        f64::INFINITY
    };
    println!(
        "{}\t{}\t{:.3}\t{:.3}\t{:.2}x\t{}",
        label, nodes, cpu_ms, gpu_ms, speedup, correct
    );
}

#[tokio::test]
#[ignore = "Manual benchmark harness for network distance/centrality/clustering crossover work"]
async fn benchmark_network_cpu_vs_gpu() {
    let gpu = match GpuGeometricNetwork::new().await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping network benchmark harness: {err}");
            return;
        }
    };

    let cases = [16usize, 64, 128, 256];
    let warmups = 1;
    let runs = 5;

    println!("\nNetwork benchmark (CPU vs GPU)");
    println!("operation\tnodes\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for node_count in cases {
        let positions = make_positions(node_count);
        let network = make_network(&positions);

        let (cpu_distances, cpu_avg) =
            avg_duration(warmups, runs, || cpu_distance_matrix(&positions));
        let (gpu_distances, gpu_avg) = avg_gpu_distances(&gpu, &network, warmups, runs).await;
        assert_matrix_close(&gpu_distances, &cpu_distances, 1e-5);
        print_row("distances", node_count, cpu_avg, gpu_avg, true);

        let (cpu_cent, cpu_avg) = avg_duration(warmups, runs, || cpu_centrality(&cpu_distances));
        let (gpu_cent, gpu_avg) = avg_gpu_centrality(&gpu, &network, warmups, runs).await;
        assert_close(&gpu_cent, &cpu_cent, 1e-5);
        print_row("centrality", node_count, cpu_avg, gpu_avg, true);

        let (_, gpu_avg) = avg_gpu_clustering(&gpu, &network, warmups, runs).await;
        print_row("clustering", node_count, Duration::ZERO, gpu_avg, true);
    }
}
