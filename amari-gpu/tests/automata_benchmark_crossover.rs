#![cfg(feature = "automata")]

//! Manual benchmark/crossover harnesses for automata GPU paths.

use amari_gpu::{AutomataGpuOps, GpuCellData, GpuEvolutionParams, GpuRuleConfig};
use std::time::{Duration, Instant};

fn make_cells(count: usize) -> Vec<GpuCellData> {
    (0..count)
        .map(|i| GpuCellData {
            scalar: ((i % 97) as f32 - 48.0) / 97.0,
            e1: ((i * 3 % 89) as f32 - 44.0) / 89.0,
            e2: ((i * 5 % 83) as f32 - 41.0) / 83.0,
            e3: ((i * 7 % 79) as f32 - 39.0) / 79.0,
            generation: 7.0,
            ..GpuCellData::default()
        })
        .collect()
}

fn cpu_apply_rules(cells: &[GpuCellData], rule: &GpuRuleConfig) -> Vec<GpuCellData> {
    cells
        .iter()
        .map(|cell| {
            let mut out = *cell;
            let damping = 1.0 - rule.damping_factor;
            out.scalar *= damping;
            out.e1 *= damping;
            out.e2 *= damping;
            out.e3 *= damping;
            out.e12 *= damping;
            out.e13 *= damping;
            out.e23 *= damping;
            out.e123 *= damping;
            if out.scalar.abs() < rule.threshold {
                out.scalar = 0.0;
            }
            out
        })
        .collect()
}

fn cpu_energy(cells: &[GpuCellData]) -> f32 {
    cells
        .iter()
        .map(|cell| {
            cell.scalar * cell.scalar
                + cell.e1 * cell.e1
                + cell.e2 * cell.e2
                + cell.e3 * cell.e3
                + cell.e12 * cell.e12
                + cell.e13 * cell.e13
                + cell.e23 * cell.e23
                + cell.e123 * cell.e123
        })
        .sum()
}

fn assert_cells_close(actual: &[GpuCellData], expected: &[GpuCellData], tolerance: f32) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        assert!(
            (actual.scalar - expected.scalar).abs() <= tolerance,
            "scalar mismatch at {index}"
        );
        assert!(
            (actual.e1 - expected.e1).abs() <= tolerance,
            "e1 mismatch at {index}"
        );
        assert!(
            (actual.e2 - expected.e2).abs() <= tolerance,
            "e2 mismatch at {index}"
        );
        assert!(
            (actual.e3 - expected.e3).abs() <= tolerance,
            "e3 mismatch at {index}"
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
    ops: &mut AutomataGpuOps,
    cells: &[GpuCellData],
    rule: &GpuRuleConfig,
    warmups: usize,
    runs: usize,
) -> (Vec<GpuCellData>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(ops.batch_apply_rules(cells, &[*rule]).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = ops.batch_apply_rules(cells, &[*rule]).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_energy(
    ops: &mut AutomataGpuOps,
    cells: &[GpuCellData],
    warmups: usize,
    runs: usize,
) -> (f32, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(ops.calculate_total_energy(cells).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = 0.0;
    for _ in 0..runs {
        let start = Instant::now();
        last = ops.calculate_total_energy(cells).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_evolve(
    ops: &mut AutomataGpuOps,
    cells: &[GpuCellData],
    rule: &GpuRuleConfig,
    params: &GpuEvolutionParams,
    warmups: usize,
    runs: usize,
) -> (Vec<GpuCellData>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(ops.batch_evolve_ca(cells, &[*rule], params).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = ops.batch_evolve_ca(cells, &[*rule], params).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

fn print_row(label: &str, cells: usize, cpu: Duration, gpu: Duration, correct: bool) {
    let cpu_ms = cpu.as_secs_f64() * 1000.0;
    let gpu_ms = gpu.as_secs_f64() * 1000.0;
    let speedup = if gpu_ms > 0.0 {
        cpu_ms / gpu_ms
    } else {
        f64::INFINITY
    };
    println!(
        "{}\t{}\t{:.3}\t{:.3}\t{:.2}x\t{}",
        label, cells, cpu_ms, gpu_ms, speedup, correct
    );
}

#[tokio::test]
#[ignore = "Manual benchmark harness for automata rule/energy/evolution crossover work"]
async fn benchmark_automata_cpu_vs_gpu() {
    let mut ops = match AutomataGpuOps::new().await {
        Ok(ops) => ops,
        Err(err) => {
            eprintln!("Skipping automata benchmark harness: {err}");
            return;
        }
    };
    let rule = GpuRuleConfig {
        damping_factor: 0.1,
        threshold: 0.25,
        ..GpuRuleConfig::default()
    };
    let params = GpuEvolutionParams {
        steps_per_batch: 1.0,
        current_generation: 7.0,
        ..GpuEvolutionParams::default()
    };

    let cases = [256usize, 4096, 16384];
    let warmups = 1;
    let runs = 5;

    println!("\nAutomata benchmark (CPU vs GPU)");
    println!("operation\tcells\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for count in cases {
        let cells = make_cells(count);
        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_apply_rules(&cells, &rule));
        let (gpu, gpu_avg) = avg_gpu_apply(&mut ops, &cells, &rule, warmups, runs).await;
        assert_cells_close(&gpu, &cpu, 1e-5);
        print_row("apply_rules", count, cpu_avg, gpu_avg, true);

        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_energy(&cells));
        let (gpu, gpu_avg) = avg_gpu_energy(&mut ops, &cells, warmups, runs).await;
        assert!((gpu - cpu).abs() <= (1e-2 * count as f32).max(1e-2));
        print_row("energy", count, cpu_avg, gpu_avg, true);

        let (evolved, gpu_avg) =
            avg_gpu_evolve(&mut ops, &cells, &rule, &params, warmups, runs).await;
        assert_eq!(evolved.len(), cells.len());
        print_row("evolve_ca", count, Duration::ZERO, gpu_avg, true);
    }
}
