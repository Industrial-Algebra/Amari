#![cfg(feature = "topology")]

//! Manual benchmark/crossover harnesses for topology GPU paths.

use amari_gpu::GpuTopology;
use amari_topology::CriticalType;
use std::time::{Duration, Instant};

fn make_points(count: usize) -> Vec<(f64, f64, f64)> {
    (0..count)
        .map(|i| {
            let t = i as f64;
            ((t * 0.31).sin(), (t * 0.17).cos(), (i % 29) as f64 * 0.1)
        })
        .collect()
}

fn cpu_distance_matrix(points: &[(f64, f64, f64)]) -> Vec<f64> {
    let mut out = Vec::with_capacity(points.len() * points.len());
    for &(ax, ay, az) in points {
        for &(bx, by, bz) in points {
            let dx = ax - bx;
            let dy = ay - by;
            let dz = az - bz;
            out.push((dx * dx + dy * dy + dz * dz).sqrt());
        }
    }
    out
}

fn make_peak_grid(width: usize, height: usize) -> Vec<f64> {
    let cx = (width / 2) as f64;
    let cy = (height / 2) as f64;
    let mut values = vec![0.0; width * height];
    for y in 0..height {
        for x in 0..width {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            values[y * width + x] = -(dx * dx + dy * dy);
        }
    }
    values
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
    gpu: &GpuTopology,
    points: &[(f64, f64, f64)],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.compute_distance_matrix(points).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.compute_distance_matrix(points).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_critical(
    gpu: &GpuTopology,
    values: &[f64],
    width: usize,
    height: usize,
    warmups: usize,
    runs: usize,
) -> (usize, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(
            gpu.find_critical_points_2d(values, width, height)
                .await
                .unwrap(),
        );
    }
    let mut total = Duration::ZERO;
    let mut last_len = 0;
    for _ in 0..runs {
        let start = Instant::now();
        let critical = gpu
            .find_critical_points_2d(values, width, height)
            .await
            .unwrap();
        total += start.elapsed();
        last_len = critical.len();
        assert!(critical.iter().any(|cp| {
            cp.position == (width / 2, height / 2)
                && matches!(cp.critical_type, CriticalType::Maximum)
        }));
        std::hint::black_box(&critical);
    }
    (last_len, total / runs as u32)
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
#[ignore = "Manual benchmark harness for topology distance/Morse crossover work"]
async fn benchmark_topology_cpu_vs_gpu() {
    let gpu = match GpuTopology::new().await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping topology benchmark harness: {err}");
            return;
        }
    };
    let warmups = 1;
    let runs = 5;

    println!("\nTopology benchmark (CPU vs GPU)");
    println!("operation\tsize\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for count in [16usize, 64, 256, 512] {
        let points = make_points(count);
        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_distance_matrix(&points));
        let (gpu_distances, gpu_avg) = avg_gpu_distances(&gpu, &points, warmups, runs).await;
        assert_close(&gpu_distances, &cpu, 1e-5);
        print_row("distance_matrix", count, cpu_avg, gpu_avg, true);
    }

    for width in [16usize, 64, 128] {
        let height = width;
        let values = make_peak_grid(width, height);
        let (_, gpu_avg) = avg_gpu_critical(&gpu, &values, width, height, warmups, runs).await;
        print_row(
            "critical_points_2d",
            width * height,
            Duration::ZERO,
            gpu_avg,
            true,
        );
    }
}
