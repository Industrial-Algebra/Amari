//! Minimal semiring abstractions for optimization-oriented tropical carriers.
//!
//! This module intentionally stays small.
//! It captures only the identities and the two core operations needed to share
//! a common algebraic surface between the existing float max-plus layer and the
//! planned ordinal substrate.
//!
//! It is **not** a commitment to make every `amari-tropical` API fully generic
//! over arbitrary semirings in `0.21.0`.

use num_traits::Float;

use crate::TropicalNumber;

/// Minimal semiring interface for `amari-tropical` carriers.
///
/// The intended interpretation is:
///
/// - `zero()` = additive identity
/// - `one()` = multiplicative identity
/// - `oplus(...)` = additive combination
/// - `otimes(...)` = multiplicative composition
///
/// For the existing max-plus layer, these become:
///
/// - `zero()` = `-∞`
/// - `one()` = `0`
/// - `oplus(a, b)` = `max(a, b)`
/// - `otimes(a, b)` = `a + b`
///
/// Future ordinal-backed carriers can implement the same interface with:
///
/// - explicit bottom as `zero()`
/// - ordinal zero as `one()`
/// - `max` as `oplus(...)`
/// - ordinal addition as `otimes(...)`
pub trait Semiring: Clone + PartialEq {
    /// Additive identity.
    fn zero() -> Self;

    /// Multiplicative identity.
    fn one() -> Self;

    /// Additive semiring combination.
    fn oplus(&self, other: &Self) -> Self;

    /// Multiplicative semiring composition.
    fn otimes(&self, other: &Self) -> Self;
}

/// Fold a sequence with semiring addition.
///
/// An empty iterator returns [`Semiring::zero`].
#[inline]
pub fn fold_oplus<S, I>(values: I) -> S
where
    S: Semiring,
    I: IntoIterator<Item = S>,
{
    values
        .into_iter()
        .fold(S::zero(), |acc, value| acc.oplus(&value))
}

/// Fold a sequence with semiring multiplication.
///
/// An empty iterator returns [`Semiring::one`].
#[inline]
pub fn fold_otimes<S, I>(values: I) -> S
where
    S: Semiring,
    I: IntoIterator<Item = S>,
{
    values
        .into_iter()
        .fold(S::one(), |acc, value| acc.otimes(&value))
}

impl<T: Float> Semiring for TropicalNumber<T> {
    #[inline]
    fn zero() -> Self {
        Self::zero()
    }

    #[inline]
    fn one() -> Self {
        Self::one()
    }

    #[inline]
    fn oplus(&self, other: &Self) -> Self {
        self.tropical_add(other)
    }

    #[inline]
    fn otimes(&self, other: &Self) -> Self {
        self.tropical_mul(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tropical_number_semiring_matches_existing_operations() {
        let a = TropicalNumber::new(2.0f64);
        let b = TropicalNumber::new(5.0f64);

        assert_eq!(
            <TropicalNumber<f64> as Semiring>::zero(),
            TropicalNumber::zero()
        );
        assert_eq!(
            <TropicalNumber<f64> as Semiring>::one(),
            TropicalNumber::one()
        );
        assert_eq!(a.oplus(&b), a.tropical_add(&b));
        assert_eq!(a.otimes(&b), a.tropical_mul(&b));
    }

    #[test]
    fn fold_helpers_match_tropical_expectations() {
        let values = [
            TropicalNumber::new(1.0f64),
            TropicalNumber::new(5.0),
            TropicalNumber::new(3.0),
        ];

        let best = fold_oplus(values.iter().copied());
        let composed = fold_otimes(values.iter().copied());
        let empty_best = fold_oplus::<TropicalNumber<f64>, _>([]);
        let empty_composed = fold_otimes::<TropicalNumber<f64>, _>([]);

        assert_eq!(best.value(), 5.0);
        assert_eq!(composed.value(), 9.0);
        assert!(empty_best.is_zero());
        assert!(empty_composed.is_one());
    }
}
