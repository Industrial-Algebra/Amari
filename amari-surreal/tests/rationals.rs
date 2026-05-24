use amari_surreal::{Dyadic, RationalSurreal, ShortSurreal, SurrealError};
use std::cmp::Ordering;

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
fn rational_surreal_normalizes_negative_ratios() {
    // Negative numerator with positive denominator
    let r = RationalSurreal::from_ratio(-2, 4).unwrap();
    assert_eq!(r.to_string(), "-1/2");
    assert!(r.is_negative());

    // Positive numerator with negative denominator
    let r = RationalSurreal::from_ratio(2, -4).unwrap();
    assert_eq!(r.to_string(), "-1/2");
    assert!(r.is_negative());

    // Both negative
    let r = RationalSurreal::from_ratio(-3, -6).unwrap();
    assert_eq!(r.to_string(), "1/2");
    assert!(r.is_positive());
}

#[test]
fn rational_surreal_equality_of_normalized_rationals() {
    let a = RationalSurreal::from_ratio(2, 4).unwrap();
    let b = RationalSurreal::from_ratio(1, 2).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.to_string(), b.to_string());

    // Negative equivalence
    let c = RationalSurreal::from_ratio(-2, 4).unwrap();
    let d = RationalSurreal::from_ratio(1, -2).unwrap();
    assert_eq!(c, d);
    assert_eq!(c.to_string(), "-1/2");
    assert_eq!(d.to_string(), "-1/2");
}

#[test]
fn rational_surreal_ordering() {
    let neg_half = RationalSurreal::from_ratio(-1, 2).unwrap();
    let zero = RationalSurreal::zero();
    let third = RationalSurreal::from_ratio(1, 3).unwrap();

    assert_eq!(neg_half.cmp(&zero), Ordering::Less);
    assert_eq!(zero.cmp(&third), Ordering::Less);
    assert_eq!(neg_half.cmp(&third), Ordering::Less);

    // Complete chain: -1/2 < 0 < 1/3
    assert!(neg_half < zero);
    assert!(zero < third);
    assert!(neg_half < third);

    // Reflexivity: equal values compare equal
    assert_eq!(zero.cmp(&RationalSurreal::zero()), Ordering::Equal);
    let another_third = RationalSurreal::from_ratio(2, 6).unwrap();
    assert_eq!(third.cmp(&another_third), Ordering::Equal);
}

#[test]
fn rational_surreal_from_integer() {
    let zero = RationalSurreal::from_integer(0);
    assert_eq!(zero, RationalSurreal::zero());
    assert!(zero.is_zero());

    let pos = RationalSurreal::from_integer(42);
    assert_eq!(pos.to_string(), "42");
    assert!(pos.is_positive());

    let neg = RationalSurreal::from_integer(-7);
    assert_eq!(neg.to_string(), "-7");
    assert!(neg.is_negative());
}

#[test]
fn rational_surreal_abs() {
    let zero = RationalSurreal::zero();
    assert_eq!(zero.abs(), RationalSurreal::zero());

    let pos = RationalSurreal::from_ratio(3, 4).unwrap();
    assert_eq!(pos.abs(), pos);

    let neg = RationalSurreal::from_ratio(-3, 4).unwrap();
    let abs_neg = neg.abs();
    assert_eq!(abs_neg.to_string(), "3/4");
    assert!(abs_neg.is_positive());
}

#[test]
fn rational_surreal_checked_reciprocal() {
    // Success case
    let two_thirds = RationalSurreal::from_ratio(2, 3).unwrap();
    let recip = two_thirds.checked_reciprocal().unwrap();
    assert_eq!(recip.to_string(), "3/2");

    // Reciprocal of negative
    let neg = RationalSurreal::from_ratio(-4, 5).unwrap();
    let neg_recip = neg.checked_reciprocal().unwrap();
    assert_eq!(neg_recip.to_string(), "-5/4");

    // Reciprocal of one
    assert_eq!(
        RationalSurreal::one().checked_reciprocal().unwrap(),
        RationalSurreal::one()
    );

    // Zero fails
    assert_eq!(
        RationalSurreal::zero().checked_reciprocal(),
        Err(SurrealError::DivisionByZero)
    );
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
