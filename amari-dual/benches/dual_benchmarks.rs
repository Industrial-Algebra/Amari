//! Performance benchmarks for amari-dual automatic differentiation
//!
//! Measures critical operations for dual number and automatic differentiation.

use amari_dual::{DualNumber, MultiDualNumber, StaticMultiDual};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

/// Benchmark basic dual number arithmetic
fn bench_dual_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_arithmetic");

    let x = DualNumber::variable(2.0);
    let y = DualNumber::variable(3.0);

    group.bench_function("add", |b| b.iter(|| black_box(black_box(x) + black_box(y))));

    group.bench_function("sub", |b| b.iter(|| black_box(black_box(x) - black_box(y))));

    group.bench_function("mul", |b| b.iter(|| black_box(black_box(x) * black_box(y))));

    group.bench_function("div", |b| b.iter(|| black_box(black_box(x) / black_box(y))));

    group.finish();
}

/// Benchmark dual number creation
fn bench_dual_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_creation");

    group.bench_function("constant", |b| {
        b.iter(|| black_box(DualNumber::constant(black_box(5.0))))
    });

    group.bench_function("variable", |b| {
        b.iter(|| black_box(DualNumber::variable(black_box(5.0))))
    });

    group.bench_function("from_parts", |b| {
        b.iter(|| black_box(DualNumber::new(black_box(5.0), black_box(1.0))))
    });

    group.finish();
}

/// Benchmark transcendental functions on dual numbers
fn bench_dual_transcendental(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_transcendental");

    let x = DualNumber::variable(1.5);

    group.bench_function("exp", |b| b.iter(|| black_box(black_box(x).exp())));

    group.bench_function("ln", |b| b.iter(|| black_box(black_box(x).ln())));

    group.bench_function("sin", |b| b.iter(|| black_box(black_box(x).sin())));

    group.bench_function("cos", |b| b.iter(|| black_box(black_box(x).cos())));

    group.bench_function("sqrt", |b| b.iter(|| black_box(black_box(x).sqrt())));

    group.bench_function("pow", |b| b.iter(|| black_box(black_box(x).powf(2.5))));

    group.bench_function("tanh", |b| b.iter(|| black_box(black_box(x).tanh())));

    group.bench_function("sigmoid", |b| b.iter(|| black_box(black_box(x).sigmoid())));

    group.finish();
}

/// Benchmark multi-variable dual numbers
fn bench_multi_dual(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_dual");

    // Test with different numbers of variables
    for num_vars in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("variable_creation", num_vars),
            num_vars,
            |b, &num_vars| {
                b.iter(|| {
                    black_box(MultiDualNumber::variable(
                        black_box(1.0),
                        0,
                        black_box(num_vars),
                    ))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("constant_creation", num_vars),
            num_vars,
            |b, &num_vars| {
                b.iter(|| {
                    black_box(MultiDualNumber::constant(
                        black_box(1.0),
                        black_box(num_vars),
                    ))
                })
            },
        );
    }

    let x = MultiDualNumber::variable(2.0, 0, 4);
    let y = MultiDualNumber::variable(3.0, 1, 4);

    group.bench_function("add_4vars", |b| {
        b.iter(|| black_box(black_box(x.clone()) + black_box(y.clone())))
    });

    group.bench_function("mul_4vars", |b| {
        b.iter(|| black_box(black_box(x.clone()) * black_box(y.clone())))
    });

    group.bench_function("get_gradient_4vars", |b| {
        b.iter(|| black_box(x.get_gradient()))
    });

    group.finish();
}

/// Benchmark fixed-size multi-dual loops
fn bench_static_multi_dual(c: &mut Criterion) {
    let mut group = c.benchmark_group("static_multi_dual");

    group.bench_function("variable_creation_2vars", |b| {
        b.iter(|| black_box(StaticMultiDual::<f64, 2>::variable(black_box(1.0), 0)))
    });

    group.bench_function("variable_creation_4vars", |b| {
        b.iter(|| black_box(StaticMultiDual::<f64, 4>::variable(black_box(1.0), 0)))
    });

    let x2 = StaticMultiDual::<f64, 2>::variable(2.0, 0);
    let y2 = StaticMultiDual::<f64, 2>::variable(3.0, 1);
    group.bench_function("mul_2vars", |b| {
        b.iter(|| black_box(black_box(x2) * black_box(y2)))
    });

    let x4 = StaticMultiDual::<f64, 4>::variable(2.0, 0);
    let y4 = StaticMultiDual::<f64, 4>::variable(3.0, 1);
    group.bench_function("mul_4vars", |b| {
        b.iter(|| black_box(black_box(x4) * black_box(y4)))
    });

    group.bench_function("quadratic_gradient_4vars", |b| {
        b.iter(|| {
            let vars = StaticMultiDual::<f64, 4>::variables([1.0, 1.1, 1.2, 1.3]);
            let mut result = StaticMultiDual::<f64, 4>::constant(0.0);
            for value in vars {
                result = result + value * value;
            }
            black_box(result.gradient)
        })
    });

    group.finish();
}

/// Benchmark gradient computation
fn bench_gradient_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gradient_computation");

    // Simple function gradient: f(x,y) = x^2 + y^2
    group.bench_function("simple_gradient", |b| {
        b.iter(|| {
            let x = MultiDualNumber::variable(black_box(1.5), 0, 2);
            let y = MultiDualNumber::variable(black_box(2.0), 1, 2);

            let term1 = x.clone() * x.clone();
            let term2 = y.clone() * y.clone();
            let result = term1 + term2;

            black_box(result.gradient.clone())
        })
    });

    // Quadratic function: f(x) = sum of x_i^2
    for dim in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("quadratic_gradient", dim),
            dim,
            |b, &dim| {
                b.iter(|| {
                    let vars: Vec<MultiDualNumber<f64>> = (0..dim)
                        .map(|i| MultiDualNumber::variable(1.0 + i as f64 * 0.1, i, dim))
                        .collect();

                    let mut result = MultiDualNumber::constant(0.0, dim);
                    for v in vars.iter() {
                        result = result + v.clone() * v.clone();
                    }

                    black_box(result.gradient)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark chain rule applications
fn bench_chain_rule(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_rule");

    // f(g(h(x))) = sin(exp(x^2))
    group.bench_function("triple_composition", |b| {
        b.iter(|| {
            let x = DualNumber::variable(black_box(0.5));
            let h = x * x; // x^2
            let g = h.exp(); // exp(x^2)
            let f = g.sin(); // sin(exp(x^2))
            black_box(f.dual)
        })
    });

    // Deeply nested composition
    for depth in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("nested_tanh", depth),
            depth,
            |b, &depth| {
                b.iter(|| {
                    let mut x = DualNumber::variable(black_box(0.1));
                    for _ in 0..depth {
                        x = x.tanh();
                    }
                    black_box(x.dual)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark batch operations
fn bench_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_gradient", size),
            size,
            |b, &size| {
                let inputs: Vec<DualNumber<f64>> = (0..size)
                    .map(|i| DualNumber::variable(i as f64 * 0.01))
                    .collect();

                b.iter(|| {
                    let outputs: Vec<_> = inputs.iter().map(|x| x.sin() * x.exp()).collect();
                    let gradients: Vec<_> = outputs.iter().map(|x| x.dual).collect();
                    black_box(gradients)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    dual_benchmarks,
    bench_dual_arithmetic,
    bench_dual_creation,
    bench_dual_transcendental,
    bench_multi_dual,
    bench_static_multi_dual,
    bench_gradient_computation,
    bench_chain_rule,
    bench_batch_operations,
);

criterion_main!(dual_benchmarks);
