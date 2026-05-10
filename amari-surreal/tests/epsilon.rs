#![cfg(feature = "experimental-epsilon")]

use amari_surreal::epsilon::{EpsilonPolynomial, EpsilonRational};
use amari_surreal::RationalSurreal;

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
