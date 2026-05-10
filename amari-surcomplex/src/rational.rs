use crate::error::{Result, SurcomplexError};
use amari_surreal::RationalSurreal;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Neg, Sub};

/// Exact rational surcomplex number `a + bi` with `a, b ∈ Q`.
///
/// `RationalSurcomplex` pairs two [`RationalSurreal`] values to form an
/// exact complex number over the rationals.  It supports addition,
/// subtraction, multiplication, conjugation, norm, and division.
#[derive(Debug, Clone)]
pub struct RationalSurcomplex {
    real: RationalSurreal,
    imag: RationalSurreal,
}

impl RationalSurcomplex {
    /// Returns zero (`0 + 0i`).
    #[must_use]
    pub fn zero() -> Self {
        Self {
            real: RationalSurreal::zero(),
            imag: RationalSurreal::zero(),
        }
    }

    /// Returns one (`1 + 0i`).
    #[must_use]
    pub fn one() -> Self {
        Self {
            real: RationalSurreal::one(),
            imag: RationalSurreal::zero(),
        }
    }

    /// Returns the imaginary unit `i` (`0 + 1i`).
    #[must_use]
    pub fn i() -> Self {
        Self {
            real: RationalSurreal::zero(),
            imag: RationalSurreal::one(),
        }
    }

    /// Creates a real surcomplex from an integer.
    #[must_use]
    pub fn from_integer<N: Into<i128>>(n: N) -> Self {
        Self {
            real: RationalSurreal::from_integer(n.into()),
            imag: RationalSurreal::zero(),
        }
    }

    /// Creates a real surcomplex from a [`RationalSurreal`].
    #[must_use]
    pub fn from_real(real: RationalSurreal) -> Self {
        Self {
            real,
            imag: RationalSurreal::zero(),
        }
    }

    /// Creates a surcomplex from real and imaginary parts.
    #[must_use]
    pub fn from_parts(real: RationalSurreal, imag: RationalSurreal) -> Self {
        Self { real, imag }
    }

    /// Returns a reference to the real part.
    #[must_use]
    pub fn real(&self) -> &RationalSurreal {
        &self.real
    }

    /// Returns a reference to the imaginary part.
    #[must_use]
    pub fn imag(&self) -> &RationalSurreal {
        &self.imag
    }

    /// Returns the complex conjugate `a - bi`.
    #[must_use]
    pub fn conjugate(&self) -> Self {
        Self {
            real: self.real.clone(),
            imag: -self.imag.clone(),
        }
    }

    /// Returns the squared norm `a² + b²`.
    #[must_use]
    pub fn norm_sq(&self) -> RationalSurreal {
        self.real.clone() * self.real.clone() + self.imag.clone() * self.imag.clone()
    }

    /// Returns whether the surcomplex is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.real.is_zero() && self.imag.is_zero()
    }

    /// Returns a checked reciprocal (`1 / self`).
    ///
    /// Returns `SurcomplexError::DivisionByZero` when the value is zero.
    ///
    /// Uses the formula:
    /// ```text
    /// 1 / (a + bi) = (a - bi) / (a² + b²) = conjugate / norm_sq
    /// ```
    pub fn checked_reciprocal(&self) -> Result<Self> {
        let norm = self.norm_sq();
        if norm.is_zero() {
            return Err(SurcomplexError::DivisionByZero);
        }
        let inv_norm = norm.checked_reciprocal()?;
        Ok(Self {
            real: self.real.clone() * inv_norm.clone(),
            imag: -self.imag.clone() * inv_norm,
        })
    }

    /// Returns `self / rhs`.
    ///
    /// Returns `SurcomplexError::DivisionByZero` when `rhs` is zero.
    ///
    /// Uses the formula:
    /// ```text
    /// (a + bi) / (c + di) = ((a + bi)(c - di)) / (c² + d²)
    /// ```
    pub fn checked_div(&self, rhs: &Self) -> Result<Self> {
        let denom_norm = rhs.norm_sq();
        if denom_norm.is_zero() {
            return Err(SurcomplexError::DivisionByZero);
        }
        // (a + bi)(c - di) = (ac + bd) + (bc - ad)i
        let ac = self.real.clone() * rhs.real.clone();
        let bd = self.imag.clone() * rhs.imag.clone();
        let bc = self.imag.clone() * rhs.real.clone();
        let ad = self.real.clone() * rhs.imag.clone();
        let inv_denom = denom_norm.checked_reciprocal()?;
        Ok(Self {
            real: (ac + bd) * inv_denom.clone(),
            imag: (bc - ad) * inv_denom,
        })
    }
}

impl PartialEq for RationalSurcomplex {
    fn eq(&self, other: &Self) -> bool {
        self.real == other.real && self.imag == other.imag
    }
}

impl Eq for RationalSurcomplex {}

impl Hash for RationalSurcomplex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.real.hash(state);
        self.imag.hash(state);
    }
}

impl fmt::Display for RationalSurcomplex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.real.is_zero(), self.imag.is_zero()) {
            (true, true) => write!(f, "0"),
            (false, true) => write!(f, "{}", self.real),
            (true, false) => write!(f, "{}i", self.imag),
            (false, false) => {
                if self.imag.is_negative() {
                    write!(f, "{} - {}i", self.real, self.imag.abs())
                } else {
                    write!(f, "{} + {}i", self.real, self.imag)
                }
            }
        }
    }
}

impl Add for RationalSurcomplex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real + rhs.real,
            imag: self.imag + rhs.imag,
        }
    }
}

impl Sub for RationalSurcomplex {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real - rhs.real,
            imag: self.imag - rhs.imag,
        }
    }
}

impl Mul for RationalSurcomplex {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        // (a + bi)(c + di) = (ac - bd) + (ad + bc)i
        let ac = self.real.clone() * rhs.real.clone();
        let bd = self.imag.clone() * rhs.imag.clone();
        let ad = self.real * rhs.imag;
        let bc = self.imag * rhs.real;
        Self {
            real: ac - bd,
            imag: ad + bc,
        }
    }
}

impl Neg for RationalSurcomplex {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            real: -self.real,
            imag: -self.imag,
        }
    }
}
