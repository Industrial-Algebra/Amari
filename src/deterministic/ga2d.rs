//! Deterministic 2D Geometric Algebra types

use super::scalar::{DetF32, Deterministic};
use core::ops::{Add, Mul, Sub};

/// Deterministic 2D vector
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetVector2 {
    pub x: DetF32,
    pub y: DetF32,
}

// SAFETY (M-REP-D S7): every DetVector2 operation reduces to DetF32
// arithmetic (add/mul for dot/wedge; magnitude/normalize use the
// fixed-iteration Newton sqrt — an approximation, no accuracy claim).
// Pure arithmetic, no libm: deterministic and cross-platform bit-exact
// on the trait's finite domain (finite inputs, intermediates, results;
// NaN bit patterns carry no promise).
unsafe impl Deterministic for DetVector2 {}

impl DetVector2 {
    #[inline]
    pub const fn new(x: DetF32, y: DetF32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn from_f32(x: f32, y: f32) -> Self {
        Self {
            x: DetF32::from_f32(x),
            y: DetF32::from_f32(y),
        }
    }

    pub const ZERO: Self = Self::new(DetF32::ZERO, DetF32::ZERO);
    pub const X_AXIS: Self = Self::new(DetF32::ONE, DetF32::ZERO);
    pub const Y_AXIS: Self = Self::new(DetF32::ZERO, DetF32::ONE);

    /// Dot product
    #[inline(never)]
    pub fn dot(self, other: Self) -> DetF32 {
        self.x * other.x + self.y * other.y
    }

    /// Magnitude squared (avoids sqrt)
    #[inline(never)]
    pub fn magnitude_sq(self) -> DetF32 {
        self.dot(self)
    }

    /// Magnitude
    #[inline(never)]
    pub fn magnitude(self) -> DetF32 {
        self.magnitude_sq().sqrt()
    }

    /// Normalized vector
    #[inline(never)]
    pub fn normalize(self) -> Self {
        let mag = self.magnitude();
        Self {
            x: self.x / mag,
            y: self.y / mag,
        }
    }

    /// Wedge product (returns bivector/scalar in 2D)
    #[inline(never)]
    pub fn wedge(self, other: Self) -> DetF32 {
        self.x * other.y - self.y * other.x
    }
}

impl Add for DetVector2 {
    type Output = Self;

    #[inline(never)]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for DetVector2 {
    type Output = Self;

    #[inline(never)]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<DetF32> for DetVector2 {
    type Output = Self;

    #[inline(never)]
    fn mul(self, scalar: DetF32) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

/// Deterministic 2D rotor for rotations
///
/// Represents rotation as R = cos(θ/2) + sin(θ/2) e₁₂
///
/// # Example
/// ```
/// use amari::deterministic::ga2d::{DetRotor2, DetVector2};
/// use amari::deterministic::DetF32;
///
/// let angle = DetF32::PI * DetF32::from_f32(0.25); // 45°
/// let rotor = DetRotor2::from_angle(angle);
///
/// let v = DetVector2::X_AXIS;
/// let rotated = rotor.transform(v);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetRotor2 {
    /// Scalar part: cos(θ/2)
    pub s: DetF32,
    /// Bivector part: sin(θ/2)
    pub b: DetF32,
}

// SAFETY (M-REP-D S7, R3 corrected): compose/transform/inverse are
// IEEE-754 basic arithmetic; `from_angle`/`to_angle` call DetF32's
// TAYLOR-series sin/cos and the atan2 approximation (approx.rs) — pure
// fixed arithmetic, NOT libm (the R1 attestation claiming libm here was
// wrong). Deterministic and cross-platform bit-exact on the trait's
// finite domain (finite inputs, intermediates, results; NaN bit
// patterns carry no promise); accuracy limited to the Taylor working
// ranges (no accuracy claim).
unsafe impl Deterministic for DetRotor2 {}

impl DetRotor2 {
    #[inline]
    pub const fn new(s: DetF32, b: DetF32) -> Self {
        Self { s, b }
    }

    pub const IDENTITY: Self = Self::new(DetF32::ONE, DetF32::ZERO);

    /// Create rotor from angle (deterministic)
    #[inline(never)]
    pub fn from_angle(angle: DetF32) -> Self {
        let half_angle = angle * DetF32::HALF;
        Self {
            s: half_angle.cos(),
            b: half_angle.sin(),
        }
    }

    /// Rotor multiplication (composition)
    ///
    /// R₁ ∘ R₂ = (s₁ + b₁e₁₂)(s₂ + b₂e₁₂)
    ///         = (s₁s₂ - b₁b₂) + (s₁b₂ + b₁s₂)e₁₂
    #[inline(never)]
    pub fn compose(self, other: Self) -> Self {
        Self {
            s: self.s * other.s - self.b * other.b,
            b: self.s * other.b + self.b * other.s,
        }
    }

    /// Transform vector by rotor
    ///
    /// v' = R v R†
    ///
    /// In 2D this simplifies to rotation matrix form:
    /// [x']   [s² - b²    -2sb  ] [x]
    /// [y'] = [2sb        s² - b²] [y]
    #[inline(never)]
    pub fn transform(self, v: DetVector2) -> DetVector2 {
        let s2 = self.s * self.s;
        let b2 = self.b * self.b;
        let sb2 = self.s * self.b * DetF32::TWO;

        DetVector2 {
            x: (s2 - b2) * v.x - sb2 * v.y,
            y: sb2 * v.x + (s2 - b2) * v.y,
        }
    }

    /// Inverse rotor (conjugate in 2D)
    #[inline]
    pub fn inverse(self) -> Self {
        Self {
            s: self.s,
            b: -self.b,
        }
    }

    /// Extract angle from rotor
    #[inline(never)]
    pub fn to_angle(self) -> DetF32 {
        DetF32::atan2(self.b, self.s) * DetF32::TWO
    }
}

impl Mul for DetRotor2 {
    type Output = Self;

    #[inline(never)]
    fn mul(self, rhs: Self) -> Self {
        self.compose(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotor_identity() {
        let v = DetVector2::new(DetF32::ONE, DetF32::TWO);
        let r = DetRotor2::IDENTITY;
        let result = r.transform(v);

        assert_eq!(result.x.to_bits(), v.x.to_bits());
        assert_eq!(result.y.to_bits(), v.y.to_bits());
    }

    #[test]
    fn test_rotor_composition_determinism() {
        // Test that rotor composition produces bit-exact results
        // (determinism test, not precision test)
        let r1 = DetRotor2::from_angle(DetF32::PI * DetF32::from_f32(0.25)); // 45°
        let r2 = DetRotor2::from_angle(DetF32::PI * DetF32::from_f32(0.25)); // 45°

        // Compose multiple times - must produce identical bit patterns
        let composed1 = r1 * r2;
        let composed2 = r1 * r2;
        let composed3 = r1 * r2;

        // Verify bit-exact reproducibility
        assert_eq!(composed1.s.to_bits(), composed2.s.to_bits());
        assert_eq!(composed1.b.to_bits(), composed2.b.to_bits());
        assert_eq!(composed2.s.to_bits(), composed3.s.to_bits());
        assert_eq!(composed2.b.to_bits(), composed3.b.to_bits());

        // Also verify transformation produces identical bits
        let v = DetVector2::X_AXIS;
        let result1 = composed1.transform(v);
        let result2 = composed2.transform(v);

        assert_eq!(result1.x.to_bits(), result2.x.to_bits());
        assert_eq!(result1.y.to_bits(), result2.y.to_bits());
    }

    #[test]
    fn test_deterministic_bits() {
        // Same operations must produce identical bit patterns
        let angle = DetF32::from_bits(0x3f490fdb);
        let r1 = DetRotor2::from_angle(angle);
        let r2 = DetRotor2::from_angle(angle);

        assert_eq!(r1.s.to_bits(), r2.s.to_bits());
        assert_eq!(r1.b.to_bits(), r2.b.to_bits());
    }
}
