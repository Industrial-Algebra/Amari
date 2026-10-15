//! Practical Creusot contracts for geometric algebra verification
//!
//! This module demonstrates the integration of Creusot formal verification
//! with concrete geometric algebra operations.

use super::verified::VerifiedMultivector;
use creusot_contracts::macros::requires;
use num_traits::{Float, One, Zero};

/// Verified scalar multiplication
#[requires(scalar != T::zero())]
#[ensures(result.coefficients.len() == mv.coefficients.len())]
#[ensures(result.coefficients[0] == scalar * mv.coefficients[0])]
pub fn verified_scalar_mult<T, const P: usize, const Q: usize, const R: usize>(
    scalar: T,
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> VerifiedMultivector<T, P, Q, R>
where
    T: Float + Zero + One + Copy,
{
    let mut coefficients = Vec::with_capacity(mv.coefficients.len());
    for &coeff in &mv.coefficients {
        coefficients.push(scalar * coeff);
    }

    VerifiedMultivector::new(coefficients).unwrap()
}

/// Verified addition with mathematical properties
#[requires(a.coefficients.len() == b.coefficients.len())]
#[ensures(result.coefficients.len() == a.coefficients.len())]
#[ensures(forall(|i: usize| i < result.coefficients.len() ==>
    result.coefficients[i] == a.coefficients[i] + b.coefficients[i]
))]
pub fn verified_addition<T, const P: usize, const Q: usize, const R: usize>(
    a: &VerifiedMultivector<T, P, Q, R>,
    b: &VerifiedMultivector<T, P, Q, R>,
) -> VerifiedMultivector<T, P, Q, R>
where
    T: Float + Zero + One + Copy,
{
    a.add(b)
}

/// Norm computation (sum of squares, then square root).
///
/// The former `ensures(result >= T::zero())` contract was removed on
/// 2026-10-15 (M-REP-A review round 1): it is falsified by NaN
/// coefficients (`verified_norm([NaN, 0]) == NaN`, and `NaN >= 0` is
/// false). For finite coefficients the claim holds (sum of squares is
/// nonnegative, as is its sqrt). A finiteness-guarded contract is
/// M-REP-C scope.
#[requires(!mv.coefficients.is_empty())]
pub fn verified_norm<T, const P: usize, const Q: usize, const R: usize>(
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> T
where
    T: Float + Zero + One + Copy,
{
    mv.coefficients
        .iter()
        .map(|&c| c * c)
        .fold(T::zero(), |acc, x| acc + x)
        .sqrt()
}

/// Verified grade computation
#[requires(!mv.coefficients.is_empty())]
#[ensures(result <= P + Q + R)]
pub fn verified_grade<T, const P: usize, const Q: usize, const R: usize>(
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> usize
where
    T: Float + Zero + One,
{
    mv.grade()
}

/// Verification that scalar identity holds
#[requires(mv.coefficients.len() == VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE)]
#[ensures(result.coefficients.len() == mv.coefficients.len())]
#[ensures(forall(|i: usize| i < result.coefficients.len() ==>
    result.coefficients[i] == mv.coefficients[i]
))]
pub fn verify_scalar_identity<T, const P: usize, const Q: usize, const R: usize>(
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> VerifiedMultivector<T, P, Q, R>
where
    T: Float + Zero + One + Copy,
{
    let identity = VerifiedMultivector::scalar(T::one());
    mv.geometric_product(&identity)
}

/// Verification that zero addition holds
#[requires(mv.coefficients.len() == VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE)]
#[ensures(result.coefficients.len() == mv.coefficients.len())]
#[ensures(forall(|i: usize| i < result.coefficients.len() ==>
    result.coefficients[i] == mv.coefficients[i]
))]
pub fn verify_zero_addition<T, const P: usize, const Q: usize, const R: usize>(
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> VerifiedMultivector<T, P, Q, R>
where
    T: Float + Zero + One + Copy,
{
    let zero = VerifiedMultivector::scalar(T::zero());
    mv.add(&zero)
}

/// Verify that basis vectors square correctly according to signature
#[requires(index < P + Q + R)]
#[ensures(
    (index < P ==> (result - T::one()).abs() < T::from(0.0001).unwrap()) &&
    (P <= index && index < P + Q ==> (result + T::one()).abs() < T::from(0.0001).unwrap()) &&
    (P + Q <= index ==> result.abs() < T::from(0.0001).unwrap())
)]
pub fn verify_basis_vector_square<T, const P: usize, const Q: usize, const R: usize>(
    index: usize,
) -> T
where
    T: Float + Zero + One + Copy,
{
    let basis = VerifiedMultivector::<T, P, Q, R>::basis_vector(index).unwrap();
    let square = basis.geometric_product(&basis);

    // Extract scalar part (coefficient[0])
    square.coefficients[0]
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn test_verified_scalar_multiplication() {
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0);
        let result = verified_scalar_mult(3.0, &mv);

        // The Creusot contracts ensure these properties
        assert_eq!(result.coefficients[0], 6.0);
        assert_eq!(result.coefficients.len(), mv.coefficients.len());
    }

    #[test]
    fn test_verified_addition_properties() {
        let mv1 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0);
        let mv2 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(3.0);
        let result = verified_addition(&mv1, &mv2);

        // Contracts guarantee coefficient-wise addition
        assert_eq!(result.coefficients[0], 5.0);
    }

    #[test]
    fn test_verified_identity_laws() {
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(5.0);

        // Test multiplicative identity
        let result = verify_scalar_identity(&mv);
        assert_eq!(result.coefficients[0], mv.coefficients[0]);

        // Test additive identity
        let result = verify_zero_addition(&mv);
        assert_eq!(result.coefficients[0], mv.coefficients[0]);
    }

    #[test]
    fn test_signature_verification() {
        // Test Euclidean 3-space Cl(3,0,0)
        // e₁² = e₂² = e₃² = +1
        for i in 0..3 {
            let square = verify_basis_vector_square::<f64, 3, 0, 0>(i);
            assert!((square - 1.0).abs() < 0.0001);
        }
    }
}
