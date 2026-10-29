use crate::dyadic::Dyadic;
use crate::error::SurrealError;
use crate::short::ShortSurreal;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Neg, Sub};

/// Exact rational number backed by `num_rational::BigRational`.
///
/// NOTE: this is an exact rational SCALAR, not itself a surreal number
/// of the layer this crate implements (short surreals are dyadic;
/// `1/3` is not a short surreal). It exists as the coefficient field
/// for the surcomplex layer and the epsilon layer.
///
/// `RationalSurreal` provides an exact rational scalar field for the
/// surcomplex layer (v0.23+). It augments the existing dyadic
/// [`ShortSurreal`] layer with true rational division, supporting
/// denominators that are not powers of two.
#[derive(Debug, Clone)]
pub struct RationalSurreal {
    value: BigRational,
}

impl RationalSurreal {
    /// Returns zero.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            value: BigRational::from_integer(BigInt::from(0)),
        }
    }

    /// Returns one.
    #[must_use]
    pub fn one() -> Self {
        Self {
            value: BigRational::from_integer(BigInt::from(1)),
        }
    }

    /// Creates a rational surreal from an integer.
    #[must_use]
    pub fn from_integer<N: Into<BigInt>>(n: N) -> Self {
        Self {
            value: BigRational::from_integer(n.into()),
        }
    }

    /// Creates a rational surreal from a numerator and denominator.
    ///
    /// Returns `SurrealError::DivisionByZero` when the denominator is zero.
    pub fn from_ratio<N: Into<BigInt>, D: Into<BigInt>>(
        numer: N,
        denom: D,
    ) -> Result<Self, SurrealError> {
        let denom: BigInt = denom.into();
        if denom.is_zero() {
            return Err(SurrealError::DivisionByZero);
        }
        let value = BigRational::new(numer.into(), denom);
        Ok(Self { value })
    }

    /// Converts a [`ShortSurreal`] into a rational surreal.
    #[must_use]
    pub fn from_short(value: ShortSurreal) -> Self {
        let dyadic = value.to_dyadic();
        let numer = dyadic.numer().clone();
        let denom = BigInt::from(1) << dyadic.exponent();
        Self {
            value: BigRational::new(numer, denom),
        }
    }

    /// Returns the normalized numerator.
    #[must_use]
    pub fn numer(&self) -> &BigInt {
        self.value.numer()
    }

    /// Returns the normalized denominator.
    #[must_use]
    pub fn denom(&self) -> &BigInt {
        self.value.denom()
    }

    /// Returns whether the rational is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.value.numer().is_zero()
    }

    /// Returns whether the rational is positive.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.value.numer().is_positive()
    }

    /// Returns whether the rational is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.value.numer().is_negative()
    }

    /// Returns the absolute value of the rational.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self {
            value: self.value.abs(),
        }
    }

    /// Returns a checked reciprocal.
    ///
    /// Returns `SurrealError::DivisionByZero` when the value is zero.
    pub fn checked_reciprocal(&self) -> Result<Self, SurrealError> {
        if self.value.numer().is_zero() {
            return Err(SurrealError::DivisionByZero);
        }
        Ok(Self {
            value: self.value.recip(),
        })
    }

    /// Returns `self / rhs`.
    ///
    /// Returns `SurrealError::DivisionByZero` when `rhs` is zero.
    pub fn checked_div(&self, rhs: &Self) -> Result<Self, SurrealError> {
        if rhs.value.numer().is_zero() {
            return Err(SurrealError::DivisionByZero);
        }
        Ok(Self {
            value: &self.value / &rhs.value,
        })
    }

    /// Converts to a [`ShortSurreal`] when the value is dyadic.
    ///
    /// Returns `Some` only when the normalized denominator is a power
    /// of two (dyadic rational); returns `None` for non-dyadic values.
    ///
    /// `BigRational` always stores the value in normalised form with
    /// a positive denominator, so we only need to check whether that
    /// denominator is a power of two.  The loop strips factors of two
    /// and, if the residual is one, the denominator is a power of two.
    #[must_use]
    pub fn to_short_if_dyadic(&self) -> Option<ShortSurreal> {
        let denom = self.value.denom();
        // Denominator is a power of two iff exactly one bit set; the
        // exponent is the trailing-zero count (O(bits), not one shift
        // per power of two).
        let Some(tz) = denom.trailing_zeros() else {
            return None; // zero denominator cannot occur in BigRational
        };
        if (denom >> tz) != BigInt::from(1) {
            return None;
        }
        // Degrade, never panic: a dyadic whose exponent exceeds the
        // bounded dyadic layer is not representable there. Compare at
        // FULL WIDTH first — `tz as u32` would wrap for 2^4294967296
        // and corrupt the value (R1: returned Some(1)).
        if tz > u64::from(crate::dyadic::MAX_EXPONENT) {
            return None;
        }
        let exponent = tz as u32;
        Some(ShortSurreal::from_dyadic(Dyadic::new(
            self.value.numer().clone(),
            exponent,
        )))
    }
}

impl PartialEq for RationalSurreal {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for RationalSurreal {}

impl Hash for RationalSurreal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl PartialOrd for RationalSurreal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RationalSurreal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl Add for RationalSurreal {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value + rhs.value,
        }
    }
}

impl Sub for RationalSurreal {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value - rhs.value,
        }
    }
}

impl Mul for RationalSurreal {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value * rhs.value,
        }
    }
}

impl Neg for RationalSurreal {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self { value: -self.value }
    }
}

impl fmt::Display for RationalSurreal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r<N: Into<BigInt>, D: Into<BigInt>>(n: N, d: D) -> RationalSurreal {
        RationalSurreal::from_ratio(n, d).unwrap()
    }

    #[test]
    fn zero_denominator_rejected() {
        assert!(matches!(
            RationalSurreal::from_ratio(1, 0).unwrap_err(),
            SurrealError::DivisionByZero
        ));
    }

    #[test]
    fn to_short_if_dyadic_both_ways() {
        assert_eq!(
            r(1, 2).to_short_if_dyadic().map(|s| s.to_dyadic()),
            Some(Dyadic::new(1, 1))
        );
        assert_eq!(
            r(3, 4).to_short_if_dyadic().map(|s| s.to_dyadic()),
            Some(Dyadic::new(3, 2))
        );
        assert!(r(1, 3).to_short_if_dyadic().is_none());
        assert!(r(2, 6)
            .to_short_if_dyadic()
            .map(|s| s.to_dyadic())
            .is_none());
    }

    #[test]
    fn to_short_degrades_at_the_bound() {
        // Denominator 2^(MAX+1): dyadic but not representable.
        let denom = BigInt::from(1) << (crate::dyadic::MAX_EXPONENT as usize + 1);
        let over = RationalSurreal::from_ratio(1, denom).unwrap();
        assert!(over.to_short_if_dyadic().is_none());
        // Denominator exactly 2^MAX: representable at the boundary.
        let at_bound =
            RationalSurreal::from_ratio(1, BigInt::from(1) << crate::dyadic::MAX_EXPONENT as usize)
                .unwrap();
        assert!(at_bound.to_short_if_dyadic().is_some());
    }

    #[test]
    fn full_width_exponent_degrades_to_none() {
        // R1 P1-3 repro: from_ratio(1, 2^4294967296) previously
        // returned Some(1) via u32 narrowing.
        let huge = BigInt::from(1) << 4_294_967_296usize;
        let r = RationalSurreal::from_ratio(1, huge).unwrap();
        assert!(r.to_short_if_dyadic().is_none());
    }

    #[test]
    fn arithmetic_posture_is_exact_and_total() {
        // Arbitrary-precision: no wrap by representation.
        let big = r(i64::MAX, 1);
        let sum = big.clone() + big.clone();
        assert_eq!(sum, r(2 * (i64::MAX as i128), 1).abs());
        // Comparisons total on extreme values.
        assert!(r(i64::MIN, 1) < r(1, i64::MAX));
        assert!(r(-1, i64::MAX) < r(0, 1));
        // Division errors only on zero.
        assert!(r(1, 3).checked_div(&r(0, 1)).is_err());
        assert!(r(1, 3).checked_div(&r(1, 3)).unwrap() == r(1, 1));
    }
}
