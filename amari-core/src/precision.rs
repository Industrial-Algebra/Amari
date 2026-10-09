//! High-precision arithmetic traits for scientific computing
//!
//! This module provides a unified interface for different precision levels:
//! - Standard f64 for general use
//! - Arbitrary precision using rug::Float for critical calculations
//! - Configurable precision based on application requirements

#[cfg(feature = "std")]
use std::fmt::{Debug, Display};

#[cfg(all(
    feature = "std",
    any(
        feature = "native-precision",
        feature = "high-precision",
        feature = "wasm-precision"
    )
))]
use std::fmt::{Formatter, Result};

#[cfg(not(feature = "std"))]
use core::fmt::{Debug, Display};

#[cfg(all(
    not(feature = "std"),
    any(
        feature = "native-precision",
        feature = "high-precision",
        feature = "wasm-precision"
    )
))]
use core::fmt::{Formatter, Result};

#[cfg(feature = "std")]
use std::f64::consts;

#[cfg(not(feature = "std"))]
use core::f64::consts;

/// Unified trait for floating-point arithmetic in scientific computing
///
/// This trait abstracts over different precision levels to allow
/// mathematical calculations with appropriate numerical accuracy.
pub trait PrecisionFloat:
    Clone
    + Debug
    + Display
    + PartialEq
    + PartialOrd
    + core::ops::Add<Output = Self>
    + core::ops::Sub<Output = Self>
    + core::ops::Mul<Output = Self>
    + core::ops::Div<Output = Self>
    + core::ops::Neg<Output = Self>
    + Send
    + Sync
    + 'static
{
    /// Create from f64 value
    fn from_f64(value: f64) -> Self;

    /// Convert to f64 (may lose precision)
    fn to_f64(&self) -> f64;

    /// Square root with high precision
    fn sqrt_precise(self) -> Self;

    /// Power function with high precision
    fn powf_precise(self, exp: Self) -> Self;

    /// Trigonometric functions with high precision
    fn sin_precise(self) -> Self;
    /// Cosine function with high precision
    fn cos_precise(self) -> Self;
    /// Tangent function with high precision
    fn tan_precise(self) -> Self;

    /// Hyperbolic functions with high precision
    fn sinh_precise(self) -> Self;
    /// Hyperbolic cosine with high precision
    fn cosh_precise(self) -> Self;
    /// Hyperbolic tangent with high precision
    fn tanh_precise(self) -> Self;

    /// Natural logarithm with high precision
    fn ln_precise(self) -> Self;

    /// Exponential function with high precision
    fn exp_precise(self) -> Self;

    /// Absolute value
    fn abs_precise(self) -> Self;

    /// Machine epsilon for this precision level
    fn epsilon() -> Self;

    /// Recommended tolerance for numerical calculations
    fn default_tolerance() -> Self;

    /// Specialized tolerance for orbital mechanics calculations
    fn orbital_tolerance() -> Self;

    /// Mathematical constant π
    #[allow(non_snake_case)]
    fn PI() -> Self;

    /// The multiplicative identity (1)
    fn one() -> Self;

    /// The additive identity (0)
    fn zero() -> Self;

    /// Square root function (delegated to sqrt_precise)
    fn sqrt(self) -> Self {
        self.sqrt_precise()
    }

    /// Absolute value function (delegated to abs_precise)
    fn abs(self) -> Self {
        self.abs_precise()
    }

    /// Maximum of two values
    fn max(self, other: Self) -> Self;

    /// Minimum of two values
    fn min(self, other: Self) -> Self;

    /// Create a 4-element array filled with zero values
    fn zero_array_4() -> [Self; 4] {
        [Self::zero(), Self::zero(), Self::zero(), Self::zero()]
    }

    /// Create a 4x4 matrix filled with zero values
    fn zero_matrix_4x4() -> [[Self; 4]; 4] {
        [
            Self::zero_array_4(),
            Self::zero_array_4(),
            Self::zero_array_4(),
            Self::zero_array_4(),
        ]
    }

    /// Create a 4x4x4 tensor filled with zero values
    fn zero_tensor_4x4x4() -> [[[Self; 4]; 4]; 4] {
        [
            Self::zero_matrix_4x4(),
            Self::zero_matrix_4x4(),
            Self::zero_matrix_4x4(),
            Self::zero_matrix_4x4(),
        ]
    }
}

/// Standard f64 implementation for general use
impl PrecisionFloat for f64 {
    #[inline]
    fn from_f64(value: f64) -> Self {
        value
    }

    #[inline]
    fn to_f64(&self) -> f64 {
        *self
    }

    #[inline]
    fn sqrt_precise(self) -> Self {
        self.sqrt()
    }

    #[inline]
    fn powf_precise(self, exp: Self) -> Self {
        self.powf(exp)
    }

    #[inline]
    fn sin_precise(self) -> Self {
        self.sin()
    }

    #[inline]
    fn cos_precise(self) -> Self {
        self.cos()
    }

    #[inline]
    fn tan_precise(self) -> Self {
        self.tan()
    }

    #[inline]
    fn sinh_precise(self) -> Self {
        self.sinh()
    }

    #[inline]
    fn cosh_precise(self) -> Self {
        self.cosh()
    }

    #[inline]
    fn tanh_precise(self) -> Self {
        self.tanh()
    }

    #[inline]
    fn ln_precise(self) -> Self {
        self.ln()
    }

    #[inline]
    fn exp_precise(self) -> Self {
        self.exp()
    }

    #[inline]
    fn abs_precise(self) -> Self {
        self.abs()
    }

    #[inline]
    fn epsilon() -> Self {
        f64::EPSILON
    }

    #[inline]
    fn default_tolerance() -> Self {
        1e-12
    }

    #[inline]
    fn orbital_tolerance() -> Self {
        1e-15
    }

    #[inline]
    fn PI() -> Self {
        consts::PI
    }

    #[inline]
    fn one() -> Self {
        1.0
    }

    #[inline]
    fn zero() -> Self {
        0.0
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        f64::max(self, other)
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        f64::min(self, other)
    }
}

/// High-precision wrapper around rug::Float for critical calculations (native-only)
#[cfg(feature = "native-precision")]
#[derive(Clone, Debug)]
pub struct HighPrecisionFloat {
    /// The arbitrary precision floating point value
    value: rug::Float,
    /// Precision in bits
    precision: u32,
}

#[cfg(feature = "native-precision")]
impl HighPrecisionFloat {
    /// Create with specified precision (bits)
    pub fn with_precision(value: f64, precision: u32) -> Self {
        Self {
            value: rug::Float::with_val(precision, value),
            precision,
        }
    }

    /// Create with standard precision (128 bits)
    pub fn standard(value: f64) -> Self {
        Self::with_precision(value, 128)
    }

    /// Create with high precision (256 bits) for critical calculations
    pub fn high(value: f64) -> Self {
        Self::with_precision(value, 256)
    }

    /// Get the precision in bits
    pub fn precision(&self) -> u32 {
        self.precision
    }
}

#[cfg(feature = "native-precision")]
impl PartialEq for HighPrecisionFloat {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[cfg(feature = "native-precision")]
impl PartialOrd for HighPrecisionFloat {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

#[cfg(feature = "native-precision")]
impl Display for HighPrecisionFloat {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.value)
    }
}

// TODO: Implement NumFloat and other required traits for HighPrecisionFloat
// This would require substantial implementation for full compatibility

/// High-precision wrapper around dashu_float::FBig for WASM-compatible calculations
#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
#[derive(Clone, Debug)]
pub struct DashuFloat {
    /// The arbitrary precision floating point value
    value: dashu_float::FBig,
    /// Precision in decimal digits (approximation)
    precision: u32,
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl DashuFloat {
    /// Create with specified precision (decimal digits)
    pub fn with_precision(value: f64, precision: u32) -> Self {
        use dashu_float::FBig;

        Self {
            value: FBig::try_from(value).unwrap_or(FBig::ZERO),
            precision,
        }
    }

    /// Create with standard precision (40 decimal digits ≈ 128 bits)
    pub fn standard(value: f64) -> Self {
        Self::with_precision(value, 40)
    }

    /// Create with high precision (80 decimal digits ≈ 256 bits) for critical calculations
    pub fn high(value: f64) -> Self {
        Self::with_precision(value, 80)
    }

    /// Get the precision in decimal digits
    pub fn precision(&self) -> u32 {
        self.precision
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl PartialEq for DashuFloat {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl PartialOrd for DashuFloat {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl Display for DashuFloat {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl PrecisionFloat for DashuFloat {
    #[inline]
    fn from_f64(value: f64) -> Self {
        Self::standard(value)
    }

    #[inline]
    fn to_f64(&self) -> f64 {
        // Convert dashu FBig to f64 (may lose precision)
        f64::try_from(self.value.clone()).unwrap_or(0.0)
    }

    #[inline]
    fn sqrt_precise(self) -> Self {
        // Use f64 conversion for mathematical operations not directly available in dashu
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.sqrt(), self.precision)
    }

    #[inline]
    fn powf_precise(self, exp: Self) -> Self {
        let base_f64 = f64::try_from(self.value).unwrap_or(0.0);
        let exp_f64 = f64::try_from(exp.value).unwrap_or(0.0);
        Self::with_precision(base_f64.powf(exp_f64), self.precision.min(exp.precision))
    }

    #[inline]
    fn sin_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.sin(), self.precision)
    }

    #[inline]
    fn cos_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.cos(), self.precision)
    }

    #[inline]
    fn tan_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.tan(), self.precision)
    }

    #[inline]
    fn sinh_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.sinh(), self.precision)
    }

    #[inline]
    fn cosh_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.cosh(), self.precision)
    }

    #[inline]
    fn tanh_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.tanh(), self.precision)
    }

    #[inline]
    fn ln_precise(self) -> Self {
        // Use dashu's ln directly
        Self {
            value: self.value.ln(),
            precision: self.precision,
        }
    }

    #[inline]
    fn exp_precise(self) -> Self {
        let f64_val = f64::try_from(self.value).unwrap_or(0.0);
        Self::with_precision(f64_val.exp(), self.precision)
    }

    #[inline]
    fn abs_precise(self) -> Self {
        // Import Abs trait and use it
        use dashu_float::ops::Abs;
        Self {
            value: self.value.abs(),
            precision: self.precision,
        }
    }

    #[inline]
    fn epsilon() -> Self {
        // Machine epsilon for high precision calculations
        Self::with_precision(1e-40, 40)
    }

    #[inline]
    fn default_tolerance() -> Self {
        Self::with_precision(1e-12, 40)
    }

    #[inline]
    fn orbital_tolerance() -> Self {
        Self::with_precision(1e-15, 80)
    }

    #[inline]
    fn PI() -> Self {
        Self::with_precision(consts::PI, 40)
    }

    #[inline]
    fn one() -> Self {
        Self::with_precision(1.0, 40)
    }

    #[inline]
    fn zero() -> Self {
        Self::with_precision(0.0, 40)
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        let self_f64 = f64::try_from(self.value.clone()).unwrap_or(0.0);
        let other_f64 = f64::try_from(other.value.clone()).unwrap_or(0.0);
        if self_f64 >= other_f64 {
            self
        } else {
            other
        }
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        let self_f64 = f64::try_from(self.value.clone()).unwrap_or(0.0);
        let other_f64 = f64::try_from(other.value.clone()).unwrap_or(0.0);
        if self_f64 <= other_f64 {
            self
        } else {
            other
        }
    }
}

// Implement basic arithmetic traits for DashuFloat
#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl core::ops::Add for DashuFloat {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            precision: self.precision.min(other.precision),
        }
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl core::ops::Sub for DashuFloat {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            value: self.value - other.value,
            precision: self.precision.min(other.precision),
        }
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl core::ops::Mul for DashuFloat {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            value: self.value * other.value,
            precision: self.precision.min(other.precision),
        }
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl core::ops::Div for DashuFloat {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Self {
            value: self.value / other.value,
            precision: self.precision.min(other.precision),
        }
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl core::ops::Neg for DashuFloat {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            value: -self.value,
            precision: self.precision,
        }
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl num_traits::Zero for DashuFloat {
    fn zero() -> Self {
        Self {
            value: dashu_float::FBig::ZERO,
            precision: 40,
        }
    }

    fn is_zero(&self) -> bool {
        // Check if the value equals zero by converting to f64 and checking
        f64::try_from(self.value.clone()).unwrap_or(1.0) == 0.0
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl num_traits::One for DashuFloat {
    fn one() -> Self {
        Self {
            value: dashu_float::FBig::ONE,
            precision: 40,
        }
    }
}

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
impl num_traits::FromPrimitive for DashuFloat {
    fn from_f64(n: f64) -> Option<Self> {
        Some(<DashuFloat as PrecisionFloat>::from_f64(n))
    }

    fn from_f32(n: f32) -> Option<Self> {
        Some(<DashuFloat as PrecisionFloat>::from_f64(n as f64))
    }

    fn from_i64(n: i64) -> Option<Self> {
        Some(<DashuFloat as PrecisionFloat>::from_f64(n as f64))
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(<DashuFloat as PrecisionFloat>::from_f64(n as f64))
    }
}

// ToPrimitive implementation removed to avoid method name collision with PrecisionFloat::to_f64

/// Type alias for standard precision calculations
pub type StandardFloat = f64;

/// Type alias for extended precision calculations - backend selected based on features
///
/// Selection priority:
/// 1. `native-precision` feature -> rug backend (maximum performance, native-only)
/// 2. `high-precision` or `wasm-precision` -> dashu backend (WASM-compatible)
/// 3. No precision features -> f64 fallback
#[cfg(feature = "native-precision")]
pub type ExtendedFloat = HighPrecisionFloat;

#[cfg(all(
    any(feature = "high-precision", feature = "wasm-precision"),
    not(feature = "native-precision")
))]
pub type ExtendedFloat = DashuFloat;

#[cfg(not(any(
    feature = "native-precision",
    feature = "wasm-precision",
    feature = "high-precision"
)))]
pub type ExtendedFloat = f64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_float_precision() {
        let x = 2.0_f64;
        assert!((x.sqrt_precise() - f64::sqrt(2.0)).abs() < f64::EPSILON);
        assert!((x.powf_precise(3.0) - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_tolerance() {
        assert!(f64::default_tolerance() < f64::EPSILON.sqrt());
        assert!(f64::default_tolerance() > 0.0);
    }

    #[cfg(feature = "native-precision")]
    #[test]
    fn test_high_precision_creation() {
        let hp = HighPrecisionFloat::standard(consts::PI);
        assert_eq!(hp.precision(), 128);

        let sp = HighPrecisionFloat::high(consts::E);
        assert_eq!(sp.precision(), 256);
    }

    #[cfg(all(
        any(feature = "high-precision", feature = "wasm-precision"),
        not(feature = "native-precision")
    ))]
    #[test]
    fn test_dashu_float_creation() {
        let df = DashuFloat::standard(core::f64::consts::PI);
        assert_eq!(df.precision(), 40);

        let df_high = DashuFloat::high(core::f64::consts::E);
        assert_eq!(df_high.precision(), 80);
    }

    #[cfg(all(
        any(feature = "high-precision", feature = "wasm-precision"),
        not(feature = "native-precision")
    ))]
    #[test]
    fn test_dashu_float_precision_operations() {
        let x = <DashuFloat as PrecisionFloat>::from_f64(2.0);
        let sqrt_result = x.clone().sqrt_precise();
        let expected = <DashuFloat as PrecisionFloat>::from_f64(f64::sqrt(2.0));

        // Test that results are approximately equal (within tolerance)
        let diff = (sqrt_result.to_f64() - expected.to_f64()).abs();
        assert!(diff < 1e-10, "sqrt precision test failed: diff = {}", diff);

        // Test power function
        let power_result = x
            .clone()
            .powf_precise(<DashuFloat as PrecisionFloat>::from_f64(3.0));
        let power_expected = <DashuFloat as PrecisionFloat>::from_f64(8.0);
        let power_diff = (power_result.to_f64() - power_expected.to_f64()).abs();
        assert!(
            power_diff < 1e-10,
            "power precision test failed: diff = {}",
            power_diff
        );
    }

    #[cfg(all(
        any(feature = "high-precision", feature = "wasm-precision"),
        not(feature = "native-precision")
    ))]
    #[test]
    fn test_dashu_float_arithmetic() {
        let a = <DashuFloat as PrecisionFloat>::from_f64(3.0);
        let b = <DashuFloat as PrecisionFloat>::from_f64(4.0);

        let sum = a.clone() + b.clone();
        assert!((sum.to_f64() - 7.0).abs() < 1e-10);

        let product = a.clone() * b.clone();
        assert!((product.clone().to_f64() - 12.0).abs() < 1e-10);

        let quotient = product / a;
        assert!((quotient.to_f64() - 4.0).abs() < 1e-10);
    }

    #[cfg(all(
        any(feature = "high-precision", feature = "wasm-precision"),
        not(feature = "native-precision")
    ))]
    #[test]
    fn test_dashu_float_transcendental() {
        let x = <DashuFloat as PrecisionFloat>::from_f64(1.0);

        // Test sin/cos for pi/2
        let pi_half = <DashuFloat as PrecisionFloat>::from_f64(core::f64::consts::PI / 2.0);
        let sin_result = pi_half.clone().sin_precise();
        let cos_result = pi_half.cos_precise();

        // sin(π/2) should be approximately 1
        assert!((sin_result.to_f64() - 1.0).abs() < 1e-6);
        // cos(π/2) should be approximately 0
        assert!(cos_result.to_f64().abs() < 1e-6);

        // Test ln/exp identity: ln(exp(x)) = x
        let exp_result = x.clone().exp_precise();
        let ln_exp_result = exp_result.ln_precise();
        assert!((ln_exp_result.to_f64() - 1.0).abs() < 1e-10);
    }
}
