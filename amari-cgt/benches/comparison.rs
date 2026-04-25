use amari_cgt::GameArena;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn compare_small_games(c: &mut Criterion) {
    c.bench_function("compare_one_vs_zero", |b| {
        let mut arena = GameArena::new();
        let zero = arena.zero();
        let one = arena.one().expect("one should construct");
        b.iter(|| {
            black_box(
                arena
                    .compare(black_box(one), black_box(zero))
                    .expect("comparison"),
            )
        });
    });
}

criterion_group!(comparison_benches, compare_small_games);
criterion_main!(comparison_benches);
