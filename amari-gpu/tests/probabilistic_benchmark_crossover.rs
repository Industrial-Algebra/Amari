#![cfg(feature = "probabilistic")]

//! Manual benchmark/crossover harnesses for probabilistic GPU kernels.

use amari_gpu::GpuProbabilistic;
use std::time::{Duration, Instant};

fn make_samples(sample_count: usize, dimension: usize) -> Vec<f64> {
    let mut samples = Vec::with_capacity(sample_count * dimension);
    for sample in 0..sample_count {
        for dim in 0..dimension {
            samples.push(((sample * 17 + dim * 31) % 997) as f64 / 997.0);
        }
    }
    samples
}

fn cpu_mean(samples: &[f64], dimension: usize) -> Vec<f64> {
    let sample_count = samples.len() / dimension;
    let mut mean = vec![0.0; dimension];
    for sample in samples.chunks_exact(dimension) {
        for (dim, value) in sample.iter().enumerate() {
            mean[dim] += value;
        }
    }
    for value in &mut mean {
        *value /= sample_count as f64;
    }
    mean
}

fn cpu_variance(samples: &[f64], mean: &[f64], dimension: usize) -> Vec<f64> {
    let sample_count = samples.len() / dimension;
    let mut variance = vec![0.0; dimension];
    for sample in samples.chunks_exact(dimension) {
        for dim in 0..dimension {
            let d = sample[dim] - mean[dim];
            variance[dim] += d * d;
        }
    }
    for value in &mut variance {
        *value /= (sample_count - 1) as f64;
    }
    variance
}

fn cpu_deterministic_gaussian(sample_count: usize, mean: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(sample_count * mean.len());
    for _ in 0..sample_count {
        out.extend_from_slice(mean);
    }
    out
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

async fn avg_gpu_mean(
    gpu: &GpuProbabilistic,
    samples: &[f64],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_mean(samples).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_mean(samples).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_variance(
    gpu: &GpuProbabilistic,
    samples: &[f64],
    mean: &[f64],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_variance(samples, mean).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_variance(samples, mean).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_sample(
    gpu: &GpuProbabilistic,
    sample_count: usize,
    mean: &[f64],
    std_dev: &[f64],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(
            gpu.batch_sample_gaussian(sample_count, mean, std_dev)
                .await
                .unwrap(),
        );
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu
            .batch_sample_gaussian(sample_count, mean, std_dev)
            .await
            .unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

fn print_row(label: &str, samples: usize, dimension: usize, cpu: Duration, gpu: Duration) {
    let cpu_ms = cpu.as_secs_f64() * 1000.0;
    let gpu_ms = gpu.as_secs_f64() * 1000.0;
    let speedup = if gpu_ms > 0.0 {
        cpu_ms / gpu_ms
    } else {
        f64::INFINITY
    };
    println!(
        "{}\t{}\t{}\t{:.3}\t{:.3}\t{:.2}x\ttrue",
        label, samples, dimension, cpu_ms, gpu_ms, speedup
    );
}

#[tokio::test]
#[ignore = "Manual benchmark harness for probabilistic sampling/statistics crossover work"]
async fn benchmark_probabilistic_cpu_vs_gpu() {
    let dimension = 8;
    let gpu = match GpuProbabilistic::new(dimension).await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping probabilistic benchmark harness: {err}");
            return;
        }
    };

    let cases = [16usize, 128, 1024, 8192];
    let warmups = 1;
    let runs = 5;
    let mean = (0..dimension).map(|i| i as f64 * 0.25).collect::<Vec<_>>();
    let zero_std = vec![0.0; dimension];

    println!("\nProbabilistic benchmark (CPU vs GPU)");
    println!("operation\tsamples\tdimension\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for sample_count in cases {
        let samples = make_samples(sample_count, dimension);

        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_mean(&samples, dimension));
        let (gpu_out, gpu_avg) = avg_gpu_mean(&gpu, &samples, warmups, runs).await;
        assert_close(&gpu_out, &cpu, 1e-5);
        print_row("mean", sample_count, dimension, cpu_avg, gpu_avg);

        let (cpu, cpu_avg) = avg_duration(warmups, runs, || {
            cpu_variance(&samples, &gpu_out, dimension)
        });
        let (gpu_var, gpu_avg) = avg_gpu_variance(&gpu, &samples, &gpu_out, warmups, runs).await;
        assert_close(&gpu_var, &cpu, 1e-4);
        print_row("variance", sample_count, dimension, cpu_avg, gpu_avg);

        let (cpu, cpu_avg) = avg_duration(warmups, runs, || {
            cpu_deterministic_gaussian(sample_count, &mean)
        });
        let (gpu_samples, gpu_avg) =
            avg_gpu_sample(&gpu, sample_count, &mean, &zero_std, warmups, runs).await;
        assert_close(&gpu_samples, &cpu, 1e-8);
        print_row(
            "gaussian_zero_std",
            sample_count,
            dimension,
            cpu_avg,
            gpu_avg,
        );
    }
}
