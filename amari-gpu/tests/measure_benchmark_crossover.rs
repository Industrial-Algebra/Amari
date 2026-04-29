#![cfg(feature = "measure")]

//! Manual benchmark/crossover harnesses for measure/integration GPU paths.

use amari_gpu::{GpuIntegrator, GpuMultidimIntegrator, GpuParametricDensity, GpuTropicalMeasure};
use std::time::{Duration, Instant};

fn evaluate_function(x: f32, function_id: u32) -> f32 {
    match function_id {
        0 => x,
        1 => x * x,
        2 => x * x * x,
        3 => x.sin(),
        4 => x.cos(),
        5 => x.exp(),
        _ => 1.0,
    }
}

fn cpu_integrate_uniform(a: f32, b: f32, n: u32, function_id: u32) -> f32 {
    let dx = (b - a) / n as f32;
    (0..n)
        .map(|i| evaluate_function(a + (i as f32 + 0.5) * dx, function_id) * dx)
        .sum()
}

fn make_values(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| ((i * 17 % 1009) as f32 - 504.0) / 97.0)
        .collect()
}

fn cpu_gaussian(values: &[f32], mean: f32, sigma: f32) -> Vec<f32> {
    let norm = 1.0 / (sigma * (2.0 * std::f32::consts::PI).sqrt());
    values
        .iter()
        .map(|x| {
            let z = (*x - mean) / sigma;
            norm * (-0.5 * z * z).exp()
        })
        .collect()
}

fn cpu_supremum(values: &[f32]) -> f32 {
    values.iter().copied().fold(f32::NEG_INFINITY, f32::max)
}

fn cpu_infimum(values: &[f32]) -> f32 {
    values.iter().copied().fold(f32::INFINITY, f32::min)
}

fn assert_close(actual: &[f32], expected: &[f32], tolerance: f32) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "mismatch at {index}"
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

async fn avg_gpu_integrate(
    gpu: &GpuIntegrator,
    n: u32,
    warmups: usize,
    runs: usize,
) -> (f32, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.integrate_uniform(0.0, 2.0, n, 1).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = 0.0;
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.integrate_uniform(0.0, 2.0, n, 1).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_gaussian(
    gpu: &GpuParametricDensity,
    values: &[f32],
    warmups: usize,
    runs: usize,
) -> (Vec<f32>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.gaussian_batch(values, 0.0, 1.0).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.gaussian_batch(values, 0.0, 1.0).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_supremum(
    gpu: &GpuTropicalMeasure,
    values: &[f32],
    warmups: usize,
    runs: usize,
) -> (f32, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.supremum(values).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = 0.0;
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.supremum(values).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_infimum(
    gpu: &GpuTropicalMeasure,
    values: &[f32],
    warmups: usize,
    runs: usize,
) -> (f32, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.infimum(values).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = 0.0;
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.infimum(values).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(last);
    }
    (last, total / runs as u32)
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
#[ignore = "Manual benchmark harness for measure/integration crossover work"]
async fn benchmark_measure_cpu_vs_gpu() {
    let warmups = 1;
    let runs = 5;

    println!("\nMeasure benchmark (CPU vs GPU)");
    println!("operation\tsize\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    if let Ok(gpu) = GpuIntegrator::new().await {
        for n in [1_000u32, 10_000, 100_000] {
            let (cpu, cpu_avg) =
                avg_duration(warmups, runs, || cpu_integrate_uniform(0.0, 2.0, n, 1));
            let (gpu_value, gpu_avg) = avg_gpu_integrate(&gpu, n, warmups, runs).await;
            assert!((gpu_value - cpu).abs() < 1e-2);
            print_row("integrate_x2", n as usize, cpu_avg, gpu_avg, true);
        }
    }

    if let Ok(gpu) = GpuParametricDensity::new().await {
        for count in [256usize, 4096, 65536] {
            let values = make_values(count);
            let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_gaussian(&values, 0.0, 1.0));
            let (gpu_values, gpu_avg) = avg_gpu_gaussian(&gpu, &values, warmups, runs).await;
            assert_close(&gpu_values, &cpu, 1e-5);
            print_row("gaussian_density", count, cpu_avg, gpu_avg, true);
        }
    }

    if let Ok(gpu) = GpuTropicalMeasure::new().await {
        for count in [256usize, 4096, 65536] {
            let values = make_values(count);
            let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_supremum(&values));
            let (gpu_value, gpu_avg) = avg_gpu_supremum(&gpu, &values, warmups, runs).await;
            assert!((gpu_value - cpu).abs() < 1e-6);
            print_row("tropical_supremum", count, cpu_avg, gpu_avg, true);

            let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_infimum(&values));
            let (gpu_value, gpu_avg) = avg_gpu_infimum(&gpu, &values, warmups, runs).await;
            assert!((gpu_value - cpu).abs() < 1e-6);
            print_row("tropical_infimum", count, cpu_avg, gpu_avg, true);
        }
    }

    if let Ok(gpu) = GpuMultidimIntegrator::new().await {
        let start = Instant::now();
        let volume = gpu
            .monte_carlo_nd(&[(0.0, 2.0), (-1.0, 1.0), (2.0, 5.0)], 10_000, 7)
            .await
            .unwrap();
        assert!((volume - 12.0).abs() < 1e-6);
        print_row(
            "monte_carlo_nd_constant",
            10_000,
            Duration::ZERO,
            start.elapsed(),
            true,
        );
    }
}
