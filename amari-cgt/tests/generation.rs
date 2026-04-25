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
fn canonical_corpus_constructor_deduplicates_input() {
    let mut arena = GameArena::new();
    let one_id = arena.one().unwrap();
    let one = arena.canonicalize(one_id).unwrap();
    let corpus = amari_cgt::CanonicalCorpus::new(vec![one, one]);

    assert_eq!(corpus.as_slice(), &[CanonicalGame(one.0)]);
}
