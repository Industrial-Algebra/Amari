use amari_surreal::Dyadic;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn dyadic_addition(c: &mut Criterion) {
    c.bench_function("dyadic_addition", |b| {
        let lhs = Dyadic::new(3, 2);
        let rhs = Dyadic::new(5, 3);
        b.iter(|| black_box(black_box(lhs.clone()) + black_box(rhs.clone())));
    });
}

criterion_group!(dyadic_benches, dyadic_addition);
criterion_main!(dyadic_benches);
