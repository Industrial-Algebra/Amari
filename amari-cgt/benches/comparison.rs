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

fn canonical_corpus_node_count_three(c: &mut Criterion) {
    c.bench_function("canonical_corpus_node_count_3", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .canonical_corpus_by_node_count(3)
                    .expect("corpus generation should succeed"),
            )
        });
    });
}

fn generate_exact_layers(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate_exact_layers");

    group.bench_function("birthday_layer_2", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .generate_birthday_layer(2)
                    .expect("birthday layer generation should succeed"),
            )
        });
    });

    group.bench_function("node_count_layer_3", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .generate_node_count_layer(3)
                    .expect("node-count layer generation should succeed"),
            )
        });
    });

    group.finish();
}

fn analyze_canonical_reduction_trends(c: &mut Criterion) {
    let mut group = c.benchmark_group("canonical_reduction_trends");

    group.bench_function("canonical_corpus_birthday_2", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .canonical_corpus_by_birthday(2)
                    .expect("canonical birthday corpus should succeed"),
            )
        });
    });

    group.bench_function("analyze_birthday_layer_2", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .analyze_birthday_layer(2)
                    .expect("birthday layer analysis should succeed"),
            )
        });
    });

    group.bench_function("analyze_birthday_layers_2", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .analyze_birthday_layers(2)
                    .expect("birthday layer report should succeed"),
            )
        });
    });

    group.bench_function("analyze_node_count_layers_3", |b| {
        b.iter(|| {
            let mut arena = GameArena::new();
            black_box(
                arena
                    .analyze_node_count_layers(3)
                    .expect("node-count layer report should succeed"),
            )
        });
    });

    group.finish();
}

criterion_group!(
    comparison_benches,
    compare_small_games,
    canonicalize_small_game,
    grundy_nim_heap,
    canonical_corpus_node_count_three,
    generate_exact_layers,
    analyze_canonical_reduction_trends
);
criterion_main!(comparison_benches);
