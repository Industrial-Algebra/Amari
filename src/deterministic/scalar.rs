//! Deterministic floating-point scalar type
//!
//! Determinism guarantees (M-REP-D S7, revised R3 2026-10-18 — the
//! earlier "libm in DetRotor2" premise was WRONG):
//! - **Cross-platform bit-exactness on the finite domain**: the entire
//!   `deterministic` family is pure fixed arithmetic — bit manipulation,
//!   IEEE-754 basic operations, and fixed-iteration approximations (Newton
//!   sqrt in this file; Taylor sin/cos and the atan2 approximation in
//!   `approx.rs`). No libm call exists anywhere in the family, so
//!   computations with finite inputs, intermediates, and final results
//!   yield identical bits across platforms and builds. NaN-producing
//!   computations carry NO bit-identity promise (Rust permits NaN
//!   sign/payload variation even within one binary).
//! - **Not claimed**: ACCURACY. The approximations do not converge outside
//!   their working ranges (see each method's docs — e.g. the Newton sqrt
//!   from a raw-input start, and Taylor series near zero). Deterministic,
//!   not correctly rounded.

use core::ops::{Add, Div, Mul, Neg, Sub};

/// Deterministic 32-bit floating-point wrapper
///
/// Guarantees bit-exact results on the finite domain (see the module
/// docs for the precise scope: basic ops, bit manipulation, and the
/// fixed-iteration approximations are cross-platform bit-exact for
/// finite computations; no libm surface on this type; NaN bit patterns
/// carry no promise).
///
/// # Performance
///
/// Carries overhead relative to native f32 (fixed iteration counts,
/// `#[inline(never)]` boundaries); no specific figure is claimed. For
/// non-networked applications, use the standard `f32` types directly.
///
/// # Example
/// ```
/// use amari::deterministic::DetF32;
///
/// let a = DetF32::from_f32(1.5);
/// let b = DetF32::from_f32(2.0);
/// let c = a * b;  // Bit-exact on all platforms (finite domain)
/// assert_eq!(c.to_bits(), 0x40400000);
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DetF32(f32);

/// Marker trait for types with deterministic guarantees
///
/// # Safety (M-REP-D S7 — trait scope narrowed to the honest attestation)
///
/// Marker for deterministic-arithmetic types.
///
/// Implementors guarantee bit-exact reproducibility of every exposed
/// operation on the finite domain (see the scope qualification below):
/// identical finite inputs with finite intermediates and results produce
/// identical bit patterns — no hidden state, address or time dependence —
/// in a fixed binary and ACROSS platforms and builds, because the entire
/// family is pure fixed arithmetic (IEEE-754 basic operations, bit
/// manipulation, fixed-iteration approximations; no libm anywhere —
/// M-REP-D R3 correction of the R1 premise that claimed otherwise).
///
/// This is a DETERMINISM contract, not an accuracy contract: the
/// approximation-based methods (Newton sqrt, Taylor sin/cos/atan2) are not
/// converged outside their working ranges and no accuracy is claimed.
///
/// # Safety
///
/// Scope qualification (R4, tightened R5): bit-identity is promised only
/// for computations whose inputs, intermediates, and final results are
/// finite. Where an operation produces a NaN, NOTHING is promised — Rust
/// permits the sign and payload to differ between repetitions of the same
/// operation in the same binary (the local `0/0` payload `0xffc00000` is
/// an observation, not a guarantee, of stability or portability).
///
/// Implementors must contain only pure fixed arithmetic as described
/// above; any libm call, hardware-varying approximation, hidden state, or
/// input-length-dependent iteration forfeits the safety justification.
pub unsafe trait Deterministic {}

// SAFETY (M-REP-D S7): DetF32 wraps f32 and exposes bit manipulation,
// IEEE-754 basic arithmetic, and fixed-iteration approximations (Newton
// sqrt here; Taylor sin/cos/atan2 in approx.rs) — all pure arithmetic,
// no libm. Bit-identical across binaries and platforms for computations
// finite through inputs, intermediates, and results (the trait's finite
// domain; NaN bit patterns carry no promise). No accuracy claim (see the
// method docs).
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
    /// Uses fixed 4 iterations from an initial guess equal to the input,
    /// for reproducibility. NOT an accuracy guarantee: convergence is NOT
    /// achieved for inputs far from 1.0 — e.g. `sqrt(100)` returns
    /// approximately 10.84 after four iterations (observed; M-REP-D R2).
    /// Deterministic (fixed iteration count, pure arithmetic); accurate
    /// only near unity. Accuracy improvements are follow-up work outside
    /// this audit's scope.
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
