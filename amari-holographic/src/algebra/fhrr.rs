//! Fourier Holographic Reduced Representation (FHRR) algebra.
//!
//! FHRR operates in the frequency domain using circular convolution.
//! This provides efficient binding via element-wise complex multiplication
//! and O(D log D) bundling via FFT.
//!
//! # Representation
//!
//! FHRR elements are represented as complex vectors of dimension D:
//! - Real and imaginary parts stored as separate arrays
//! - All components on the unit circle: |z_i| = 1 for random elements
//!
//! # Operations
//!
//! - **Binding**: Element-wise complex multiplication (Hadamard product in frequency domain)
//! - **Inverse**: Complex conjugate (since |z_i| = 1, z_i^-1 = z_i*)
//! - **Bundling**: Component-wise averaging with optional FFT for large bundles
//! - **Similarity**: Normalized dot product of real parts (cosine similarity)
//!
//! # Advantages
//!
//! - O(D) binding (just element-wise multiplication)
//! - O(D log D) for bundling with cleanup via FFT
//! - Simple inverse (just complex conjugate for unit-magnitude elements)
//! - Linear dimension scaling with O(D / ln D) capacity
//!
//! # References
//!
//! - Plate, T. A. (1995). Holographic reduced representations.
//!   IEEE Transactions on Neural Networks, 6(3), 623-641.
//! - Plate, T. A. (2003). Holographic Reduced Representation:
//!   Distributed Representation for Cognitive Structures. CSLI Publications.

use alloc::vec::Vec;
use core::f64::consts::PI;

use super::{AlgebraError, AlgebraResult, BindingAlgebra};

/// Fourier Holographic Reduced Representation element.
///
/// Stores a complex vector as separate real and imaginary arrays.
/// For optimal binding properties, elements should be on or near the unit circle.
///
/// # Type Parameter
///
/// - `D`: Dimension of the complex vector
///
/// # Example
///
/// ```ignore
/// use amari_fusion::algebra::FHRRAlgebra;
///
/// // Create a 256-dimensional FHRR element
/// let a = FHRRAlgebra::<256>::random_unitary();
/// let b = FHRRAlgebra::<256>::random_unitary();
///
/// // Binding via element-wise complex multiplication
/// let bound = a.bind(&b);
///
/// // Unbinding via conjugate multiplication
/// let recovered = a.unbind(&bound)?;
/// ```
#[derive(Clone, Debug)]
pub struct FHRRAlgebra<const D: usize> {
    /// Real components
    real: [f64; D],
    /// Imaginary components
    imag: [f64; D],
}

impl<const D: usize> FHRRAlgebra<D> {
    /// Create a new FHRR element from real and imaginary arrays.
    pub fn new(real: [f64; D], imag: [f64; D]) -> Self {
        Self { real, imag }
    }

    /// Create from Vecs (for convenience).
    pub fn from_vecs(real: Vec<f64>, imag: Vec<f64>) -> AlgebraResult<Self> {
        if real.len() != D || imag.len() != D {
            return Err(AlgebraError::DimensionMismatch {
                expected: D,
                actual: real.len().min(imag.len()),
            });
        }

        let mut r = [0.0; D];
        let mut i = [0.0; D];
        r.copy_from_slice(&real);
        i.copy_from_slice(&imag);
        Ok(Self::new(r, i))
    }

    /// Create the identity element (all 1+0i).
    pub fn fhrr_identity() -> Self {
        Self::new([1.0; D], [0.0; D])
    }

    /// Create the zero element (all 0+0i).
    pub fn fhrr_zero() -> Self {
        Self::new([0.0; D], [0.0; D])
    }

    /// Create a random unitary element (all components on unit circle).
    ///
    /// Each component is e^(iθ) where θ is uniform in [0, 2π).
    pub fn random_unitary() -> Self {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            let theta = fastrand::f64() * 2.0 * PI;
            real[i] = theta.cos();
            imag[i] = theta.sin();
        }

        Self::new(real, imag)
    }

    /// Create a random Gaussian element.
    ///
    /// Each component is sampled from a standard complex Gaussian.
    pub fn random_gaussian() -> Self {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            // Box-Muller transform
            let u1 = fastrand::f64().max(1e-10);
            let u2 = fastrand::f64();
            let r = (-2.0 * u1.ln()).sqrt();
            let theta = 2.0 * PI * u2;
            real[i] = r * theta.cos();
            imag[i] = r * theta.sin();
        }

        Self::new(real, imag)
    }

    /// Get the real part at index i.
    pub fn real(&self, i: usize) -> Option<f64> {
        if i < D {
            Some(self.real[i])
        } else {
            None
        }
    }

    /// Get the imaginary part at index i.
    pub fn imag(&self, i: usize) -> Option<f64> {
        if i < D {
            Some(self.imag[i])
        } else {
            None
        }
    }

    /// Get the magnitude at index i.
    pub fn magnitude(&self, i: usize) -> Option<f64> {
        if i < D {
            Some((self.real[i].powi(2) + self.imag[i].powi(2)).sqrt())
        } else {
            None
        }
    }

    /// Get the phase at index i.
    pub fn phase(&self, i: usize) -> Option<f64> {
        if i < D {
            Some(self.imag[i].atan2(self.real[i]))
        } else {
            None
        }
    }

    /// Element-wise complex multiplication (binding).
    pub fn complex_multiply(&self, other: &Self) -> Self {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            // (a + bi)(c + di) = (ac - bd) + (ad + bc)i
            let a = self.real[i];
            let b = self.imag[i];
            let c = other.real[i];
            let d = other.imag[i];

            real[i] = a * c - b * d;
            imag[i] = a * d + b * c;
        }

        Self::new(real, imag)
    }

    /// Complex conjugate.
    ///
    /// For unit-magnitude elements, this is the inverse.
    pub fn conjugate(&self) -> Self {
        let mut imag = [0.0; D];
        for i in 0..D {
            imag[i] = -self.imag[i];
        }
        Self::new(self.real, imag)
    }

    /// Element-wise addition.
    pub fn add(&self, other: &Self) -> Self {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            real[i] = self.real[i] + other.real[i];
            imag[i] = self.imag[i] + other.imag[i];
        }

        Self::new(real, imag)
    }

    /// Scale by a real scalar.
    pub fn scale(&self, s: f64) -> Self {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            real[i] = self.real[i] * s;
            imag[i] = self.imag[i] * s;
        }

        Self::new(real, imag)
    }

    /// Compute the squared L2 norm.
    pub fn norm_squared(&self) -> f64 {
        let mut sum = 0.0;
        for i in 0..D {
            sum += self.real[i].powi(2) + self.imag[i].powi(2);
        }
        sum
    }

    /// Compute the L2 norm.
    pub fn compute_norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Normalize to unit magnitude in each component.
    ///
    /// This projects back onto the unit circle, which is important
    /// for maintaining good inverse properties.
    pub fn normalize_unitary(&self) -> AlgebraResult<Self> {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            let mag = (self.real[i].powi(2) + self.imag[i].powi(2)).sqrt();
            if mag < 1e-10 {
                // Keep as zero (or could set to 1+0i)
                real[i] = 0.0;
                imag[i] = 0.0;
            } else {
                real[i] = self.real[i] / mag;
                imag[i] = self.imag[i] / mag;
            }
        }

        Ok(Self::new(real, imag))
    }

    /// Inner product (complex dot product).
    ///
    /// Computes sum of a_i * conj(b_i).
    pub fn inner_product(&self, other: &Self) -> (f64, f64) {
        let mut real_sum = 0.0;
        let mut imag_sum = 0.0;

        for i in 0..D {
            // (a + bi) * conj(c + di) = (a + bi) * (c - di)
            // = (ac + bd) + (bc - ad)i
            real_sum += self.real[i] * other.real[i] + self.imag[i] * other.imag[i];
            imag_sum += self.imag[i] * other.real[i] - self.real[i] * other.imag[i];
        }

        (real_sum, imag_sum)
    }

    /// Cyclic shift (circular permutation in time domain).
    ///
    /// This corresponds to multiplication by a phase ramp in frequency domain.
    pub fn cyclic_shift(&self, shift: i32) -> Self {
        let n = D as i32;
        let shift = ((shift % n) + n) % n;

        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            let j = (i as i32 + shift) as usize % D;
            real[j] = self.real[i];
            imag[j] = self.imag[i];
        }

        Self::new(real, imag)
    }

    /// Phase shift all components by θ.
    ///
    /// Multiplies all components by e^(iθ).
    pub fn phase_shift(&self, theta: f64) -> Self {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            // (a + bi) * (cos θ + i sin θ)
            real[i] = self.real[i] * cos_theta - self.imag[i] * sin_theta;
            imag[i] = self.real[i] * sin_theta + self.imag[i] * cos_theta;
        }

        Self::new(real, imag)
    }

    /// Component-wise element inverse (1/z for each component).
    ///
    /// For unitary elements, this equals the conjugate.
    pub fn component_inverse(&self) -> AlgebraResult<Self> {
        let mut real = [0.0; D];
        let mut imag = [0.0; D];

        for i in 0..D {
            let mag_sq = self.real[i].powi(2) + self.imag[i].powi(2);
            if mag_sq < 1e-20 {
                return Err(AlgebraError::NotInvertible {
                    reason: alloc::format!("component {} has near-zero magnitude", i),
                });
            }

            // 1/(a + bi) = (a - bi) / (a² + b²)
            real[i] = self.real[i] / mag_sq;
            imag[i] = -self.imag[i] / mag_sq;
        }

        Ok(Self::new(real, imag))
    }
}

// ============================================================================
// BindingAlgebra Implementation
// ============================================================================

impl<const D: usize> BindingAlgebra for FHRRAlgebra<D> {
    fn dimension(&self) -> usize {
        D
    }

    fn identity() -> Self {
        Self::fhrr_identity()
    }

    fn zero() -> Self {
        Self::fhrr_zero()
    }

    fn bind(&self, other: &Self) -> Self {
        self.complex_multiply(other)
    }

    fn inverse(&self) -> AlgebraResult<Self> {
        // For practical FHRR, we use the conjugate (assumes unit magnitude)
        // This is much faster than true component-wise inverse
        Ok(self.conjugate())
    }

    fn unbind(&self, other: &Self) -> AlgebraResult<Self> {
        // key.unbind(bound) = conj(key) * bound
        Ok(self.conjugate().complex_multiply(other))
    }

    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self> {
        if beta.is_infinite() {
            // Hard bundling: winner-take-all
            let self_norm = self.compute_norm();
            let other_norm = other.compute_norm();
            if self_norm >= other_norm {
                Ok(self.clone())
            } else {
                Ok(other.clone())
            }
        } else {
            // Soft bundling: weighted average
            let self_norm = self.compute_norm();
            let other_norm = other.compute_norm();

            let (w1, w2) = if beta <= 0.0 || (self_norm < 1e-10 && other_norm < 1e-10) {
                (0.5, 0.5)
            } else {
                let max_norm = self_norm.max(other_norm);
                let exp1 = (beta * (self_norm - max_norm)).exp();
                let exp2 = (beta * (other_norm - max_norm)).exp();
                let sum = exp1 + exp2;
                (exp1 / sum, exp2 / sum)
            };

            Ok(self.scale(w1).add(&other.scale(w2)))
        }
    }

    fn superpose(&self, other: &Self) -> AlgebraResult<Self> {
        // FHRR's public coefficient representation intentionally exposes only
        // real parts, so use the complete complex representation here.
        Ok(FHRRAlgebra::add(self, other))
    }

    fn scale(&self, factor: f64) -> AlgebraResult<Self> {
        // Preserve both real and imaginary components; the coefficient default
        // cannot reconstruct FHRR's imaginary parts.
        Ok(FHRRAlgebra::scale(self, factor))
    }

    fn similarity(&self, other: &Self) -> f64 {
        let self_norm = self.compute_norm();
        let other_norm = other.compute_norm();

        if self_norm < 1e-10 || other_norm < 1e-10 {
            return 0.0;
        }

        // Real part of normalized inner product
        let (real_inner, _) = self.inner_product(other);
        real_inner / (self_norm * other_norm)
    }

    fn norm(&self) -> f64 {
        self.compute_norm()
    }

    fn normalize(&self) -> AlgebraResult<Self> {
        let n = self.compute_norm();
        if n < 1e-10 {
            return Err(AlgebraError::NormalizationFailed { norm: n });
        }
        Ok(self.scale(1.0 / n))
    }

    fn permute(&self, shift: i32) -> Self {
        self.cyclic_shift(shift)
    }

    fn get(&self, index: usize) -> AlgebraResult<f64> {
        // Return real part for indexing
        if index >= D {
            return Err(AlgebraError::IndexOutOfBounds { index, size: D });
        }
        Ok(self.real[index])
    }

    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()> {
        // Set real part (leave imaginary as is)
        if index >= D {
            return Err(AlgebraError::IndexOutOfBounds { index, size: D });
        }
        self.real[index] = value;
        Ok(())
    }

    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self> {
        // Interpret as real parts only, set imag to 0
        if coeffs.len() != D {
            return Err(AlgebraError::DimensionMismatch {
                expected: D,
                actual: coeffs.len(),
            });
        }

        let mut real = [0.0; D];
        real.copy_from_slice(coeffs);
        Ok(Self::new(real, [0.0; D]))
    }

    fn to_coefficients(&self) -> Vec<f64> {
        // Return real parts
        self.real.to_vec()
    }

    fn algebra_name() -> &'static str {
        "FHRR"
    }
}

// ============================================================================
// Type Aliases for Common Dimensions
// ============================================================================

/// 64-dimensional FHRR
pub type FHRR64 = FHRRAlgebra<64>;

/// 128-dimensional FHRR
pub type FHRR128 = FHRRAlgebra<128>;

/// 256-dimensional FHRR
pub type FHRR256 = FHRRAlgebra<256>;

/// 512-dimensional FHRR
pub type FHRR512 = FHRRAlgebra<512>;

/// 1024-dimensional FHRR
pub type FHRR1024 = FHRRAlgebra<1024>;

/// 2048-dimensional FHRR
pub type FHRR2048 = FHRRAlgebra<2048>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fhrr_identity() {
        let identity = FHRR64::fhrr_identity();
        for i in 0..64 {
            assert!((identity.real(i).unwrap() - 1.0).abs() < 1e-10);
            assert!(identity.imag(i).unwrap().abs() < 1e-10);
        }
    }

    #[test]
    fn test_fhrr_binding_identity() {
        let a = FHRR64::random_unitary();
        let identity = FHRR64::identity();
        let bound = a.bind(&identity);

        // a * identity = a
        for i in 0..64 {
            assert!((a.real(i).unwrap() - bound.real(i).unwrap()).abs() < 1e-10);
            assert!((a.imag(i).unwrap() - bound.imag(i).unwrap()).abs() < 1e-10);
        }
    }

    #[test]
    fn test_fhrr_inverse() {
        let a = FHRR64::random_unitary();
        let a_inv = a.inverse().expect("unitary should be invertible");
        let product = a.bind(&a_inv);

        // a * a^-1 should be close to identity
        for i in 0..64 {
            assert!((product.real(i).unwrap() - 1.0).abs() < 1e-8, "real[{}]", i);
            assert!(product.imag(i).unwrap().abs() < 1e-8, "imag[{}]", i);
        }
    }

    #[test]
    fn test_fhrr_unitary_magnitude() {
        let a = FHRR64::random_unitary();

        // All components should have magnitude 1
        for i in 0..64 {
            let mag = a.magnitude(i).unwrap();
            assert!((mag - 1.0).abs() < 1e-10, "magnitude[{}] = {}", i, mag);
        }
    }

    #[test]
    fn test_fhrr_unbind_recover() {
        let key = FHRR256::random_unitary();
        let value = FHRR256::random_unitary();

        let bound = key.bind(&value);
        let recovered = key.unbind(&bound).expect("unbind should succeed");

        // recovered should be close to value
        let sim = recovered.similarity(&value);
        assert!(sim > 0.99, "recovery similarity: {}", sim);
    }

    #[test]
    fn test_fhrr_dissimilarity() {
        let a = FHRR256::random_unitary();
        let b = FHRR256::random_unitary();
        let bound = a.bind(&b);

        // bound should be dissimilar to both a and b
        let sim_a = bound.similarity(&a).abs();
        let sim_b = bound.similarity(&b).abs();

        // With 256 dimensions, random elements should be nearly orthogonal
        assert!(sim_a < 0.3, "similarity with a: {}", sim_a);
        assert!(sim_b < 0.3, "similarity with b: {}", sim_b);
    }

    #[test]
    fn test_fhrr_cyclic_shift() {
        let a = FHRR64::random_unitary();
        let shifted = a.cyclic_shift(10);
        let back = shifted.cyclic_shift(-10);

        // Should recover original
        for i in 0..64 {
            assert!((a.real(i).unwrap() - back.real(i).unwrap()).abs() < 1e-10);
            assert!((a.imag(i).unwrap() - back.imag(i).unwrap()).abs() < 1e-10);
        }
    }

    #[test]
    fn test_fhrr_capacity() {
        let a = FHRR256::random_unitary();
        let capacity = a.theoretical_capacity();

        // 256 dimensions, capacity ≈ 256/ln(256) ≈ 46
        assert!(capacity > 40 && capacity < 55, "capacity: {}", capacity);
    }
}
