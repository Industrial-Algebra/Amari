#![cfg(feature = "experimental-epsilon")]

use amari_surreal::epsilon::{EpsilonPolynomial, EpsilonRational};
use amari_surreal::{RationalSurreal, SurrealError};

#[test]
fn epsilon_is_positive_infinitesimal() {
    let eps = EpsilonRational::epsilon();
    assert!(eps > EpsilonRational::zero());
    // from_ratio(1, 1_000_000) is known-valid because the denominator is nonzero.
    assert!(eps < EpsilonRational::from_scalar(RationalSurreal::from_ratio(1, 1_000_000).unwrap()));
}

#[test]
fn epsilon_squared_is_strictly_smaller_than_epsilon() {
    let eps = EpsilonRational::epsilon();
    let eps_sq = eps.clone() * eps.clone();
    assert!(eps_sq < eps);
}

#[test]
fn inverse_epsilon_is_larger_than_any_test_integer() {
    let inv = EpsilonRational::one()
        .checked_div(&EpsilonRational::epsilon())
        .unwrap();
    assert!(inv > EpsilonRational::from_scalar(RationalSurreal::from_integer(1_000_000)));
}

#[test]
fn epsilon_rational_arithmetic_is_exact() {
    let eps = EpsilonRational::epsilon();
    let one = EpsilonRational::one();
    let expr = (one.clone() + eps.clone())
        .checked_div(&(one.clone() - eps.clone()))
        .unwrap();
    assert_eq!(expr.denom().degree(), Some(1));
}

#[test]
fn epsilon_rational_polynomial_degree() {
    // 3 + 2ε + ε² as a polynomial
    let coeffs = vec![
        RationalSurreal::from_integer(3),
        RationalSurreal::from_integer(2),
        RationalSurreal::one(),
    ];
    let poly = EpsilonRational::from_polynomial(coeffs.clone());
    let numer: &EpsilonPolynomial = poly.numer();
    assert_eq!(numer.degree(), Some(2));
    assert_eq!(poly.denom().degree(), Some(0));
}

#[test]
fn epsilon_rational_polynomial_ordering() {
    // 3 + 2ε + ε²
    let coeffs = vec![
        RationalSurreal::from_integer(3),
        RationalSurreal::from_integer(2),
        RationalSurreal::one(),
    ];
    let poly = EpsilonRational::from_polynomial(coeffs.clone());
    // It must be positive and larger than 3
    assert!(poly > EpsilonRational::from_scalar(RationalSurreal::from_integer(3)));
    // And less than 4 (since 2ε + ε² < 1 for small epsilon)
    assert!(poly < EpsilonRational::from_scalar(RationalSurreal::from_integer(4)));
}

// -- from_parts with non-scalar denominator ---------------------------------

#[test]
fn from_parts_with_nonscalar_denominator() {
    // ε / (1 + ε) — denominator has degree 1, not a scalar constant.
    let numer = EpsilonPolynomial::epsilon();
    let denom =
        EpsilonPolynomial::from_coefficients(vec![RationalSurreal::one(), RationalSurreal::one()]);
    let r = EpsilonRational::from_parts(numer, denom).unwrap();
    // Denominator is not simplified to 1 because it has degree > 0.
    assert_eq!(r.denom().degree(), Some(1));
    // Value is positive and between 0 and ε (since ε/(1+ε) < ε).
    assert!(r > EpsilonRational::zero());
    assert!(r < EpsilonRational::epsilon());
}

// -- checked_reciprocal of non-scalar ---------------------------------------

#[test]
fn checked_reciprocal_of_nonscalar() {
    // 1 + ε as a rational function (denominator 1).
    let r = EpsilonRational::from_polynomial(vec![RationalSurreal::one(), RationalSurreal::one()]);
    // reciprocal = 1 / (1 + ε). The denominator is (1 + ε), degree 1.
    let recip = r.checked_reciprocal().unwrap();
    assert_eq!(recip.denom().degree(), Some(1));
    assert!(recip > EpsilonRational::zero());
    assert!(recip < EpsilonRational::one());
}

// -- error paths ------------------------------------------------------------

#[test]
fn checked_reciprocal_of_zero_returns_division_by_zero() {
    let result = EpsilonRational::zero().checked_reciprocal();
    assert!(matches!(result, Err(SurrealError::DivisionByZero)));
}

#[test]
fn checked_div_by_zero_returns_division_by_zero() {
    let result = EpsilonRational::one().checked_div(&EpsilonRational::zero());
    assert!(matches!(result, Err(SurrealError::DivisionByZero)));
}
