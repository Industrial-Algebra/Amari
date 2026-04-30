#![cfg(feature = "functional")]

//! Manual benchmark/crossover harnesses for functional-analysis GPU paths.

use amari_core::Multivector;
use amari_functional::{LinearOperator, MatrixOperator};
use amari_gpu::{GpuHilbertSpace, GpuMatrixOperator};
use std::time::{Duration, Instant};

fn make_matrix() -> MatrixOperator<2, 0, 0> {
    let mut values = vec![0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            values[row * 4 + col] = if row == col {
                1.0 + row as f64
            } else {
                ((row + col) as f64) * 0.01
            };
        }
    }
    MatrixOperator::<2, 0, 0>::new(values, 4, 4).unwrap()
}

fn make_vectors(batch_size: usize) -> Vec<Multivector<2, 0, 0>> {
    (0..batch_size)
        .map(|i| {
            Multivector::<2, 0, 0>::from_coefficients(
                (0..4)
                    .map(|j| (((i * 17 + j * 31) % 97) as f64 - 48.0) / 97.0)
                    .collect(),
            )
        })
        .collect()
}

fn cpu_apply_batch(
    matrix: &MatrixOperator<2, 0, 0>,
    vectors: &[Multivector<2, 0, 0>],
) -> Vec<Multivector<2, 0, 0>> {
    vectors.iter().map(|v| matrix.apply(v).unwrap()).collect()
}

fn cpu_inner_batch(left: &[Multivector<2, 0, 0>], right: &[Multivector<2, 0, 0>]) -> Vec<f64> {
    left.iter()
        .zip(right.iter())
        .map(|(a, b)| a.to_vec().iter().zip(b.to_vec()).map(|(x, y)| x * y).sum())
        .collect()
}

fn assert_mv_batch_close(
    actual: &[Multivector<2, 0, 0>],
    expected: &[Multivector<2, 0, 0>],
    tolerance: f64,
) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        for (component, (a, e)) in actual.to_vec().iter().zip(expected.to_vec()).enumerate() {
            assert!(
                (a - e).abs() <= tolerance,
                "mismatch at vector {index}, component {component}"
            );
        }
    }
}

fn assert_close(actual: &[f64], expected: &[f64], tolerance: f64) {
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

async fn avg_gpu_apply(
    gpu: &GpuMatrixOperator<2, 0, 0>,
    vectors: &[Multivector<2, 0, 0>],
    warmups: usize,
    runs: usize,
) -> (Vec<Multivector<2, 0, 0>>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.apply_batch(vectors).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.apply_batch(vectors).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_inner(
    hilbert: &GpuHilbertSpace<2, 0, 0>,
    vectors: &[Multivector<2, 0, 0>],
    warmups: usize,
    runs: usize,
) -> (Vec<f64>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(hilbert.inner_product_batch(vectors, vectors).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = hilbert.inner_product_batch(vectors, vectors).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

fn print_row(label: &str, batch_size: usize, cpu: Duration, gpu: Duration) {
    let cpu_ms = cpu.as_secs_f64() * 1000.0;
    let gpu_ms = gpu.as_secs_f64() * 1000.0;
    let speedup = if gpu_ms > 0.0 {
        cpu_ms / gpu_ms
    } else {
        f64::INFINITY
    };
    println!(
        "{}\t{}\t{:.3}\t{:.3}\t{:.2}x\ttrue",
        label, batch_size, cpu_ms, gpu_ms, speedup
    );
}

#[tokio::test]
#[ignore = "Manual benchmark harness for functional matrix/Hilbert crossover work"]
async fn benchmark_functional_cpu_vs_gpu() {
    let matrix = make_matrix();
    let gpu_matrix = match GpuMatrixOperator::from_matrix_operator(&matrix).await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping functional matrix benchmark harness: {err}");
            return;
        }
    };
    let hilbert = match GpuHilbertSpace::<2, 0, 0>::new().await {
        Ok(hilbert) => hilbert,
        Err(err) => {
            eprintln!("Skipping functional Hilbert benchmark harness: {err}");
            return;
        }
    };

    let cases = [16usize, 64, 256, 1024, 4096];
    let warmups = 1;
    let runs = 5;

    println!("\nFunctional benchmark (CPU vs GPU)");
    println!("operation\tbatch_size\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for batch_size in cases {
        let vectors = make_vectors(batch_size);

        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_apply_batch(&matrix, &vectors));
        let (gpu, gpu_avg) = avg_gpu_apply(&gpu_matrix, &vectors, warmups, runs).await;
        assert_mv_batch_close(&gpu, &cpu, 1e-5);
        print_row("matrix_apply", batch_size, cpu_avg, gpu_avg);

        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_inner_batch(&vectors, &vectors));
        let (gpu, gpu_avg) = avg_gpu_inner(&hilbert, &vectors, warmups, runs).await;
        assert_close(&gpu, &cpu, 1e-5);
        print_row("hilbert_inner", batch_size, cpu_avg, gpu_avg);
    }
}
