use amari_cgt::GameArena;
use amari_surreal::{Dyadic, ShortSurreal, SurrealError};

#[test]
fn dyadic_normalization_and_arithmetic() {
    let half = Dyadic::new(2, 2);
    assert_eq!(half, Dyadic::new(1, 1));

    let three_halves = Dyadic::new(3, 1);
    let quarter = Dyadic::new(1, 2);
    assert_eq!(three_halves.clone() + quarter.clone(), Dyadic::new(7, 2));
    assert_eq!(three_halves - quarter, Dyadic::new(5, 2));
}

#[test]
fn simplest_between_standard_examples() {
    let zero = ShortSurreal::zero();
    let one = ShortSurreal::one();
    let half =
        ShortSurreal::simplest_between(std::slice::from_ref(&zero), std::slice::from_ref(&one))
            .unwrap();

    assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));

    let two = ShortSurreal::from_integer(2);
    let three_halves =
        ShortSurreal::simplest_between(std::slice::from_ref(&one), std::slice::from_ref(&two))
            .unwrap();
    assert_eq!(three_halves.to_dyadic(), Dyadic::new(3, 1));
}

#[test]
fn convert_numeric_game_to_short_surreal() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let one = arena.one().unwrap();
    let half_game = arena.from_options([zero], [one]).unwrap();

    let half = ShortSurreal::from_game(&mut arena, half_game).unwrap();
    assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));
    assert_eq!(half.birthday().0, arena.birthday(half_game).unwrap().0);
    assert_eq!(half.provenance(), Some(half_game));
}

#[test]
fn non_numeric_game_is_rejected() {
    let mut arena = GameArena::new();
    let star = arena.star().unwrap();

    let result = ShortSurreal::from_game(&mut arena, star);
    assert!(matches!(result, Err(SurrealError::NotNumericGame(_))));
}
