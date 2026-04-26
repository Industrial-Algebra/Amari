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
    assert!(Dyadic::new(-1, 1).is_negative());
    assert!(Dyadic::new(1, 1).is_positive());
    assert_eq!(Dyadic::new(-3, 2).abs(), Dyadic::new(3, 2));
}

#[test]
fn dyadic_division_and_reciprocal_respect_short_surreal_layer() {
    assert_eq!(
        Dyadic::from_integer(2).checked_reciprocal().unwrap(),
        Dyadic::new(1, 1)
    );
    assert_eq!(
        Dyadic::from_integer(-4).checked_reciprocal().unwrap(),
        Dyadic::new(-1, 2)
    );

    assert_eq!(
        Dyadic::new(3, 1)
            .checked_div(&Dyadic::from_integer(3))
            .unwrap(),
        Dyadic::new(1, 1)
    );
    assert_eq!(
        Dyadic::one().checked_div(&Dyadic::from_integer(3)),
        Err(SurrealError::NonDyadicQuotient)
    );
    assert_eq!(
        Dyadic::from_integer(3).checked_reciprocal(),
        Err(SurrealError::NonDyadicQuotient)
    );
    assert_eq!(
        Dyadic::one().checked_div(&Dyadic::zero()),
        Err(SurrealError::DivisionByZero)
    );
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

    let one_from_interval = ShortSurreal::simplest_between(
        std::slice::from_ref(&half),
        std::slice::from_ref(&three_halves),
    )
    .unwrap();
    assert_eq!(one_from_interval.to_dyadic(), Dyadic::one());
}

#[test]
fn convert_numeric_game_to_short_surreal() {
    let mut arena = GameArena::new();
    let zero = arena.zero();
    let one = arena.one().unwrap();
    let half_game = arena.from_options([zero], [one]).unwrap();

    let half = ShortSurreal::from_game(&mut arena, half_game).unwrap();
    assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));
    assert_eq!(half, ShortSurreal::from_dyadic(Dyadic::new(1, 1)));
    assert_eq!(half.birthday().0, arena.birthday(half_game).unwrap().0);
    assert_eq!(half.provenance(), Some(half_game));
}

#[test]
fn short_surreal_checked_reciprocal_and_division_work() {
    let three_halves = ShortSurreal::from_dyadic(Dyadic::new(3, 1));
    let three = ShortSurreal::from_integer(3);
    let two = ShortSurreal::from_integer(2);

    assert_eq!(
        two.checked_reciprocal().unwrap().to_dyadic(),
        Dyadic::new(1, 1)
    );
    assert_eq!(
        three_halves.checked_div(&three).unwrap().to_dyadic(),
        Dyadic::new(1, 1)
    );
    assert_eq!(
        ShortSurreal::one().checked_div(&three),
        Err(SurrealError::NonDyadicQuotient)
    );
}

#[test]
fn short_surreal_ordering_and_utilities_work() {
    let minus_half = ShortSurreal::from_dyadic(Dyadic::new(-1, 1));
    let zero = ShortSurreal::zero();
    let half = ShortSurreal::from_dyadic(Dyadic::new(1, 1));
    let one = ShortSurreal::one();

    assert!(minus_half < zero);
    assert!(zero < half);
    assert!(half < one);
    assert!(minus_half.is_negative());
    assert!(half.is_positive());
    assert!(zero.is_zero());
    assert_eq!(minus_half.abs(), half);

    let mut values = vec![half.clone(), minus_half.clone(), one.clone(), zero.clone()];
    values.sort();
    let sorted_dyadics: Vec<_> = values.into_iter().map(|value| value.to_dyadic()).collect();
    assert_eq!(
        sorted_dyadics,
        vec![
            Dyadic::new(-1, 1),
            Dyadic::zero(),
            Dyadic::new(1, 1),
            Dyadic::one()
        ]
    );
}

#[test]
fn short_surreal_round_trips_through_cgt() {
    let values = vec![
        ShortSurreal::zero(),
        ShortSurreal::from_integer(-2),
        ShortSurreal::from_dyadic(Dyadic::new(-3, 2)),
        ShortSurreal::from_dyadic(Dyadic::new(1, 1)),
        ShortSurreal::from_dyadic(Dyadic::new(5, 2)),
    ];

    let mut arena = GameArena::new();
    for value in values {
        let game = value.to_game_in(&mut arena).unwrap();
        assert!(arena.is_numeric(game).unwrap());

        let round_trip = ShortSurreal::from_game(&mut arena, game).unwrap();
        assert_eq!(round_trip, value);
        assert_eq!(round_trip.to_dyadic(), value.to_dyadic());
        assert_eq!(round_trip.birthday(), value.birthday());
        assert_eq!(round_trip.provenance(), Some(game));
    }
}

#[test]
fn numeric_games_round_trip_back_through_short_surreal_reconstruction() {
    let mut source = GameArena::new();
    let zero = source.zero();
    let one = source.one().unwrap();
    let two = source.from_options([one], []).unwrap();
    let half_game = source.from_options([zero], [one]).unwrap();
    let three_halves_game = source.from_options([one], [two]).unwrap();

    for game in [half_game, three_halves_game] {
        let value = ShortSurreal::from_game(&mut source, game).unwrap();

        let mut target = GameArena::new();
        let rebuilt_game = value.to_game_in(&mut target).unwrap();
        assert!(target.is_numeric(rebuilt_game).unwrap());

        let rebuilt_value = ShortSurreal::from_game(&mut target, rebuilt_game).unwrap();
        assert_eq!(rebuilt_value, value);
        assert_eq!(rebuilt_value.birthday(), value.birthday());
        assert_eq!(rebuilt_value.provenance(), Some(rebuilt_game));
    }
}

#[test]
fn non_numeric_game_is_rejected() {
    let mut arena = GameArena::new();
    let star = arena.star().unwrap();

    let result = ShortSurreal::from_game(&mut arena, star);
    assert!(matches!(result, Err(SurrealError::NotNumericGame(_))));
}
