#![cfg(feature = "tropical")]

//! Manual benchmark/crossover harness for tropical attention scores.
//!
//! Ignored by default because it is hardware diagnostic output, not a CI gate.

use amari_gpu::TropicalGpuOps;
use amari_tropical::{TropicalMatrix, TropicalNumber};
use std::time::{Duration, Instant};

fn make_logits(rows: usize, cols: usize) -> TropicalMatrix<f32> {
    let mut logits = TropicalMatrix::new(rows, cols);
    for i in 0..rows {
        for j in 0..cols {
            let value = (((i * 31 + j * 17) % 101) as f32 - 50.0) / 11.0;
            logits.data[i][j] = TropicalNumber::new(value);
        }
    }
    logits
}

fn cpu_attention_scores(logits: &TropicalMatrix<f32>) -> TropicalMatrix<f32> {
    let mut scores = TropicalMatrix::new(logits.rows, logits.cols);
    for i in 0..logits.rows {
        let mut row_max = f32::NEG_INFINITY;
        for j in 0..logits.cols {
            row_max = row_max.max(logits.data[i][j].value());
        }
        for j in 0..logits.cols {
            scores.data[i][j] = TropicalNumber::new(if logits.data[i][j].value() == row_max {
                1.0
            } else {
                0.0
            });
        }
    }
    scores
}

fn assert_matrix_close(a: &TropicalMatrix<f32>, b: &TropicalMatrix<f32>, tol: f32) {
    assert_eq!(a.rows, b.rows);
    assert_eq!(a.cols, b.cols);
    for i in 0..a.rows {
        for j in 0..a.cols {
            assert!(
                (a.data[i][j].value() - b.data[i][j].value()).abs() <= tol,
                "mismatch at ({i}, {j})"
            );
        }
    }
}

fn avg_cpu(
    logits: &TropicalMatrix<f32>,
    warmups: usize,
    runs: usize,
) -> (TropicalMatrix<f32>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(cpu_attention_scores(logits));
    }
    let mut total = Duration::ZERO;
    let mut last = TropicalMatrix::new(0, 0);
    for _ in 0..runs {
        let start = Instant::now();
        last = cpu_attention_scores(logits);
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu(
    gpu: &mut TropicalGpuOps,
    logits: &TropicalMatrix<f32>,
    warmups: usize,
    runs: usize,
) -> (TropicalMatrix<f32>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.attention_scores(logits).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = TropicalMatrix::new(0, 0);
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.attention_scores(logits).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

#[tokio::test]
#[ignore = "Manual benchmark harness for tropical attention-score crossover work"]
async fn benchmark_tropical_attention_scores_cpu_vs_gpu() {
    let mut gpu = match TropicalGpuOps::new().await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping tropical attention benchmark harness: {err}");
            return;
        }
    };

    let cases = [(16usize, 64usize), (64, 64), (128, 128), (256, 256)];
    let warmups = 1;
    let runs = 5;

    println!("\nTropical attention-score benchmark (CPU vs GPU)");
    println!("rowsxcols\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for (rows, cols) in cases {
        let logits = make_logits(rows, cols);
        let (cpu_result, cpu_avg) = avg_cpu(&logits, warmups, runs);
        let (gpu_result, gpu_avg) = avg_gpu(&mut gpu, &logits, warmups, runs).await;
        assert_matrix_close(&gpu_result, &cpu_result, 1e-6);

        let cpu_ms = cpu_avg.as_secs_f64() * 1000.0;
        let gpu_ms = gpu_avg.as_secs_f64() * 1000.0;
        let speedup = if gpu_ms > 0.0 {
            cpu_ms / gpu_ms
        } else {
            f64::INFINITY
        };
        println!(
            "{}x{}\t{:.3}\t{:.3}\t{:.2}x\ttrue",
            rows, cols, cpu_ms, gpu_ms, speedup
        );
    }
}
