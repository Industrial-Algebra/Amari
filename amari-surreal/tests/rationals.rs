use amari_surreal::{Dyadic, RationalSurreal, ShortSurreal, SurrealError};

#[test]
fn rational_surreal_normalizes_and_displays() {
    let value = RationalSurreal::from_ratio(2, 4).unwrap();
    assert_eq!(value.to_string(), "1/2");
    assert_eq!(value.numer().to_string(), "1");
    assert_eq!(value.denom().to_string(), "2");
}

#[test]
fn rational_surreal_supports_exact_field_arithmetic() {
    let third = RationalSurreal::from_ratio(1, 3).unwrap();
    let sixth = RationalSurreal::from_ratio(1, 6).unwrap();
    assert_eq!((third.clone() + sixth.clone()).to_string(), "1/2");
    assert_eq!((third.clone() * sixth.clone()).to_string(), "1/18");
    assert_eq!(third.checked_div(&sixth).unwrap().to_string(), "2");
}

#[test]
fn rational_surreal_converts_to_short_only_when_dyadic() {
    let half = RationalSurreal::from_ratio(1, 2).unwrap();
    assert_eq!(
        half.to_short_if_dyadic().unwrap().to_dyadic(),
        Dyadic::new(1, 1)
    );

    let third = RationalSurreal::from_ratio(1, 3).unwrap();
    assert!(third.to_short_if_dyadic().is_none());
}

#[test]
fn rational_surreal_embeds_short_surreal() {
    let short = ShortSurreal::from_dyadic(Dyadic::new(3, 2));
    let rational = RationalSurreal::from_short(short);
    assert_eq!(rational.to_string(), "3/4");
}

#[test]
fn rational_surreal_rejects_zero_denominator_and_division_by_zero() {
    assert_eq!(
        RationalSurreal::from_ratio(1, 0),
        Err(SurrealError::DivisionByZero)
    );
    assert_eq!(
        RationalSurreal::one().checked_div(&RationalSurreal::zero()),
        Err(SurrealError::DivisionByZero)
    );
}
