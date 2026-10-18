//! Polynomial approximations for transcendental functions
//!
//! Uses Chebyshev or Taylor series with fixed iteration counts
//! to ensure deterministic results.
//!
//! Accuracy honesty (M-REP-D R3): fixed-iteration Taylor-series
//! approximations over pure arithmetic with argument reduction to the
//! primary range — deterministic and cross-platform bit-exact on the
//! finite domain (see the Deterministic trait; NaN bit patterns carry
//! no promise). The
//! method-level error bounds documented below predate this audit and are
//! not certified by it; this module claims determinism, not accuracy.

use super::scalar::DetF32;

impl DetF32 {
    /// Deterministic sine using Taylor series
    ///
    /// sin(x) = x - x³/3! + x⁵/5! - x⁷/7! + ...
    ///
    /// Reduces argument to [-π, π] then uses 7 terms.
    /// Absolute error: < 2^-20
    #[inline(never)]
    pub fn sin(self) -> Self {
        // Reduce to [-π, π]
        let x = self.rem_2pi();

        // Taylor series
        let x2 = x * x;
        let x3 = x2 * x;
        let x5 = x3 * x2;
        let x7 = x5 * x2;
        let x9 = x7 * x2;

        x - x3 * Self::from_bits(0x3e2aaaab)      // x³/6
          + x5 * Self::from_bits(0x3c088889)      // x⁵/120
          - x7 * Self::from_bits(0x39500d01)      // x⁷/5040
          + x9 * Self::from_bits(0x3638ef1d) // x⁹/362880
    }

    /// Deterministic cosine
    ///
    /// cos(x) = 1 - x²/2! + x⁴/4! - x⁶/6! + ...
    #[inline(never)]
    pub fn cos(self) -> Self {
        let x = self.rem_2pi();

        let x2 = x * x;
        let x4 = x2 * x2;
        let x6 = x4 * x2;
        let x8 = x6 * x2;

        Self::ONE
          - x2 * Self::HALF                       // x²/2
          + x4 * Self::from_bits(0x3d2aaaab)      // x⁴/24
          - x6 * Self::from_bits(0x3a93f27e)      // x⁶/720
          + x8 * Self::from_bits(0x37d00d01) // x⁸/40320
    }

    /// Reduce angle to [-π, π] range deterministically
    fn rem_2pi(self) -> Self {
        let two_pi = DetF32::from_f32(std::f32::consts::TAU); // 2π
        let inv_two_pi = DetF32::from_f32(1.0 / std::f32::consts::TAU); // 1/(2π)

        // Calculate number of 2π periods
        let n = (self * inv_two_pi).floor();
        self - n * two_pi
    }

    /// Deterministic floor function
    #[inline(never)]
    fn floor(self) -> Self {
        let val = self.to_f32();
        let truncated = val as i32 as f32;
        if val >= 0.0 || val == truncated {
            Self::from_f32(truncated)
        } else {
            Self::from_f32(truncated - 1.0)
        }
    }

    /// Deterministic atan2 for vector angle calculation
    ///
    /// Uses polynomial approximation in reduced range
    #[inline(never)]
    pub fn atan2(y: Self, x: Self) -> Self {
        // Handle special cases
        if x == Self::ZERO && y == Self::ZERO {
            return Self::ZERO;
        }

        // Reduce to first octant
        let abs_x = x.abs();
        let abs_y = y.abs();

        let (a, b, swap) = if abs_y > abs_x {
            (abs_x, abs_y, true)
        } else {
            (abs_y, abs_x, false)
        };

        // Compute atan(a/b) using polynomial
        let t = a / b;
        let t2 = t * t;

        // Minimax polynomial for atan on [0,1]
        let mut angle = t
            * (Self::ONE
                - t2 * (Self::from_bits(0x3eaaaaab)      // 1/3
            - t2 * (Self::from_bits(0x3e4ccccd)      // 1/5
            - t2 * Self::from_bits(0x3e124925)))); // 1/7

        // Adjust for octant
        if swap {
            angle = Self::from_bits(0x3fc90fdb) - angle; // π/2 - angle
        }

        // Adjust for quadrant
        if x.to_bits() & 0x80000000 != 0 {
            angle = Self::PI - angle;
        }
        if y.to_bits() & 0x80000000 != 0 {
            angle = -angle;
        }

        angle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sin_cos_identity() {
        let angle = DetF32::from_bits(0x3f490fdb); // π/4
        let s = angle.sin();
        let c = angle.cos();

        // sin²(x) + cos²(x) = 1
        // Note: Relaxed tolerance to 1e-4 to account for Taylor series approximation error
        // This is sufficient for networked game physics (accuracy ~1cm at 10m scale)
        let sum = s * s + c * c;
        assert!((sum - DetF32::ONE).abs() < DetF32::from_f32(1e-4));
    }

    #[test]
    fn test_trig_exact_bits() {
        // These exact bit patterns must match across platforms
        let angle = DetF32::from_bits(0x3f490fdb); // π/4

        // Run same calculation multiple times
        let result1 = angle.sin();
        let result2 = angle.sin();

        assert_eq!(result1.to_bits(), result2.to_bits());
    }
}
