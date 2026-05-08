use crate::error::{Result, SurrealError};
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
    #[must_use]
    pub fn new<N>(numer: N, exponent: u32) -> Self
    where
        N: Into<BigInt>,
    {
        Self {
            numer: numer.into(),
            exponent,
        }
        .normalize()
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
    #[must_use]
    pub fn normalize(mut self) -> Self {
        if self.numer.is_zero() {
            self.exponent = 0;
            return self;
        }

        while self.exponent > 0 && self.numer.is_even() {
            self.numer /= 2u8;
            self.exponent -= 1;
        }

        self
    }

    /// Returns a checked reciprocal within the dyadic short-surreal layer.
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

        let exponent =
            Self::power_of_two_exponent(&denom).ok_or(SurrealError::NonDyadicQuotient)?;
        Ok(Self::new(numer, exponent))
    }

    fn power_of_two_exponent(value: &BigInt) -> Option<u32> {
        if value.is_zero() || value.is_negative() {
            return None;
        }

        if *value == BigInt::one() {
            return Some(0);
        }

        let mut value = value.clone();
        let mut exponent = 0;
        while value.is_even() {
            value /= 2u8;
            exponent += 1;
        }

        (value == BigInt::one()).then_some(exponent)
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

impl Mul for Dyadic {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let numer = self.numer * rhs.numer;
        let exponent = self.exponent.saturating_add(rhs.exponent);
        Self::new(numer, exponent)
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
