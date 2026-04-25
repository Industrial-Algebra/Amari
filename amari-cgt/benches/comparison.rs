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

fn canonicalize_small_game(c: &mut Criterion) {
    c.bench_function("canonicalize_dominated_game", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            let zero = arena.zero();
            let minus_one = arena.minus_one().expect("minus one should construct");
            let dominated = arena
                .from_options([minus_one, zero], [])
                .expect("dominated game should construct");
            black_box(
                arena
                    .canonicalize(black_box(dominated))
                    .expect("canonicalize"),
            )
        });
    });
}

fn grundy_nim_heap(c: &mut Criterion) {
    c.bench_function("grundy_nim_heap_8", |b| {
        let mut arena = GameArena::new();
        let heap = arena.nim_heap(8).expect("nim heap should construct");
        b.iter(|| black_box(arena.grundy(black_box(heap)).expect("grundy")));
    });
}

criterion_group!(
    comparison_benches,
    compare_small_games,
    canonicalize_small_game,
    grundy_nim_heap
);
criterion_main!(comparison_benches);
