use amari_cgt::{GameArena, GameComparison, GameForm, Nimber, OutcomeClass};

#[test]
fn constructors_and_birthdays() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let star = arena.star().unwrap();
    let one = arena.one().unwrap();
    let minus_one = arena.minus_one().unwrap();

    assert_eq!(arena.birthday(zero).unwrap().0, 0);
    assert_eq!(arena.birthday(star).unwrap().0, 1);
    assert_eq!(arena.birthday(one).unwrap().0, 1);
    assert_eq!(arena.birthday(minus_one).unwrap().0, 1);
}

#[test]
fn basic_comparisons_and_outcomes() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let star = arena.star().unwrap();
    let one = arena.one().unwrap();
    let minus_one = arena.minus_one().unwrap();

    assert_eq!(arena.compare(one, zero).unwrap(), GameComparison::Greater);
    assert_eq!(
        arena.compare(minus_one, zero).unwrap(),
        GameComparison::Less
    );
    assert_eq!(arena.compare(star, zero).unwrap(), GameComparison::Fuzzy);
    assert_eq!(arena.compare(zero, zero).unwrap(), GameComparison::Equal);

    assert_eq!(
        arena.outcome(zero).unwrap(),
        OutcomeClass::PreviousPlayerWins
    );
    assert_eq!(arena.outcome(star).unwrap(), OutcomeClass::NextPlayerWins);
    assert_eq!(arena.outcome(one).unwrap(), OutcomeClass::LeftWins);
    assert_eq!(arena.outcome(minus_one).unwrap(), OutcomeClass::RightWins);
}

#[test]
fn negation_and_addition() {
    let mut arena = GameArena::new();
    let one = arena.one().unwrap();
    let minus_one = arena.minus_one().unwrap();
    let star = arena.star().unwrap();

    assert_eq!(arena.neg(one).unwrap(), minus_one);
    assert_eq!(arena.neg(star).unwrap(), star);

    let sum = arena.add(one, minus_one).unwrap();
    let zero = arena.zero();
    assert!(arena.equivalent(sum, zero).unwrap());
}

#[test]
fn nimbers_match_heap_sizes() {
    let mut arena = GameArena::new();

    for size in 0..6 {
        let heap = arena.nim_heap(size).unwrap();
        assert!(arena.is_impartial(heap).unwrap());
        assert_eq!(arena.grundy(heap).unwrap(), Nimber(size));
    }
}

#[test]
fn numeric_examples_work() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let one = arena.one().unwrap();
    let half = arena.from_options([zero], [one]).unwrap();
    let star = arena.star().unwrap();

    assert!(arena.is_numeric(zero).unwrap());
    assert!(arena.is_numeric(one).unwrap());
    assert!(arena.is_numeric(half).unwrap());
    assert!(!arena.is_numeric(star).unwrap());

    assert_eq!(arena.validate_numeric(half).unwrap().game_id(), half);
    assert!(matches!(
        arena.validate_numeric(star),
        Err(amari_cgt::CgtError::NotNumericGame(game)) if game == star
    ));
}

#[test]
fn option_deduplication_is_stable() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let one = arena.one().unwrap();
    let duplicate = arena.from_options([zero, zero], []).unwrap();

    assert_eq!(duplicate, one);
}

#[test]
fn canonicalization_removes_dominated_left_options() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let minus_one = arena.minus_one().unwrap();
    let dominated = arena.from_options([minus_one, zero], []).unwrap();
    let canonical = arena.canonicalize(dominated).unwrap().0;
    let one = arena.one().unwrap();

    assert_eq!(canonical, one);
}

#[test]
fn canonicalization_removes_reversible_left_options() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let one = arena.one().unwrap();
    let reversible_option = arena.from_options([one], [zero]).unwrap();
    let game = arena.from_options([reversible_option], []).unwrap();
    let canonical = arena.canonicalize(game).unwrap().0;

    assert_eq!(canonical, zero);
    assert!(arena.equivalent(game, zero).unwrap());
}

#[test]
fn canonicalization_is_idempotent() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let minus_one = arena.minus_one().unwrap();
    let dominated = arena.from_options([minus_one, zero], []).unwrap();
    let once = arena.canonicalize(dominated).unwrap().0;
    let twice = arena.canonicalize(once).unwrap().0;

    assert_eq!(once, twice);
}

#[test]
fn nim_sum_matches_xor() {
    let mut arena = GameArena::new();

    for lhs in 0..6 {
        for rhs in 0..6 {
            let lhs_heap = arena.nim_heap(lhs).unwrap();
            let rhs_heap = arena.nim_heap(rhs).unwrap();
            let sum = arena.add(lhs_heap, rhs_heap).unwrap();

            assert!(arena.is_impartial(sum).unwrap());
            assert_eq!(arena.grundy(sum).unwrap(), Nimber(lhs ^ rhs));
        }
    }
}

#[test]
fn game_form_normalizes_recursive_option_sets() {
    let zero = GameForm::zero();
    let one = GameForm::one();
    let minus_one = GameForm::minus_one();
    let form = GameForm::new(
        [one.clone(), zero.clone(), zero.clone()],
        [minus_one.clone(), minus_one],
    );

    assert_eq!(form, GameForm::new([zero, one], [GameForm::minus_one()]));
    assert_eq!(GameForm::star().birthday().0, 1);
}

#[test]
fn arena_round_trips_through_structural_game_form() {
    let half_form = GameForm::new([GameForm::zero()], [GameForm::one()]);

    let mut arena = GameArena::new();
    let half = arena.intern_form(&half_form).unwrap();
    let round_trip = arena.to_form(half).unwrap();

    assert_eq!(round_trip, half_form);
    assert_eq!(arena.birthday(half).unwrap(), round_trip.birthday());
    assert!(arena.is_numeric(half).unwrap());
}

#[test]
fn canonical_form_export_matches_canonicalization() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let minus_one = arena.minus_one().unwrap();
    let dominated = arena.from_options([minus_one, zero], []).unwrap();

    assert_eq!(
        arena.to_form(dominated).unwrap(),
        GameForm::new([GameForm::minus_one(), GameForm::zero()], [])
    );
    assert_eq!(arena.canonical_form(dominated).unwrap(), GameForm::one());
}
