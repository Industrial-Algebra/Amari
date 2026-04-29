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
}
