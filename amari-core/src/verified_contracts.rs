//! Practical Creusot contracts for geometric algebra verification
//!
//! This module demonstrates the integration of Creusot formal verification
//! with concrete geometric algebra operations.

use super::verified::VerifiedMultivector;
use creusot_contracts::macros::requires;
use num_traits::{Float, One, Zero};

/// True iff every coefficient is finite (no `NaN`, no ±infinity).
///
/// M-REP-C runtime precondition predicate for the equality contracts. It
/// mirrors the construction-time finiteness guarantee enforced by
/// [`VerifiedMultivector::new`]. `T: Float` supplies `is_finite`.
fn coefficients_are_finite<T: Float>(coefficients: &[T]) -> bool {
    coefficients
        .iter()
        .all(|coefficient| coefficient.is_finite())
}

/// Verified scalar multiplication
#[requires(scalar != T::zero())]
#[requires(scalar.is_finite())]
#[requires(coefficients_are_finite(&mv.coefficients))]
#[ensures(result.coefficients.len() == mv.coefficients.len())]
#[ensures(result.coefficients[0] == scalar * mv.coefficients[0])]
pub fn verified_scalar_mult<T, const P: usize, const Q: usize, const R: usize>(
    scalar: T,
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> VerifiedMultivector<T, P, Q, R>
where
    T: Float + Zero + One + Copy,
{
    debug_assert!(
        scalar.is_finite(),
        "verified_scalar_mult: scalar must be finite"
    );
    debug_assert!(
        coefficients_are_finite(&mv.coefficients),
        "verified_scalar_mult: all multivector coefficients must be finite"
    );

    let mut coefficients = Vec::with_capacity(mv.coefficients.len());
    for &coeff in &mv.coefficients {
        coefficients.push(scalar * coeff);
    }

    VerifiedMultivector::new(coefficients).unwrap()
}

/// Verified addition with mathematical properties
#[requires(coefficients_are_finite(&a.coefficients))]
#[requires(coefficients_are_finite(&b.coefficients))]
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
    debug_assert!(
        coefficients_are_finite(&a.coefficients),
        "verified_addition: all coefficients of the first operand must be finite"
    );
    debug_assert!(
        coefficients_are_finite(&b.coefficients),
        "verified_addition: all coefficients of the second operand must be finite"
    );

    a.add(b)
}

/// Norm computation (sum of squares, then square root).
///
/// The former `ensures(result >= T::zero())` contract was removed on
/// 2026-10-15 (M-REP-A review round 1): it is falsified by NaN
/// coefficients (`verified_norm([NaN, 0]) == NaN`, and `NaN >= 0` is
/// false). M-REP-C (2026-10-15) closed the non-finite gap at construction
/// (`VerifiedMultivector` constructors reject `NaN`/±infinity) and added the
/// sound, sqrt-free nonnegativity contract
/// [`Vector::verified_self_dot`](super::verified::Vector::verified_self_dot).
/// This function keeps only its non-emptiness precondition: it still does not
/// assert nonnegativity, because a `VerifiedMultivector` produced by an
/// overflowing operation can carry `inf`.
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
#[requires(coefficients_are_finite(&mv.coefficients))]
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
    debug_assert!(
        coefficients_are_finite(&mv.coefficients),
        "verify_scalar_identity: all coefficients must be finite"
    );

    let identity = VerifiedMultivector::scalar(T::one()).unwrap();
    mv.geometric_product(&identity)
}

/// Verification that zero addition holds
#[requires(mv.coefficients.len() == VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE)]
#[requires(coefficients_are_finite(&mv.coefficients))]
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
    debug_assert!(
        coefficients_are_finite(&mv.coefficients),
        "verify_zero_addition: all coefficients must be finite"
    );

    let zero = VerifiedMultivector::scalar(T::zero()).unwrap();
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
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
        let result = verified_scalar_mult(3.0, &mv);

        // The annotations express these intended properties; the asserts check them for this instance
        assert_eq!(result.coefficients[0], 6.0);
        assert_eq!(result.coefficients.len(), mv.coefficients.len());
    }

    #[test]
    fn test_verified_addition_properties() {
        let mv1 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
        let mv2 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(3.0).unwrap();
        let result = verified_addition(&mv1, &mv2);

        // The annotations express intended coefficient-wise addition
        assert_eq!(result.coefficients[0], 5.0);
    }

    #[test]
    fn test_verified_identity_laws() {
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(5.0).unwrap();

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
