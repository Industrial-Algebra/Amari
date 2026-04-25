use amari_cgt::{GameArena, GameComparison, Nimber, OutcomeClass};

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
}
