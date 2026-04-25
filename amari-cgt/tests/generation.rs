use amari_cgt::{CanonicalGame, CgtError, GameArena};
use std::collections::HashSet;

#[test]
fn birthday_generation_recovers_small_named_games() {
    let mut arena = GameArena::new();
    let games = arena.generate_by_birthday(1).unwrap();
    let game_set: HashSet<_> = games.into_iter().collect();

    assert_eq!(game_set.len(), 4);
    assert!(game_set.contains(&arena.zero()));
    assert!(game_set.contains(&arena.star().unwrap()));
    assert!(game_set.contains(&arena.one().unwrap()));
    assert!(game_set.contains(&arena.minus_one().unwrap()));
}

#[test]
fn birthday_generation_two_produces_full_small_layer() {
    let mut arena = GameArena::new();
    let games = arena.generate_by_birthday(2).unwrap();

    assert_eq!(games.len(), 256);
    assert!(games
        .iter()
        .all(|&game| arena.birthday(game).unwrap().0 <= 2));
}

#[test]
fn birthday_generation_reports_large_universe() {
    let mut arena = GameArena::new();
    let error = arena.generate_by_birthday(3).unwrap_err();

    assert_eq!(error, CgtError::GenerationUniverseTooLarge(256));
}

#[test]
fn exact_birthday_layer_one_recovers_named_nonzero_games() {
    let mut arena = GameArena::new();
    let layer = arena.generate_birthday_layer(1).unwrap();
    let layer_set: HashSet<_> = layer.into_iter().collect();

    assert_eq!(layer_set.len(), 3);
    assert!(!layer_set.contains(&arena.zero()));
    assert!(layer_set.contains(&arena.star().unwrap()));
    assert!(layer_set.contains(&arena.one().unwrap()));
    assert!(layer_set.contains(&arena.minus_one().unwrap()));
}

#[test]
fn birthday_layers_partition_small_generation() {
    let mut arena = GameArena::new();
    let layers = arena.generate_birthday_layers(2).unwrap();
    let cumulative = arena.generate_by_birthday(2).unwrap();
    let total_layered: usize = layers.values().map(Vec::len).sum();
    let flattened: HashSet<_> = layers.values().flatten().copied().collect();
    let cumulative_set: HashSet<_> = cumulative.into_iter().collect();

    assert_eq!(layers.get(&0).unwrap().len(), 1);
    assert_eq!(layers.get(&1).unwrap().len(), 3);
    assert_eq!(layers.get(&2).unwrap().len(), 252);
    assert_eq!(total_layered, 256);
    assert_eq!(flattened, cumulative_set);
}

#[test]
fn reachable_node_count_counts_shared_subgraphs_once() {
    let mut arena = GameArena::new();
    let one = arena.one().unwrap();
    let shared = arena.from_options([one], [one]).unwrap();

    assert_eq!(arena.reachable_node_count(shared).unwrap(), 3);
}

#[test]
fn node_count_generation_finds_small_three_node_games() {
    let mut arena = GameArena::new();
    let games = arena.generate_by_node_count(3).unwrap();
    let game_set: HashSet<_> = games.iter().copied().collect();

    let zero = arena.zero();
    let one = arena.one().unwrap();
    let half = arena.from_options([zero], [one]).unwrap();

    assert!(game_set.contains(&zero));
    assert!(game_set.contains(&one));
    assert!(game_set.contains(&half));
    assert!(games
        .iter()
        .all(|&game| arena.reachable_node_count(game).unwrap() <= 3));
}

#[test]
fn exact_node_count_layer_two_recovers_small_two_node_games() {
    let mut arena = GameArena::new();
    let layer = arena.generate_node_count_layer(2).unwrap();
    let layer_set: HashSet<_> = layer.into_iter().collect();

    assert_eq!(layer_set.len(), 3);
    assert!(!layer_set.contains(&arena.zero()));
    assert!(layer_set.contains(&arena.star().unwrap()));
    assert!(layer_set.contains(&arena.one().unwrap()));
    assert!(layer_set.contains(&arena.minus_one().unwrap()));
}

#[test]
fn node_count_layers_partition_small_generation() {
    let mut arena = GameArena::new();
    let layers = arena.generate_node_count_layers(3).unwrap();
    let cumulative = arena.generate_by_node_count(3).unwrap();
    let total_layered: usize = layers.values().map(Vec::len).sum();
    let flattened: HashSet<_> = layers.values().flatten().copied().collect();
    let cumulative_set: HashSet<_> = cumulative.into_iter().collect();

    assert_eq!(layers.get(&1).unwrap().len(), 1);
    assert_eq!(layers.get(&2).unwrap().len(), 3);
    assert!(!layers.get(&3).unwrap().is_empty());
    assert_eq!(total_layered, flattened.len());
    assert_eq!(flattened, cumulative_set);
}

#[test]
fn canonical_corpus_by_node_count_deduplicates_equivalent_games() {
    let mut arena = GameArena::new();
    let raw_games = arena.generate_by_node_count(3).unwrap();
    let corpus = arena.canonical_corpus_by_node_count(3).unwrap();

    assert!(corpus.len() < raw_games.len());

    let one = arena.one().unwrap();
    let canonical_one = arena.canonicalize(one).unwrap();
    let occurrences = corpus.iter().filter(|&&game| game == canonical_one).count();

    assert_eq!(occurrences, 1);
}

#[test]
fn canonical_corpus_by_birthday_preserves_small_named_canonicals() {
    let mut arena = GameArena::new();
    let corpus = arena.canonical_corpus_by_birthday(1).unwrap();
    let canonical_set: HashSet<_> = corpus.as_slice().iter().copied().collect();

    let zero = arena.zero();
    let star = arena.star().unwrap();
    let one = arena.one().unwrap();
    let minus_one = arena.minus_one().unwrap();
    let expected = [
        arena.canonicalize(zero).unwrap(),
        arena.canonicalize(star).unwrap(),
        arena.canonicalize(one).unwrap(),
        arena.canonicalize(minus_one).unwrap(),
    ];

    assert_eq!(corpus.len(), 4);
    for game in expected {
        assert!(canonical_set.contains(&game));
    }
}

#[test]
fn canonical_exact_birthday_layer_one_contains_named_triple() {
    let mut arena = GameArena::new();
    let corpus = arena.canonical_corpus_birthday_layer(1).unwrap();
    let star_id = arena.star().unwrap();
    let one_id = arena.one().unwrap();
    let minus_one_id = arena.minus_one().unwrap();
    let star = arena.canonicalize(star_id).unwrap();
    let one = arena.canonicalize(one_id).unwrap();
    let minus_one = arena.canonicalize(minus_one_id).unwrap();

    assert_eq!(corpus.len(), 3);
    assert!(corpus.contains(star));
    assert!(corpus.contains(one));
    assert!(corpus.contains(minus_one));
}

#[test]
fn canonical_layer_maps_preserve_expected_small_keys() {
    let mut arena = GameArena::new();
    let birthday_layers = arena.canonical_corpus_birthday_layers(1).unwrap();
    let node_layers = arena.canonical_corpus_node_count_layers(2).unwrap();

    assert_eq!(birthday_layers.get(&0).unwrap().len(), 1);
    assert_eq!(birthday_layers.get(&1).unwrap().len(), 3);
    assert_eq!(node_layers.get(&1).unwrap().len(), 1);
    assert_eq!(node_layers.get(&2).unwrap().len(), 3);
}

#[test]
fn canonical_corpus_constructor_deduplicates_input() {
    let mut arena = GameArena::new();
    let one_id = arena.one().unwrap();
    let one = arena.canonicalize(one_id).unwrap();
    let corpus = amari_cgt::CanonicalCorpus::new(vec![one, one]);

    assert_eq!(corpus.as_slice(), &[CanonicalGame(one.0)]);
}

#[test]
fn canonical_corpus_contains_named_games() {
    let mut arena = GameArena::new();
    let corpus = arena.canonical_corpus_by_birthday(1).unwrap();
    let zero_id = arena.zero();
    let star_id = arena.star().unwrap();
    let zero = arena.canonicalize(zero_id).unwrap();
    let star = arena.canonicalize(star_id).unwrap();

    assert!(corpus.contains(zero));
    assert!(corpus.contains(star));
}

#[test]
fn canonical_corpus_buckets_group_small_named_games() {
    let mut arena = GameArena::new();
    let corpus = arena.canonical_corpus_by_birthday(1).unwrap();
    let birthday_buckets = corpus.birthday_buckets(&arena).unwrap();
    let node_buckets = corpus.reachable_node_buckets(&arena).unwrap();

    assert_eq!(birthday_buckets.get(&0).unwrap().len(), 1);
    assert_eq!(birthday_buckets.get(&1).unwrap().len(), 3);
    assert_eq!(node_buckets.get(&1).unwrap().len(), 1);
    assert_eq!(node_buckets.get(&2).unwrap().len(), 3);
}

#[test]
fn canonical_corpus_stats_classify_small_named_games() {
    let mut arena = GameArena::new();
    let corpus = arena.canonical_corpus_by_birthday(1).unwrap();
    let stats = corpus.stats(&mut arena).unwrap();

    assert_eq!(stats.total_games(), 4);
    assert_eq!(stats.birthday_histogram().get(&0), Some(&1));
    assert_eq!(stats.birthday_histogram().get(&1), Some(&3));
    assert_eq!(stats.reachable_node_histogram().get(&1), Some(&1));
    assert_eq!(stats.reachable_node_histogram().get(&2), Some(&3));
    assert_eq!(stats.outcome_counts().left_wins(), 1);
    assert_eq!(stats.outcome_counts().right_wins(), 1);
    assert_eq!(stats.outcome_counts().next_player_wins(), 1);
    assert_eq!(stats.outcome_counts().previous_player_wins(), 1);
    assert_eq!(stats.outcome_counts().total(), 4);
    assert_eq!(stats.impartial_games(), 2);
    assert_eq!(stats.partizan_games(), 2);
    assert_eq!(stats.numeric_games(), 3);
    assert_eq!(stats.non_numeric_games(), 1);
}
