use crate::error::{Result, SurrealError};

/// Maximum representable exponent (2^20) — the representation invariant
/// of the dyadic layer. Stored dyadics must satisfy `exponent <=
/// MAX_EXPONENT`; comparisons and transient arithmetic are unbounded.
/// The bound kills exponent wrap, saturation, and the 2^e-bit allocation
/// amplification in one invariant.
pub const MAX_EXPONENT: u32 = 1 << 20;
use amari_cgt::Birthday;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

/// Exact dyadic rational `numer / 2^exponent`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dyadic {
    numer: BigInt,
    exponent: u32,
}

impl Dyadic {
    /// Creates a new dyadic rational and normalizes it.
    ///
    /// # Panics
    ///
    /// Panics when `exponent > MAX_EXPONENT` (the representation bound).
    /// Use [`Dyadic::try_new`] for the fallible path.
    #[must_use]
    pub fn new<N>(numer: N, exponent: u32) -> Self
    where
        N: Into<BigInt>,
    {
        Self::try_new(numer, exponent).expect("exponent exceeds MAX_EXPONENT")
    }

    /// Creates a new dyadic rational, enforcing the representation bound.
    ///
    /// Returns [`SurrealError::ExponentOverflow`] when
    /// `exponent > MAX_EXPONENT` (checked BEFORE normalization, so the
    /// bound holds for the normalized form too: normalization only
    /// decreases the exponent).
    pub fn try_new<N>(numer: N, exponent: u32) -> Result<Self>
    where
        N: Into<BigInt>,
    {
        if exponent > MAX_EXPONENT {
            return Err(SurrealError::ExponentOverflow {
                requested: exponent,
                bound: MAX_EXPONENT,
            });
        }
        Ok(Self {
            numer: numer.into(),
            exponent,
        }
        .normalize())
    }

    /// Creates a dyadic integer.
    #[must_use]
    pub fn from_integer<N>(value: N) -> Self
    where
        N: Into<BigInt>,
    {
        Self::new(value, 0)
    }

    /// Returns zero.
    #[must_use]
    pub fn zero() -> Self {
        Self::from_integer(0)
    }

    /// Returns one.
    #[must_use]
    pub fn one() -> Self {
        Self::from_integer(1)
    }

    /// Returns the numerator.
    #[must_use]
    pub fn numer(&self) -> &BigInt {
        &self.numer
    }

    /// Returns the exponent of the power-of-two denominator.
    #[must_use]
    pub fn exponent(&self) -> u32 {
        self.exponent
    }

    /// Returns whether the dyadic is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.numer.is_zero()
    }

    /// Returns whether the dyadic is positive.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.numer.is_positive()
    }

    /// Returns whether the dyadic is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.numer.is_negative()
    }

    /// Returns the absolute value of the dyadic.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self::new(self.numer.abs(), self.exponent)
    }

    /// Returns a normalized version of the dyadic.
    ///
    /// Strips the common power of two in one trailing-zeros step
    /// (O(bits), not O(2^exponent) halvings).
    #[must_use]
    pub fn normalize(mut self) -> Self {
        if self.numer.is_zero() {
            self.exponent = 0;
            return self;
        }

        if let Some(tz) = self.numer.trailing_zeros() {
            let tz = tz as usize;
            let strip = tz.min(self.exponent as usize);
            if strip > 0 {
                self.numer >>= strip;
                self.exponent -= strip as u32;
            }
        }

        self
    }

    /// Returns a checked reciprocal within the dyadic short-surreal layer.
    ///
    /// Never panics: the reduction routes through [`Dyadic::try_new`],
    /// so an over-bound result (impossible from a bounded input, but
    /// enforced regardless) surfaces as `Err`.
    pub fn checked_reciprocal(&self) -> Result<Self> {
        if self.numer.is_zero() {
            return Err(SurrealError::DivisionByZero);
        }

        let numer = BigInt::one() << (self.exponent as usize);
        Self::from_ratio(numer, self.numer.clone())
    }

    /// Returns `self / rhs` when the quotient remains dyadic.
    pub fn checked_div(&self, rhs: &Self) -> Result<Self> {
        if rhs.numer.is_zero() {
            return Err(SurrealError::DivisionByZero);
        }

        let numer = self.numer.clone() << (rhs.exponent as usize);
        let denom = rhs.numer.clone() << (self.exponent as usize);
        Self::from_ratio(numer, denom)
    }

    /// Returns the floor as an integer.
    #[must_use]
    pub fn floor_integer(&self) -> BigInt {
        self.numer.div_floor(&(BigInt::one() << self.exponent))
    }

    /// Returns the ceiling as an integer.
    #[must_use]
    pub fn ceil_integer(&self) -> BigInt {
        self.numer.div_ceil(&(BigInt::one() << self.exponent))
    }

    /// Estimates the short surreal birthday of this dyadic.
    ///
    /// Saturates at `u32::MAX` for dyadics with magnitude beyond
    /// `u32::MAX` (unreachable under normal use; the `Birthday` type is
    /// a `u32` newtype in `amari-cgt`).
    #[must_use]
    pub fn short_birthday(&self) -> Birthday {
        if self.numer.is_zero() {
            return Birthday(0);
        }

        let denominator = BigInt::one() << self.exponent;
        let abs = self.numer.abs();
        let ceil = abs.div_ceil(&denominator);
        let ceil_u32 = ceil
            .to_u32()
            .unwrap_or(u32::MAX.saturating_sub(self.exponent));
        Birthday(ceil_u32.saturating_add(self.exponent))
    }

    fn scaled_numer(&self, exponent: u32) -> BigInt {
        self.numer.clone() << ((exponent - self.exponent) as usize)
    }

    fn from_ratio(mut numer: BigInt, mut denom: BigInt) -> Result<Self> {
        if denom.is_zero() {
            return Err(SurrealError::DivisionByZero);
        }

        if denom.is_negative() {
            numer = -numer;
            denom = -denom;
        }

        let gcd = numer.gcd(&denom);
        if !gcd.is_zero() {
            numer /= &gcd;
            denom /= gcd;
        }

        let exponent_wide =
            Self::power_of_two_exponent_wide(&denom).ok_or(SurrealError::NonDyadicQuotient)?;
        if exponent_wide > u64::from(MAX_EXPONENT) {
            return Err(SurrealError::ExponentOverflow {
                requested: u32::MAX, // full-width exponent; saturating report
                bound: MAX_EXPONENT,
            });
        }
        Self::try_new(numer, exponent_wide as u32)
    }

    /// Power-of-two exponent as FULL-WIDTH u64 (no u32 narrowing: a
    /// denominator 2^4294967296 must not corrupt values by wrapping).
    fn power_of_two_exponent_wide(value: &BigInt) -> Option<u64> {
        if value.is_zero() || value.is_negative() {
            return None;
        }

        if *value == BigInt::one() {
            return Some(0);
        }

        // Power of two iff exactly one bit set (any exponent size).
        match value.trailing_zeros() {
            Some(tz) => {
                let shifted = value >> tz;
                (shifted == BigInt::one()).then_some(tz)
            }
            None => None,
        }
    }
}

impl PartialOrd for Dyadic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dyadic {
    fn cmp(&self, other: &Self) -> Ordering {
        let exponent = self.exponent.max(other.exponent);
        self.scaled_numer(exponent)
            .cmp(&other.scaled_numer(exponent))
    }
}

impl Add for Dyadic {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let exponent = self.exponent.max(rhs.exponent);
        let numer = self.scaled_numer(exponent) + rhs.scaled_numer(exponent);
        Self::new(numer, exponent)
    }
}

impl Sub for Dyadic {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl Dyadic {
    /// Checked multiplication: returns
    /// [`SurrealError::ExponentOverflow`] when the result exponent would
    /// exceed `MAX_EXPONENT` (the growth path — `Add` takes the max of
    /// bounded exponents and cannot grow).
    pub fn checked_mul(&self, rhs: &Self) -> Result<Self> {
        let exponent = self.exponent + rhs.exponent;
        if exponent > MAX_EXPONENT {
            return Err(SurrealError::ExponentOverflow {
                requested: exponent,
                bound: MAX_EXPONENT,
            });
        }
        let numer = self.numer.clone() * rhs.numer.clone();
        Ok(Self::new(numer, exponent))
    }
}

impl Mul for Dyadic {
    type Output = Self;

    /// # Panics
    ///
    /// Panics when the result exponent exceeds `MAX_EXPONENT`
    /// (documented honest panic; see [`Dyadic::checked_mul`]).
    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(&rhs)
            .expect("exponent exceeds MAX_EXPONENT")
    }
}

impl Neg for Dyadic {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            numer: -self.numer,
            exponent: self.exponent,
        }
    }
}

impl fmt::Display for Dyadic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.exponent == 0 {
            write!(f, "{}", self.numer)
        } else {
            let denom = BigInt::one() << self.exponent;
            write!(f, "{}/{}", self.numer, denom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(n: i64, e: u32) -> Dyadic {
        Dyadic::new(n, e)
    }

    #[test]
    fn normalization_strips_powers_of_two() {
        assert_eq!(d(4, 3), d(1, 1));
        assert_eq!(d(6, 2), d(3, 1));
        assert_eq!(d(0, 7), d(0, 0));
        assert_eq!(d(-8, 5), d(-1, 2));
    }

    #[test]
    fn arithmetic_is_exact() {
        assert_eq!(d(1, 1) + d(1, 2), d(3, 2));
        assert_eq!(d(1, 1) - d(1, 2), d(1, 2));
        assert_eq!(d(1, 1) * d(1, 1), d(1, 2));
        assert_eq!(-d(1, 1), d(-1, 1));
    }

    #[test]
    fn ordering_across_scales() {
        assert!(d(-1, 1) < d(0, 0));
        assert!(d(1, 2) < d(1, 1));
        assert!(d(5, 2) > d(2, 1)); // 5/4 > 1
        assert_eq!(d(2, 2), d(1, 1));
    }

    #[test]
    fn checked_div_reports_non_dyadic() {
        let err = d(1, 0).checked_div(&d(3, 0)).unwrap_err();
        assert!(matches!(err, SurrealError::NonDyadicQuotient));
        assert_eq!(d(1, 1).checked_div(&d(1, 1)).unwrap(), d(1, 0));
        // Division BY zero errors; zero divided by one is zero.
        assert!(matches!(
            d(1, 0).checked_div(&d(0, 0)).unwrap_err(),
            SurrealError::DivisionByZero
        ));
        assert_eq!(d(0, 0).checked_div(&d(1, 0)).unwrap(), d(0, 0));
        // Reciprocal of 3 is 1/3: non-dyadic, not division-by-zero.
        assert!(matches!(
            d(3, 0).checked_reciprocal().unwrap_err(),
            SurrealError::NonDyadicQuotient
        ));
        assert!(matches!(
            d(0, 0).checked_reciprocal().unwrap_err(),
            SurrealError::DivisionByZero
        ));
    }

    #[test]
    fn birthday_table() {
        // Known short-surreal birthdays.
        assert_eq!(d(0, 0).short_birthday(), Birthday(0));
        assert_eq!(d(1, 0).short_birthday(), Birthday(1));
        assert_eq!(d(-1, 0).short_birthday(), Birthday(1));
        assert_eq!(d(1, 1).short_birthday(), Birthday(2));
        assert_eq!(d(3, 2).short_birthday(), Birthday(3));
        assert_eq!(d(5, 3).short_birthday(), Birthday(4)); // 5/8
        assert_eq!(d(3, 1).short_birthday(), Birthday(3));
        assert_eq!(d(5, 0).short_birthday(), Birthday(5));
        assert_eq!(d(-3, 2).short_birthday(), Birthday(3));
    }

    #[test]
    fn exponent_bound_enforced_fallibly() {
        let over = Dyadic::try_new(1, MAX_EXPONENT + 1).unwrap_err();
        assert!(matches!(
            over,
            SurrealError::ExponentOverflow {
                requested,
                bound
            } if requested == MAX_EXPONENT + 1 && bound == MAX_EXPONENT
        ));
        // At the bound is fine; normalization may reduce it.
        assert_eq!(
            Dyadic::try_new(1, MAX_EXPONENT).unwrap(),
            d(1, MAX_EXPONENT)
        );
    }

    #[test]
    fn checked_mul_crossing_the_bound_errors() {
        let at_bound = d(1, MAX_EXPONENT);
        let err = at_bound.checked_mul(&at_bound.clone()).unwrap_err();
        assert!(matches!(err, SurrealError::ExponentOverflow { .. }));
        // One below double-crosses only at the pair's sum.
        assert_eq!(d(1, 10).checked_mul(&d(1, 10)).unwrap(), d(1, 20));
    }

    #[test]
    fn full_width_exponents_never_corrupt() {
        // R1 P1-3 repro: denominator 2^4294967296 previously NARROWED
        // (tz as u32 wrapped to 0) and returned 1 instead of erroring.
        let huge = BigInt::from(1) << 4_294_967_296usize;
        // from_integer(2^2^32).checked_reciprocal() must NOT be Ok(1).
        let d = Dyadic::from_integer(huge.clone());
        assert!(matches!(
            d.checked_reciprocal(),
            Err(SurrealError::ExponentOverflow { .. })
        ));
    }

    #[test]
    #[should_panic(expected = "exceeds MAX_EXPONENT")]
    fn infallible_mul_panics_honestly_at_the_bound() {
        let at_bound = d(1, MAX_EXPONENT);
        let _ = at_bound.clone() * at_bound;
    }
}
