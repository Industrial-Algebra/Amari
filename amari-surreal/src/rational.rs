use crate::dyadic::Dyadic;
use crate::error::SurrealError;
use crate::short::ShortSurreal;
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Neg, Sub};

/// Exact rational number backed by `num_rational::BigRational`.
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
    #[must_use]
    pub fn to_short_if_dyadic(&self) -> Option<ShortSurreal> {
        let denom = self.value.denom();
        let mut d = denom.clone();
        let mut exponent: u32 = 0;
        while d.is_even() {
            d >>= 1;
            exponent += 1;
        }
        if !d.is_one() {
            return None;
        }
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
