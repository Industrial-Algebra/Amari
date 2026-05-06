#![cfg(feature = "gf2")]

//! Manual benchmark/crossover harnesses for GF(2) GPU kernels.

use amari_core::gf2::{GF2Matrix, GF2Vector, GF2};
use amari_gpu::{GF2GpuOps, GpuGF2CliffordPair, GpuGF2HammingPair, GpuGF2MatVecData};
use std::time::{Duration, Instant};

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

fn make_clifford_pairs(batch_size: usize) -> Vec<GpuGF2CliffordPair> {
    (0..batch_size)
        .map(|i| {
            let a_index = i % 8;
            let b_index = (i * 3 + 1) % 8;
            let mut a_bits = vec![0u8; 8];
            let mut b_bits = vec![0u8; 8];
            a_bits[a_index] = 1;
            b_bits[b_index] = 1;
            GpuGF2CliffordPair::from_bits(&a_bits, &b_bits, 3, 0)
        })
        .collect()
}

fn cpu_clifford_one_hot_expected(pairs: &[GpuGF2CliffordPair]) -> Vec<[u32; 4]> {
    pairs
        .iter()
        .map(|pair| {
            let a = pair.a_words[0].trailing_zeros();
            let b = pair.b_words[0].trailing_zeros();
            let blade = a ^ b;
            let mut out = [0u32; 4];
            out[(blade / 32) as usize] = 1 << (blade % 32);
            out
        })
        .collect()
}

fn make_matvec_batch(batch_size: usize) -> Vec<GpuGF2MatVecData> {
    (0..batch_size)
        .map(|i| {
            let mut matrix = GF2Matrix::zero(16, 16);
            for row in 0..16 {
                matrix.set(row, row, GF2::ONE);
                if (row + i) % 3 == 0 {
                    matrix.set(row, (row + 1) % 16, GF2::ONE);
                }
            }
            let bits: Vec<u8> = (0..16).map(|bit| ((i + bit) & 1) as u8).collect();
            let vector = GF2Vector::from_bits(&bits);
            GpuGF2MatVecData::from_matrix_and_vector(&matrix, &vector)
        })
        .collect()
}

fn cpu_matvec(data: &[GpuGF2MatVecData]) -> Vec<u32> {
    data.iter()
        .map(|entry| {
            let mut result = 0u32;
            for row in 0..entry.nrows {
                let parity =
                    ((entry.matrix_rows[row as usize] & entry.vector).count_ones() & 1) != 0;
                if parity {
                    result |= 1 << row;
                }
            }
            result
        })
        .collect()
}

fn make_hamming_batch(batch_size: usize) -> Vec<GpuGF2HammingPair> {
    (0..batch_size)
        .map(|i| GpuGF2HammingPair {
            a_words: [0xAAAA_AAAAu32 ^ i as u32, 0x0123_4567u32, 0, 0],
            b_words: [
                0x5555_5555u32.rotate_left((i % 31) as u32),
                0x89AB_CDEFu32,
                0,
                0,
            ],
            dim: 64,
            padding: [0; 3],
        })
        .collect()
}

fn cpu_hamming(data: &[GpuGF2HammingPair]) -> Vec<u32> {
    data.iter()
        .map(|entry| {
            let words = entry.dim.div_ceil(32) as usize;
            (0..words)
                .map(|word| {
                    let mut xor = entry.a_words[word] ^ entry.b_words[word];
                    if word == words - 1 && entry.dim % 32 != 0 {
                        xor &= (1u32 << (entry.dim % 32)) - 1;
                    }
                    xor.count_ones()
                })
                .sum()
        })
        .collect()
}

async fn avg_gpu_clifford(
    gpu: &mut GF2GpuOps,
    data: &[GpuGF2CliffordPair],
    warmups: usize,
    runs: usize,
) -> (Vec<[u32; 4]>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_gf2_geometric_product(data).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_gf2_geometric_product(data).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_matvec(
    gpu: &mut GF2GpuOps,
    data: &[GpuGF2MatVecData],
    warmups: usize,
    runs: usize,
) -> (Vec<u32>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_gf2_matvec(data).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_gf2_matvec(data).await.unwrap();
        total += start.elapsed();
        std::hint::black_box(&last);
    }
    (last, total / runs as u32)
}

async fn avg_gpu_hamming(
    gpu: &mut GF2GpuOps,
    data: &[GpuGF2HammingPair],
    warmups: usize,
    runs: usize,
) -> (Vec<u32>, Duration) {
    for _ in 0..warmups {
        std::hint::black_box(gpu.batch_gf2_hamming_distance(data).await.unwrap());
    }
    let mut total = Duration::ZERO;
    let mut last = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        last = gpu.batch_gf2_hamming_distance(data).await.unwrap();
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
#[ignore = "Manual benchmark harness for GF(2) kernel crossover work"]
async fn benchmark_gf2_kernels_cpu_vs_gpu() {
    let mut gpu = match GF2GpuOps::new().await {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("Skipping GF(2) benchmark harness: {err}");
            return;
        }
    };

    let cases = [16usize, 64, 256, 1024, 4096];
    let warmups = 1;
    let runs = 5;

    println!("\nGF(2) kernel benchmark (CPU vs GPU)");
    println!("operation\tbatch_size\tcpu_avg_ms\tgpu_avg_ms\tspeedup\tcorrect");

    for batch_size in cases {
        let data = make_clifford_pairs(batch_size);
        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_clifford_one_hot_expected(&data));
        let (gpu_out, gpu_avg) = avg_gpu_clifford(&mut gpu, &data, warmups, runs).await;
        assert_eq!(gpu_out, cpu);
        print_row("clifford_one_hot", batch_size, cpu_avg, gpu_avg);

        let data = make_matvec_batch(batch_size);
        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_matvec(&data));
        let (gpu_out, gpu_avg) = avg_gpu_matvec(&mut gpu, &data, warmups, runs).await;
        assert_eq!(gpu_out, cpu);
        print_row("matvec_16x16", batch_size, cpu_avg, gpu_avg);

        let data = make_hamming_batch(batch_size);
        let (cpu, cpu_avg) = avg_duration(warmups, runs, || cpu_hamming(&data));
        let (gpu_out, gpu_avg) = avg_gpu_hamming(&mut gpu, &data, warmups, runs).await;
        assert_eq!(gpu_out, cpu);
        print_row("hamming_64", batch_size, cpu_avg, gpu_avg);
    }
}
