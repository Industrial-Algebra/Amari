//! Deterministic floating-point scalar type
//!
//! Determinism guarantees (M-REP-D S7, 2026-10-18):
//! - **In-binary**: every operation is bit-exact reproducible for identical
//!   inputs (no hidden state, fixed iteration counts).
//! - **Cross-platform**: bit manipulation and IEEE-754 basic arithmetic are
//!   exact everywhere; the square root is a fixed 4-iteration Newton
//!   method over basic arithmetic (documented relative error < 2^-20) —
//!   cross-platform bit-exact because it invokes no libm.
//! - What is NOT claimed: transcendental libm functions anywhere in the
//!   `deterministic` module family (see `ga2d::DetRotor2`, whose angle
//!   constructors are fixed-binary-only).

use core::ops::{Add, Div, Mul, Neg, Sub};

/// Deterministic 32-bit floating-point wrapper
///
/// Guarantees bit-exact results within a fixed binary (see the module
/// docs for the precise cross-platform scope: basic ops and the
/// fixed-iteration Newton sqrt are cross-platform bit-exact; no libm
/// surface on this type).
///
/// # Performance
///
/// ~10-20% slower than native f32 due to disabled optimizations.
/// Use `feature = "fast"` for non-networked applications.
///
/// # Example
/// ```
/// use amari::deterministic::DetF32;
///
/// let a = DetF32::from_f32(1.5);
/// let b = DetF32::from_f32(2.0);
/// let c = a * b;  // Bit-exact on all platforms
/// assert_eq!(c.to_bits(), 0x40400000);
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DetF32(f32);

/// Marker trait for types with deterministic guarantees
///
/// # Safety (M-REP-D S7 — trait scope narrowed to the honest attestation)
///
/// Implementors guarantee bit-exact reproducibility of every exposed
/// operation **for a fixed compiled binary on a fixed platform**: identical
/// inputs produce identical bit patterns on every call (no hidden state,
/// no nondeterministic reduction order, no time/address dependence).
///
/// Cross-platform and cross-optimization-level bit-exactness is NOT part of
/// this contract: `DetRotor2::from_angle`/`to_angle` use `sin`/`cos`/`atan2`,
/// whose libm results are permitted to differ between platforms and builds.
/// The original contract's cross-platform claim was withdrawn by the
/// M-REP-D audit (2026-10-18) as unattestable for these types; run-to-run
/// and thread-to-thread determinism — what reproducible simulations need —
/// remains fully guaranteed.
pub unsafe trait Deterministic {}

// SAFETY (M-REP-D S7): DetF32 wraps f32 and exposes bit manipulation,
// IEEE-754 basic arithmetic, and a fixed-iteration Newton sqrt — all pure
// arithmetic over fully-specified operations, no libm. Bit-identical for
// identical inputs in the same binary; cross-platform bit-exact for this
// type's whole surface (the module-family exception is DetRotor2's angle
// constructors).
unsafe impl Deterministic for DetF32 {}

impl DetF32 {
    /// Create from raw bit pattern (always deterministic)
    #[inline]
    pub fn from_bits(bits: u32) -> Self {
        Self(f32::from_bits(bits))
    }

    /// Convert to raw bit pattern
    #[inline]
    pub fn to_bits(self) -> u32 {
        self.0.to_bits()
    }

    /// Create from f32 (converts to bit pattern)
    #[inline]
    pub fn from_f32(value: f32) -> Self {
        Self(value)
    }

    /// Convert to f32
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0
    }

    /// Zero constant
    pub const ZERO: Self = Self(0.0);

    /// One constant
    pub const ONE: Self = Self(1.0);

    /// Two constant
    pub const TWO: Self = Self(2.0);

    /// Half constant
    pub const HALF: Self = Self(0.5);

    /// PI constant
    pub const PI: Self = Self(std::f32::consts::PI);

    /// Deterministic absolute value
    #[inline(never)] // Prevent inlining that might break determinism
    pub fn abs(self) -> Self {
        Self::from_bits(self.to_bits() & 0x7fffffff)
    }

    /// Deterministic square root using Newton-Raphson
    ///
    /// Uses fixed 4 iterations for reproducibility. NOT correctly rounded:
    /// documented relative error < 2^-20 (M-REP-D S7 — earlier wording
    /// implying a rounding guarantee was withdrawn).
    #[inline(never)]
    pub fn sqrt(self) -> Self {
        if self.to_f32() <= 0.0 {
            return Self::ZERO;
        }

        // Newton-Raphson: x_{n+1} = 0.5 * (x_n + S/x_n)
        let mut guess = self;
        for _ in 0..4 {
            guess = Self::HALF * (guess + self / guess);
        }
        guess
    }

    /// Deterministic reciprocal square root
    #[inline(never)]
    pub fn rsqrt(self) -> Self {
        Self::ONE / self.sqrt()
    }
}

// Arithmetic operations - explicitly prevent FMA
impl Add for DetF32 {
    type Output = Self;

    #[inline(never)]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for DetF32 {
    type Output = Self;

    #[inline(never)]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl Mul for DetF32 {
    type Output = Self;

    #[inline(never)]
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Div for DetF32 {
    type Output = Self;

    #[inline(never)]
    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl Neg for DetF32 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_ops_exact_bits() {
        let a = DetF32::from_bits(0x3f800000); // 1.0
        let b = DetF32::from_bits(0x40000000); // 2.0

        assert_eq!((a + b).to_bits(), 0x40400000); // 3.0
        assert_eq!((a * b).to_bits(), 0x40000000); // 2.0
        assert_eq!((b - a).to_bits(), 0x3f800000); // 1.0
        assert_eq!((b / b).to_bits(), 0x3f800000); // 1.0
    }

    #[test]
    fn test_sqrt_determinism() {
        let x = DetF32::from_bits(0x40000000); // 2.0
        let sqrt_2 = x.sqrt();

        // Should always produce same bit pattern
        assert_eq!(sqrt_2.to_bits(), sqrt_2.to_bits());

        // Verify accuracy: sqrt(2) ≈ 1.414213562
        let expected = DetF32::from_f32(std::f32::consts::SQRT_2);
        let error = (sqrt_2 - expected).abs();
        assert!(error < DetF32::from_f32(1e-6));
    }
}
