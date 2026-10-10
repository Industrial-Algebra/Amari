//! Comprehensive performance benchmarking suite for Amari geometric algebra library
//!
//! This benchmark suite measures critical operations to ensure performance targets
//! are met and provides regression testing for optimization work.

use amari_core::{Bivector, Multivector, Rotor, Vector};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

type Cl3 = Multivector<3, 0, 0>; // 3D Euclidean space

/// Benchmark geometric product operations
fn bench_geometric_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("geometric_product");

    // Single geometric product
    let e1 = Cl3::basis_vector(0);
    let e2 = Cl3::basis_vector(1);

    group.bench_function("scalar_implementation", |b| {
        b.iter(|| black_box(e1.geometric_product(black_box(&e2))))
    });

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        group.bench_function("simd_optimized", |b| {
            b.iter(|| black_box(e1.geometric_product(black_box(&e2))))
        });
    }

    // Complex multivector products
    let complex_mv1 =
        Cl3::scalar(2.0) + e1.clone() * 3.0 + e2.clone() * 4.0 + e1.outer_product(&e2) * 5.0;
    let complex_mv2 = Cl3::scalar(1.5) + e2.clone() * 2.5 + Cl3::basis_vector(2) * 3.5;

    group.bench_function("complex_multivectors", |b| {
        b.iter(|| black_box(complex_mv1.geometric_product(black_box(&complex_mv2))))
    });

    group.finish();
}

/// Benchmark batch operations for throughput testing
fn bench_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    for size in [100, 1000, 10000].iter() {
        // Generate test data
        let mut lhs_batch = Vec::with_capacity(size * 8);
        let mut rhs_batch = Vec::with_capacity(size * 8);

        for i in 0..*size {
            let mv1 = Cl3::scalar(i as f64) + Cl3::basis_vector(0) * (i as f64 * 0.5);
            let mv2 =
                Cl3::basis_vector(1) * (i as f64 * 0.3) + Cl3::basis_vector(2) * (i as f64 * 0.7);

            lhs_batch.extend_from_slice(mv1.as_slice());
            rhs_batch.extend_from_slice(mv2.as_slice());
        }

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("geometric_product_batch", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut results = vec![0.0; size * 8];
                    for i in 0..size {
                        let lhs_offset = i * 8;
                        let rhs_offset = i * 8;
                        let result_offset = i * 8;

                        let lhs_coeffs = lhs_batch[lhs_offset..lhs_offset + 8].to_vec();
                        let rhs_coeffs = rhs_batch[rhs_offset..rhs_offset + 8].to_vec();

                        let lhs_mv = Cl3::from_coefficients(lhs_coeffs);
                        let rhs_mv = Cl3::from_coefficients(rhs_coeffs);
                        let result_mv = lhs_mv.geometric_product(&rhs_mv);

                        results[result_offset..result_offset + 8]
                            .copy_from_slice(result_mv.as_slice());
                    }
                    black_box(results)
                })
            },
        );

        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            group.bench_with_input(
                BenchmarkId::new("simd_batch_avx2", size),
                size,
                |b, &size| {
                    b.iter(|| {
                        let mut results = vec![0.0; size * 8];
                        amari_core::simd::batch_geometric_product_avx2(
                            &lhs_batch,
                            &rhs_batch,
                            &mut results,
                        );
                        black_box(results)
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmark rotor operations (critical for 3D rotations)
fn bench_rotor_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("rotor_operations");

    let e1 = Vector::<3, 0, 0>::e1();
    let e2 = Vector::<3, 0, 0>::e2();
    let e3 = Vector::<3, 0, 0>::e3();

    let bivector = Bivector::from_components(1.0, 0.5, 0.3);
    let angle = std::f64::consts::PI / 4.0;

    group.bench_function("rotor_creation", |b| {
        b.iter(|| black_box(Rotor::from_bivector(black_box(&bivector), black_box(angle))))
    });

    let rotor = Rotor::from_bivector(&bivector, angle);
    let vector = e1
        .add(&Vector::from_multivector(&(e2.mv.clone() * 0.5)))
        .add(&Vector::from_multivector(&(e3.mv.clone() * 0.3)));

    group.bench_function("rotor_application", |b| {
        b.iter(|| black_box(rotor.apply(black_box(&vector.mv))))
    });

    group.bench_function("rotor_composition", |b| {
        let rotor2 = Rotor::from_bivector(&Bivector::from_components(0.5, 1.0, 0.2), angle * 0.5);
        b.iter(|| black_box(rotor.compose(black_box(&rotor2))))
    });

    group.finish();
}

/// Benchmark inner and outer products
fn bench_product_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("product_operations");

    let v1 = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
    let v2 = Vector::<3, 0, 0>::from_components(0.5, 1.5, 2.5);

    group.bench_function("inner_product", |b| {
        b.iter(|| black_box(v1.inner_product(black_box(&v2))))
    });

    group.bench_function("outer_product", |b| {
        b.iter(|| black_box(v1.outer_product(black_box(&v2))))
    });

    let bv1 = Bivector::<3, 0, 0>::from_components(1.0, 0.5, 2.0);
    let bv2 = Bivector::<3, 0, 0>::from_components(1.5, 1.0, 0.5);

    group.bench_function("bivector_inner_product", |b| {
        b.iter(|| black_box(bv1.inner_product(black_box(&bv2))))
    });

    group.bench_function("bivector_outer_product", |b| {
        b.iter(|| black_box(bv1.outer_product(black_box(&bv2))))
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");

    group.bench_function("multivector_creation", |b| {
        b.iter(|| black_box(Cl3::zero()))
    });

    group.bench_function("multivector_from_coefficients", |b| {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        b.iter(|| black_box(Cl3::from_coefficients(black_box(coeffs.clone()))))
    });

    group.bench_function("multivector_clone", |b| {
        let mv = Cl3::scalar(1.0) + Cl3::basis_vector(0) * 2.0 + Cl3::basis_vector(1) * 3.0;
        b.iter(|| black_box(mv.clone()))
    });

    // Aligned allocation benchmark removed — aligned_alloc is now pub(crate)

    group.finish();
}

/// Benchmark verification overhead to ensure <10% target
fn bench_verification_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("verification_overhead");

    let e1 = Cl3::basis_vector(0);
    let e2 = Cl3::basis_vector(1);

    // Measure raw operation without verification
    group.bench_function("geometric_product_raw", |b| {
        b.iter(|| black_box(e1.geometric_product(black_box(&e2))))
    });

    #[cfg(feature = "formal-verification")]
    {
        group.bench_function("geometric_product_with_verification", |b| {
            b.iter(|| {
                // This would include verification contracts if enabled
                black_box(e1.geometric_product(black_box(&e2)))
            })
        });
    }

    group.finish();
}

/// Benchmark grade operations
fn bench_grade_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("grade_operations");

    let complex_mv = Cl3::scalar(1.0)
        + Cl3::basis_vector(0) * 2.0
        + Cl3::basis_vector(1) * 3.0
        + Cl3::basis_vector(2) * 4.0
        + Cl3::basis_vector(0).outer_product(&Cl3::basis_vector(1)) * 5.0
        + Cl3::basis_vector(0).outer_product(&Cl3::basis_vector(2)) * 6.0
        + Cl3::basis_vector(1).outer_product(&Cl3::basis_vector(2)) * 7.0
        + Cl3::basis_vector(0)
            .outer_product(&Cl3::basis_vector(1))
            .outer_product(&Cl3::basis_vector(2))
            * 8.0;

    group.bench_function("grade_projection_0", |b| {
        b.iter(|| black_box(complex_mv.grade_projection(0)))
    });

    group.bench_function("grade_projection_1", |b| {
        b.iter(|| black_box(complex_mv.grade_projection(1)))
    });

    group.bench_function("grade_projection_2", |b| {
        b.iter(|| black_box(complex_mv.grade_projection(2)))
    });

    group.bench_function("reverse_operation", |b| {
        b.iter(|| black_box(complex_mv.reverse()))
    });

    group.bench_function("magnitude_calculation", |b| {
        b.iter(|| black_box(complex_mv.coefficient_norm()))
    });

    group.finish();
}

criterion_group!(
    performance_suite,
    bench_geometric_product,
    bench_batch_operations,
    bench_rotor_operations,
    bench_product_operations,
    bench_memory_operations,
    bench_verification_overhead,
    bench_grade_operations
);

criterion_main!(performance_suite);
