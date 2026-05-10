use amari_surcomplex::{RationalSurcomplex, SurcomplexError};
use amari_surreal::RationalSurreal;

#[test]
fn rational_surcomplex_multiplies_i_squared_to_minus_one() {
    let i = RationalSurcomplex::i();
    assert_eq!(i.clone() * i, RationalSurcomplex::from_integer(-1));
}

#[test]
fn rational_surcomplex_division_produces_non_dyadic_coefficients() {
    let one = RationalSurreal::one();
    let half = RationalSurreal::from_ratio(1, 2).unwrap();
    let z = RationalSurcomplex::from_parts(one.clone(), half);
    let quotient = RationalSurcomplex::one().checked_div(&z).unwrap();
    assert_eq!(quotient.real().to_string(), "4/5");
    assert_eq!(quotient.imag().to_string(), "-2/5");
}

#[test]
fn rational_surcomplex_norm_and_conjugate_identity() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(3, 2).unwrap(),
        RationalSurreal::from_ratio(-5, 3).unwrap(),
    );
    let product = z.clone() * z.conjugate();
    assert_eq!(product.imag(), &RationalSurreal::zero());
    assert_eq!(product.real(), &z.norm_sq());
}

// ── division-by-zero error paths ──────────────────────────────────

#[test]
fn checked_reciprocal_of_zero_returns_error() {
    assert_eq!(
        RationalSurcomplex::zero().checked_reciprocal(),
        Err(SurcomplexError::DivisionByZero),
    );
}

#[test]
fn checked_div_by_zero_returns_error() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(3, 1).unwrap(),
        RationalSurreal::from_ratio(4, 1).unwrap(),
    );
    assert_eq!(
        z.checked_div(&RationalSurcomplex::zero()),
        Err(SurcomplexError::DivisionByZero),
    );
}

// ── core identities ──────────────────────────────────────────────

#[test]
fn zero_is_additive_identity() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(7, 5).unwrap(),
        RationalSurreal::from_ratio(-3, 2).unwrap(),
    );
    let zero = RationalSurcomplex::zero();
    assert_eq!(z.clone() + zero.clone(), z);
    assert_eq!(zero + z.clone(), z);
}

#[test]
fn one_is_multiplicative_identity() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(7, 5).unwrap(),
        RationalSurreal::from_ratio(-3, 2).unwrap(),
    );
    let one = RationalSurcomplex::one();
    assert_eq!(z.clone() * one.clone(), z);
    assert_eq!(one * z.clone(), z);
}

#[test]
fn negation_is_additive_inverse() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(7, 5).unwrap(),
        RationalSurreal::from_ratio(-3, 2).unwrap(),
    );
    let neg = -z.clone();
    assert_eq!(z + neg, RationalSurcomplex::zero());
}

// ── Display smoke tests ──────────────────────────────────────────

#[test]
fn display_zero() {
    assert_eq!(RationalSurcomplex::zero().to_string(), "0");
}

#[test]
fn display_pure_real() {
    let r = RationalSurcomplex::from_integer(5);
    assert_eq!(r.to_string(), "5");
}

#[test]
fn display_i() {
    assert_eq!(RationalSurcomplex::i().to_string(), "1i");
}

#[test]
fn display_full_complex() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(3, 2).unwrap(),
        RationalSurreal::from_ratio(-5, 3).unwrap(),
    );
    assert_eq!(z.to_string(), "3/2 - 5/3i");
}
