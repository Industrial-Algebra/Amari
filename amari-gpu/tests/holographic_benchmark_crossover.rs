#![cfg(feature = "holographic")]

//! Manual benchmark/crossover harnesses for holographic and optical GPU paths.

use amari_gpu::holographic::ProductCl3x32;
use amari_gpu::{GpuHolographic, GpuOpticalField};
use amari_holographic::optical::{LeeEncoderConfig, OpticalRotorField};
use amari_holographic::BindingAlgebra;
use std::time::{Duration, Instant};

fn make_basis_batch(batch_size: usize) -> (Vec<f64>, Vec<f64>) {
    let dim = ProductCl3x32::DIMENSION;
    let mut keys = vec![0.0; batch_size * dim];
    let mut values = vec![0.0; batch_size * dim];
    for batch in 0..batch_size {
        let start = batch * dim;
        keys[start + (batch % 8)] = 1.0;
        values[start + ((batch * 3 + 1) % 8)] = 1.0;
    }
    (keys, values)
}

fn cpu_bind(keys: &[f64], values: &[f64]) -> Vec<f64> {
    let dim = ProductCl3x32::DIMENSION;
    let mut output = Vec::with_capacity(keys.len());
    for (key, value) in keys.chunks_exact(dim).zip(values.chunks_exact(dim)) {
        let key = ProductCl3x32::from_coefficients(key).unwrap();
        let value = ProductCl3x32::from_coefficients(value).unwrap();
        output.extend(key.bind(&value).to_coefficients());
    }
    output
}

fn cpu_similarity(left: &[f64], right: &[f64]) -> Vec<f64> {
    let dim = ProductCl3x32::DIMENSION;
    left.chunks_exact(dim)
        .zip(right.chunks_exact(dim))
        .map(|(a, b)| {
            let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            let na = a.iter().map(|x| x * x).sum::<f64>().sqrt();
            let nb = b.iter().map(|x| x * x).sum::<f64>().sqrt();
            if na > 0.0 && nb > 0.0 {
                dot / (na * nb)
            } else {
                0.0
            }
        })
        .collect()
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

async fn avg_gpu_bind(
    gpu: &GpuHolographic,
    keys: &[f64],
    values: &[f64],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_bind(keys, values).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_bind(keys, values).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_similarity(
    gpu: &GpuHolographic,
    left: &[f64],
    right: &[f64],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_similarity(left, right).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_similarity(left, right).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_optical_bind(
    gpu: &GpuOpticalField,
    a: &OpticalRotorField,
    b: &OpticalRotorField,
    warmups: usize,
    runs: usize,
) -> Duration {
    for _ in 0..warmups {
        std::hint::black_box(gpu.bind(a, b).await.unwrap());
    }
    let mut total = Duration::ZERO;
    for _ in 0..runs {
        let start = Instant::now();
        std::hint::black_box(gpu.bind(a, b).await.unwrap());
        total += start.elapsed();
    }
    total / runs as u32
}

async fn avg_optical_similarity(
    gpu: &GpuOpticalField,
    a: &OpticalRotorField,
    b: &OpticalRotorField,
    warmups: usize,
    runs: usize,
) -> Duration {
    for _ in 0..warmups {
        std::hint::black_box(gpu.similarity(a, b).await.unwrap());
    }
    let mut total = Duration::ZERO;
    for _ in 0..runs {
        let start = Instant::now();
        std::hint::black_box(gpu.similarity(a, b).await.unwrap());
        total += start.elapsed();
    }
    total / runs as u32
}

async fn avg_optical_lee(
    gpu: &GpuOpticalField,
    field: &OpticalRotorField,
    config: &LeeEncoderConfig,
    warmups: usize,
    runs: usize,
) -> Duration {
    for _ in 0..warmups {
        std::hint::black_box(gpu.encode_lee(field, config).await.unwrap());
    }
    let mut total = Duration::ZERO;
    for _ in 0..runs {
        let start = Instant::now();
        std::hint::black_box(gpu.encode_lee(field, config).await.unwrap());
        total += start.elapsed();
    }
    total / runs as u32
}

fn print_row(label: &str, size: usize, cpu: Duration, gpu: Duration, correct: bool) {
    let cpu_ms = cpu.as_secs_f64() * 1000.0;
    let gpu_ms = gpu.as_secs_f64() * 1000.0;
    let speedup = if gpu_ms > 0.0 {
        cpu_ms / gpu_ms
    } else {
        f64::INFINITY
    };
    println!(
        "{}\t{}\t{:.3}\t{:.3}\t{:.2}x\t{}",
        label, size, cpu_ms, gpu_ms, speedup, correct
    );
}

#[tokio::test]
#[ignore = "Manual benchmark harness for holographic/optical crossover work"]
async fn benchmark_holographic_and_optical_cpu_vs_gpu() {
    let warmups = 1;
    let runs = 5;

    if let Ok(gpu) = GpuHolographic::new_product_cl3x32().await {
        println!("\nHolographic ProductCl3x32 benchmark (CPU vs GPU)");
        println!("operation\tbatch_or_size\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");
        for batch_size in [16usize, 100, 512, 2048] {
            let (keys, values) = make_basis_batch(batch_size);
            let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_bind(&keys, &values));
            let (gpu_out, gpu_avg) = avg_gpu_bind(&gpu, &keys, &values, warmups, runs).await;
            assert_close(&gpu_out, &cpu, 1e-6);
            print_row("bind", batch_size, cpu_avg, gpu_avg, true);

            let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_similarity(&keys, &keys));
            let (gpu_out, gpu_avg) = avg_gpu_similarity(&gpu, &keys, &keys, warmups, runs).await;
            assert_close(&gpu_out, &cpu, 1e-6);
            print_row("similarity", batch_size, cpu_avg, gpu_avg, true);
        }
    } else {
        eprintln!("Skipping holographic benchmark harness: GPU unavailable");
    }

    println!("\nOptical holographic benchmark (GPU path timings)");
    println!("operation\tbatch_or_size\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");
    for dims in [(16usize, 16usize), (64, 64), (128, 128)] {
        let gpu = match GpuOpticalField::new(dims).await {
            Ok(gpu) => gpu,
            Err(err) => {
                eprintln!("Skipping optical {:?}: {err}", dims);
                continue;
            }
        };
        let a = OpticalRotorField::uniform(0.0, 1.0, dims);
        let b = OpticalRotorField::uniform(std::f32::consts::FRAC_PI_4, 1.0, dims);
        let config = LeeEncoderConfig::new(dims, 0.25);
        let size = dims.0 * dims.1;

        let bind = avg_optical_bind(&gpu, &a, &b, warmups, runs).await;
        print_row("optical_bind", size, Duration::ZERO, bind, true);
        let sim = avg_optical_similarity(&gpu, &a, &a, warmups, runs).await;
        print_row("optical_similarity", size, Duration::ZERO, sim, true);
        let lee = avg_optical_lee(&gpu, &a, &config, warmups, runs).await;
        print_row("lee_encode", size, Duration::ZERO, lee, true);
    }
}
