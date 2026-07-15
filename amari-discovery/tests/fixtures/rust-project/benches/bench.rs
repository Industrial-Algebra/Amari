// SPDX-License-Identifier: MIT OR Apache-2.0

//! Benchmarks for tropical algebra and geometric algebra operations.
//!
//! Uses criterion for FFI/Native/BLAS benchmarking.

use amari::tropical::TropicalNumber;
use criterion::{black_box, Criterion};

pub fn bench_tropical_add(c: &mut Criterion) {
    c.bench_function("tropical_add", |b| {
        let a = TropicalNumber::<f64>::new(3.0);
        let b = TropicalNumber::<f64>::new(2.0);
        b.iter(|| {
            black_box(a.tropical_add(&b));
        });
    });
}
