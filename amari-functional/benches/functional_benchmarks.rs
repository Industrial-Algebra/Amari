//! Performance benchmarks for amari-functional analysis operations
//!
//! Measures critical operations for Hilbert spaces, operators, and spectral theory.

use amari_core::Multivector;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

type Cl3 = Multivector<3, 0, 0>;

/// Benchmark inner product computation
fn bench_inner_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("inner_product");

    for size in [4, 8, 16, 32].iter() {
        let v1: Vec<f64> = (0..*size).map(|i| i as f64 * 0.1).collect();
        let v2: Vec<f64> = (0..*size).map(|i| (size - i) as f64 * 0.1).collect();

        group.bench_with_input(BenchmarkId::new("compute", size), size, |b, _| {
            b.iter(|| {
                let result: f64 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
                black_box(result)
            })
        });
    }

    group.finish();
}

/// Benchmark multivector inner product
fn bench_multivector_inner_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("multivector_inner_product");

    let mv1 = Cl3::basis_vector(0) + Cl3::basis_vector(1) * 2.0 + Cl3::basis_vector(2) * 3.0;
    let mv2 = Cl3::basis_vector(0) * 0.5 + Cl3::basis_vector(1) * 1.5 + Cl3::basis_vector(2) * 2.5;

    group.bench_function("inner_product", |b| {
        b.iter(|| black_box(black_box(&mv1).inner_product(black_box(&mv2))))
    });

    group.bench_function("geometric_product", |b| {
        b.iter(|| black_box(black_box(&mv1).geometric_product(black_box(&mv2))))
    });

    group.finish();
}

/// Benchmark norm computation
fn bench_norms(c: &mut Criterion) {
    let mut group = c.benchmark_group("norms");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let v: Vec<f64> = (0..*size).map(|i| i as f64 * 0.01).collect();

        group.bench_with_input(BenchmarkId::new("l2_norm", size), size, |b, _| {
            b.iter(|| {
                let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
                black_box(norm)
            })
        });

        group.bench_with_input(BenchmarkId::new("l1_norm", size), size, |b, _| {
            b.iter(|| {
                let norm: f64 = v.iter().map(|x| x.abs()).sum();
                black_box(norm)
            })
        });

        group.bench_with_input(BenchmarkId::new("linf_norm", size), size, |b, _| {
            b.iter(|| {
                let norm: f64 = v.iter().map(|x| x.abs()).fold(0.0f64, |a, b| a.max(b));
                black_box(norm)
            })
        });
    }

    group.finish();
}

/// Benchmark multivector magnitude
fn bench_multivector_magnitude(c: &mut Criterion) {
    let mut group = c.benchmark_group("multivector_magnitude");

    let mv = Cl3::scalar(1.0)
        + Cl3::basis_vector(0) * 2.0
        + Cl3::basis_vector(1) * 3.0
        + Cl3::basis_vector(2) * 4.0;

    group.bench_function("magnitude", |b| {
        b.iter(|| black_box(black_box(&mv).coefficient_norm()))
    });

    group.bench_function("norm_squared", |b| {
        b.iter(|| black_box(black_box(&mv).norm_squared()))
    });

    group.finish();
}

/// Benchmark matrix-vector-like operations
fn bench_matrix_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_operations");

    for size in [4, 8, 16, 32].iter() {
        // Simulate matrix-vector product
        let matrix: Vec<Vec<f64>> = (0..*size)
            .map(|i| (0..*size).map(|j| (i * size + j) as f64 * 0.01).collect())
            .collect();
        let vector: Vec<f64> = (0..*size).map(|i| i as f64).collect();

        group.bench_with_input(BenchmarkId::new("matvec", size), size, |b, _| {
            b.iter(|| {
                let result: Vec<f64> = matrix
                    .iter()
                    .map(|row| row.iter().zip(vector.iter()).map(|(a, b)| a * b).sum())
                    .collect();
                black_box(result)
            })
        });
    }

    group.finish();
}

/// Benchmark projection operations
fn bench_projections(c: &mut Criterion) {
    let mut group = c.benchmark_group("projections");

    let mv = Cl3::scalar(1.0)
        + Cl3::basis_vector(0) * 2.0
        + Cl3::basis_vector(1) * 3.0
        + Cl3::basis_vector(2) * 4.0
        + Cl3::basis_vector(0).outer_product(&Cl3::basis_vector(1)) * 5.0;

    group.bench_function("grade_projection_0", |b| {
        b.iter(|| black_box(black_box(&mv).grade_projection(0)))
    });

    group.bench_function("grade_projection_1", |b| {
        b.iter(|| black_box(black_box(&mv).grade_projection(1)))
    });

    group.bench_function("grade_projection_2", |b| {
        b.iter(|| black_box(black_box(&mv).grade_projection(2)))
    });

    group.finish();
}

criterion_group!(
    functional_benchmarks,
    bench_inner_product,
    bench_multivector_inner_product,
    bench_norms,
    bench_multivector_magnitude,
    bench_matrix_operations,
    bench_projections,
);

criterion_main!(functional_benchmarks);
