//! Practical Creusot contracts for geometric algebra verification
//!
//! This module demonstrates the integration of Creusot formal verification
//! with concrete geometric algebra operations.

use super::verified::VerifiedMultivector;
use alloc::vec::Vec;
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
///
/// The product can be non-finite even when every input is finite
/// (`2.0 * f64::MAX == inf`); the result is therefore routed through the
/// finiteness gate in [`VerifiedMultivector::new`] and reported as `Err`
/// rather than unwrapped (which formerly panicked on overflow).
#[requires(scalar != T::zero())]
#[requires(scalar.is_finite())]
#[requires(coefficients_are_finite(&mv.coefficients))]
#[ensures(result.is_ok() ==>
    result.unwrap().coefficients.len() == mv.coefficients.len())]
#[ensures(result.is_ok() ==> forall(|i: usize|
    i < result.unwrap().coefficients.len() ==>
        result.unwrap().coefficients[i] == scalar * mv.coefficients[i]
))]
pub fn verified_scalar_mult<T, const P: usize, const Q: usize, const R: usize>(
    scalar: T,
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> Result<VerifiedMultivector<T, P, Q, R>, &'static str>
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

    VerifiedMultivector::new(coefficients)
}

/// Verified addition with mathematical properties
///
/// The coefficient-wise sum can be non-finite even when both operands are
/// finite; the sum is routed through [`VerifiedMultivector::new`] and
/// reported as `Err` in that case, so the `Ok` coefficient equalities are
/// sound (a `NaN` result would otherwise falsify `result[i] == a[i] + b[i]`).
#[requires(coefficients_are_finite(&a.coefficients))]
#[requires(coefficients_are_finite(&b.coefficients))]
#[requires(a.coefficients.len() == b.coefficients.len())]
#[ensures(result.is_ok() ==>
    result.unwrap().coefficients.len() == a.coefficients.len())]
#[ensures(result.is_ok() ==> forall(|i: usize|
    i < result.unwrap().coefficients.len() ==>
        result.unwrap().coefficients[i] == a.coefficients[i] + b.coefficients[i]
))]
pub fn verified_addition<T, const P: usize, const Q: usize, const R: usize>(
    a: &VerifiedMultivector<T, P, Q, R>,
    b: &VerifiedMultivector<T, P, Q, R>,
) -> Result<VerifiedMultivector<T, P, Q, R>, &'static str>
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

    let sum = a.add(b);
    VerifiedMultivector::new(sum.coefficients)
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
#[ensures(result.is_ok() ==>
    result.unwrap().coefficients.len() == mv.coefficients.len())]
#[ensures(result.is_ok() ==> forall(|i: usize|
    i < result.unwrap().coefficients.len() ==>
        result.unwrap().coefficients[i] == mv.coefficients[i]
))]
pub fn verify_scalar_identity<T, const P: usize, const Q: usize, const R: usize>(
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> Result<VerifiedMultivector<T, P, Q, R>, &'static str>
where
    T: Float + Zero + One + Copy,
{
    debug_assert!(
        coefficients_are_finite(&mv.coefficients),
        "verify_scalar_identity: all coefficients must be finite"
    );

    let identity = VerifiedMultivector::scalar(T::one())?;
    let product = mv.geometric_product(&identity);
    VerifiedMultivector::new(product.coefficients)
}

/// Verification that zero addition holds
#[requires(mv.coefficients.len() == VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE)]
#[requires(coefficients_are_finite(&mv.coefficients))]
#[ensures(result.is_ok() ==>
    result.unwrap().coefficients.len() == mv.coefficients.len())]
#[ensures(result.is_ok() ==> forall(|i: usize|
    i < result.unwrap().coefficients.len() ==>
        result.unwrap().coefficients[i] == mv.coefficients[i]
))]
pub fn verify_zero_addition<T, const P: usize, const Q: usize, const R: usize>(
    mv: &VerifiedMultivector<T, P, Q, R>,
) -> Result<VerifiedMultivector<T, P, Q, R>, &'static str>
where
    T: Float + Zero + One + Copy,
{
    debug_assert!(
        coefficients_are_finite(&mv.coefficients),
        "verify_zero_addition: all coefficients must be finite"
    );

    let zero = VerifiedMultivector::scalar(T::zero())?;
    let sum = mv.add(&zero);
    VerifiedMultivector::new(sum.coefficients)
}

/// Verify that basis vectors square correctly according to signature
#[requires(index < P + Q + R)]
#[ensures(result.is_ok() ==> (
    (index < P ==> (result.unwrap() - T::one()).abs() < T::from(0.0001).unwrap()) &&
    (P <= index && index < P + Q ==> (result.unwrap() + T::one()).abs() < T::from(0.0001).unwrap()) &&
    (P + Q <= index ==> result.unwrap().abs() < T::from(0.0001).unwrap())
))]
pub fn verify_basis_vector_square<T, const P: usize, const Q: usize, const R: usize>(
    index: usize,
) -> Result<T, &'static str>
where
    T: Float + Zero + One + Copy,
{
    let basis = VerifiedMultivector::<T, P, Q, R>::basis_vector(index)?;
    let square = basis.geometric_product(&basis);

    // Extract scalar part (coefficient[0])
    Ok(square.coefficients[0])
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn test_verified_scalar_multiplication() {
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
        let result = verified_scalar_mult(3.0, &mv).unwrap();

        // The annotations express these intended properties; the asserts check them for this instance
        assert_eq!(result.coefficients[0], 6.0);
        assert_eq!(result.coefficients.len(), mv.coefficients.len());
    }

    #[test]
    fn test_verified_addition_properties() {
        let mv1 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
        let mv2 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(3.0).unwrap();
        let result = verified_addition(&mv1, &mv2).unwrap();

        // The annotations express intended coefficient-wise addition
        assert_eq!(result.coefficients[0], 5.0);
    }

    #[test]
    fn test_verified_identity_laws() {
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(5.0).unwrap();

        // Test multiplicative identity
        let result = verify_scalar_identity(&mv).unwrap();
        assert_eq!(result.coefficients[0], mv.coefficients[0]);

        // Test additive identity
        let result = verify_zero_addition(&mv).unwrap();
        assert_eq!(result.coefficients[0], mv.coefficients[0]);
    }

    #[test]
    fn test_signature_verification() {
        // Test Euclidean 3-space Cl(3,0,0)
        // e₁² = e₂² = e₃² = +1
        for i in 0..3 {
            let square = verify_basis_vector_square::<f64, 3, 0, 0>(i).unwrap();
            assert!((square - 1.0).abs() < 0.0001);
        }
    }
}
