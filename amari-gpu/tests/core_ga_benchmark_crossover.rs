//! Manual benchmark/crossover harnesses for amari-gpu core GA paths.
//!
//! These tests are ignored by default because they are hardware diagnostics, not
//! deterministic CI checks. Run with:
//!
//! ```bash
//! cargo +stable test -p amari-gpu --test core_ga_benchmark_crossover -- --ignored --nocapture --test-threads=1
//! ```

use amari_core::Multivector;
use amari_gpu::GpuCliffordAlgebra;
use std::time::{Duration, Instant};

fn make_multivector(seed: usize) -> Multivector<3, 0, 0> {
    let coeffs = (0..8)
        .map(|i| (((seed * 31 + i * 17) % 97) as f64 - 48.0) / 97.0)
        .collect();
    Multivector::<3, 0, 0>::from_coefficients(coeffs)
}

fn make_flat_batches(batch_size: usize) -> (Vec<f64>, Vec<f64>) {
    let mut a = Vec::with_capacity(batch_size * 8);
    let mut b = Vec::with_capacity(batch_size * 8);
    for i in 0..batch_size {
        a.extend(make_multivector(i).to_vec());
        b.extend(make_multivector(i ^ 0x5A5A).to_vec());
    }
    (a, b)
}

fn cpu_batch_geometric_product(a_batch: &[f64], b_batch: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(a_batch.len());
    for (a, b) in a_batch.chunks_exact(8).zip(b_batch.chunks_exact(8)) {
        let a = Multivector::<3, 0, 0>::from_slice(a);
        let b = Multivector::<3, 0, 0>::from_slice(b);
        out.extend(a.geometric_product(&b).to_vec());
    }
    out
}

fn avg_duration<F>(warmups: usize, runs: usize, mut f: F) -> (Vec<f64>, Duration)
where
    F: FnMut() -> Vec<f64>,
{
    for _ in 0..warmups {
        std::hint::black_box(f());
    }

    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = f();
        total += start.elapsed();
        std::hint::black_box(&last);
    }

    (last, total / runs as u32)
}

async fn avg_gpu_duration(
    gpu: &GpuCliffordAlgebra,
    a: &[f64],
    b: &[f64],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_geometric_product(a, b).await.unwrap());
    }

    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_geometric_product(a, b).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }

    (last, total / runs as u32)
}

fn assert_close(a: &[f64], b: &[f64], tolerance: f64) {
    assert_eq!(a.len(), b.len());
    for (index, (actual, expected)) in a.iter().zip(b.iter()).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "mismatch at {index}: expected {expected}, got {actual}"
        );
    }
}

#[tokio::test]
#[ignore = "Manual GB10/RTX benchmark harness for core GA batch crossover work"]
async fn benchmark_core_ga_batch_geometric_product_cpu_vs_gpu() {
    let gpu = match GpuCliffordAlgebra::new::<3, 0, 0>().await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping core GA benchmark harness: {err}");
            return;
        }
    };

    let cases = [16usize, 64, 256, 1024, 4096];
    let warmups = 1;
    let runs = 5;

    println!("\nCore GA batch geometric product benchmark (CPU vs GPU)");
    println!("batch_size\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for batch_size in cases {
        let (a, b) = make_flat_batches(batch_size);
        let (cpu_result, cpu_avg) =
            avg_duration(warmups, runs, || cpu_batch_geometric_product(&a, &b));
        let (gpu_result, gpu_avg) = avg_gpu_duration(&gpu, &a, &b, warmups, runs).await;
        assert_close(&gpu_result, &cpu_result, 1e-5);

        let cpu_ms = cpu_avg.as_secs_f64() * 1000.0;
        let gpu_ms = gpu_avg.as_secs_f64() * 1000.0;
        let speedup = if gpu_ms > 0.0 {
            cpu_ms / gpu_ms
        } else {
            f64::INFINITY
        };
        println!(
            "{}\t{:.3}\t{:.3}\t{:.2}x\ttrue",
            batch_size, cpu_ms, gpu_ms, speedup
        );
    }
}
