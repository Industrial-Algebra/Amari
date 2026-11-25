//! Optimized Cl(3,0,0) implementation with fixed-size operations.
//!
//! This module provides a highly optimized implementation of the 3D Euclidean
//! Clifford algebra, which is the building block for ProductCliffordAlgebra.
//!
//! # Basis Elements
//!
//! Cl(3,0,0) has 8 basis elements:
//! - Grade 0 (scalar): `1`
//! - Grade 1 (vectors): `e1, e2, e3`
//! - Grade 2 (bivectors): `e12, e13, e23`
//! - Grade 3 (trivector/pseudoscalar): `e123`
//!
//! # Representation
//!
//! We use a fixed-size array of 8 coefficients:
//! ```text
//! [s, e1, e2, e3, e12, e13, e23, e123]
//!  0   1   2   3    4    5    6     7
//! ```
//!
//! # Geometric Product
//!
//! Products use the signed basis table (row times column):
//! ```text
//!      |     1    e1    e2    e3   e12   e13   e23  e123
//!    1 |     1    e1    e2    e3   e12   e13   e23  e123
//!   e1 |    e1     1   e12   e13    e2    e3  e123   e23
//!   e2 |    e2  -e12     1   e23   -e1 -e123    e3  -e13
//!   e3 |    e3  -e13  -e23     1  e123   -e1   -e2   e12
//!  e12 |   e12   -e2    e1  e123    -1  -e23   e13   -e3
//!  e13 |   e13   -e3 -e123    e1   e23    -1  -e12    e2
//!  e23 |   e23  e123   -e3    e2  -e13   e12    -1   -e1
//! e123 |  e123   e23  -e13   e12   -e3    e2   -e1    -1
//! ```

use alloc::vec::Vec;

use super::{AlgebraError, AlgebraResult, BindingAlgebra, GeometricAlgebra};

/// Optimized 3D Euclidean Clifford algebra Cl(3,0,0).
///
/// Uses a fixed-size array representation with a table-driven geometric product
/// for maximum performance. This is the building block for `ProductCliffordAlgebra`.
///
/// # Example
///
/// ```ignore
/// use amari_fusion::algebra::Cl3;
///
/// // Create a vector e1
/// let e1 = Cl3::unit_vector(0);
///
/// // Create a vector e2
/// let e2 = Cl3::unit_vector(1);
///
/// // Geometric product gives bivector e12
/// let e12 = e1.bind(&e2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cl3 {
    /// Coefficients: [s, e1, e2, e3, e12, e13, e23, e123]
    coeffs: [f64; 8],
}

impl Cl3 {
    /// The algebra dimension (8 basis elements).
    pub const DIMENSION: usize = 8;

    /// Index constants for clarity
    const S: usize = 0;
    const E1: usize = 1;
    const E2: usize = 2;
    const E3: usize = 3;
    const E12: usize = 4;
    const E13: usize = 5;
    const E23: usize = 6;
    const E123: usize = 7;

    /// Create a new Cl3 element from coefficients.
    #[inline]
    pub const fn new(coeffs: [f64; 8]) -> Self {
        Self { coeffs }
    }

    /// Create the scalar 1.
    #[inline]
    pub const fn one() -> Self {
        Self::new([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
    }

    /// Create a scalar element.
    #[inline]
    pub const fn scalar(value: f64) -> Self {
        Self::new([value, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
    }

    /// Create the zero element.
    #[inline]
    pub const fn new_zero() -> Self {
        Self::new([0.0; 8])
    }

    /// Create a unit vector: e1 (i=0), e2 (i=1), or e3 (i=2).
    #[inline]
    pub fn unit_vector(i: usize) -> Self {
        let mut coeffs = [0.0; 8];
        if i < 3 {
            coeffs[i + 1] = 1.0;
        }
        Self::new(coeffs)
    }

    /// Create a vector from components.
    #[inline]
    pub fn vector(x: f64, y: f64, z: f64) -> Self {
        Self::new([0.0, x, y, z, 0.0, 0.0, 0.0, 0.0])
    }

    /// Create a bivector from components.
    #[inline]
    pub fn bivector(xy: f64, xz: f64, yz: f64) -> Self {
        Self::new([0.0, 0.0, 0.0, 0.0, xy, xz, yz, 0.0])
    }

    /// Create the pseudoscalar e123.
    #[inline]
    pub fn pseudoscalar(value: f64) -> Self {
        Self::new([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, value])
    }

    /// Get a coefficient by index.
    #[inline]
    pub const fn coeff(&self, i: usize) -> f64 {
        self.coeffs[i]
    }

    /// Set a coefficient by index.
    #[inline]
    pub fn set_coeff(&mut self, i: usize, value: f64) {
        self.coeffs[i] = value;
    }

    /// Get all coefficients.
    #[inline]
    pub const fn coefficients(&self) -> &[f64; 8] {
        &self.coeffs
    }

    /// Signed blade multiplication in local coefficient order. Entry `[lhs][rhs]`
    /// is `(output_slot, sign)`, where `sign` is +1 or −1. Backends generating
    /// shaders must use these slots, not bit-mask order.
    pub const PRODUCT_TABLE: [[(usize, i8); 8]; 8] = {
        let mut table = [[(0, 1); 8]; 8];
        let mut i = 0;
        while i < 8 {
            let mut j = 0;
            while j < 8 {
                let a = Self::MASK[i];
                let b = Self::MASK[j];
                // Moving each right generator past larger left generators.
                let swaps = ((a >> 1) & b).count_ones() + ((a >> 2) & b).count_ones();
                table[i][j] = (Self::MASK[a ^ b], if swaps % 2 == 0 { 1 } else { -1 });
                j += 1;
            }
            i += 1;
        }
        table
    };

    // Swapping e3 and e12 is its own inverse: slot -> mask and mask -> slot.
    const MASK: [usize; 8] = [0, 1, 2, 4, 3, 5, 6, 7];

    /// Geometric product, extended bilinearly from the Euclidean basis table.
    #[inline]
    pub fn geometric_product(&self, other: &Self) -> Self {
        let mut out = [0.; 8];
        for (i, row) in Self::PRODUCT_TABLE.iter().enumerate() {
            for (j, &(slot, sign)) in row.iter().enumerate() {
                out[slot] += f64::from(sign) * self.coeffs[i] * other.coeffs[j];
            }
        }
        Self::new(out)
    }

    /// Compute the reverse.
    ///
    /// Reverses the order of vectors in each blade:
    /// - Scalars and vectors: unchanged
    /// - Bivectors: negated
    /// - Trivector: negated
    #[inline]
    pub fn reverse(&self) -> Self {
        Self::new([
            self.coeffs[Self::S],
            self.coeffs[Self::E1],
            self.coeffs[Self::E2],
            self.coeffs[Self::E3],
            -self.coeffs[Self::E12],
            -self.coeffs[Self::E13],
            -self.coeffs[Self::E23],
            -self.coeffs[Self::E123],
        ])
    }

    /// Compute the conjugate (Clifford conjugate).
    ///
    /// Combines reversion and grade involution.
    #[inline]
    pub fn conjugate(&self) -> Self {
        Self::new([
            self.coeffs[Self::S],
            -self.coeffs[Self::E1],
            -self.coeffs[Self::E2],
            -self.coeffs[Self::E3],
            -self.coeffs[Self::E12],
            -self.coeffs[Self::E13],
            -self.coeffs[Self::E23],
            self.coeffs[Self::E123],
        ])
    }

    /// Compute the squared norm: x * x̃
    #[inline]
    pub fn norm_squared(&self) -> f64 {
        // For Cl(3,0,0), |x|² = <x x̃>₀
        let rev = self.reverse();
        self.geometric_product(&rev).coeffs[Self::S]
    }

    /// Compute the inverse.
    ///
    /// For Cl(3,0,0): x⁻¹ = x̃ / (x x̃)
    pub fn inverse(&self) -> Option<Self> {
        let norm_sq = self.norm_squared();
        if norm_sq.abs() < 1e-10 {
            return None;
        }

        let rev = self.reverse();
        let scale = 1.0 / norm_sq;
        Some(rev.scale(scale))
    }

    /// Scale by a scalar.
    #[inline]
    pub fn scale(&self, s: f64) -> Self {
        Self::new([
            self.coeffs[0] * s,
            self.coeffs[1] * s,
            self.coeffs[2] * s,
            self.coeffs[3] * s,
            self.coeffs[4] * s,
            self.coeffs[5] * s,
            self.coeffs[6] * s,
            self.coeffs[7] * s,
        ])
    }

    /// Add two elements.
    #[inline]
    pub fn add(&self, other: &Self) -> Self {
        Self::new([
            self.coeffs[0] + other.coeffs[0],
            self.coeffs[1] + other.coeffs[1],
            self.coeffs[2] + other.coeffs[2],
            self.coeffs[3] + other.coeffs[3],
            self.coeffs[4] + other.coeffs[4],
            self.coeffs[5] + other.coeffs[5],
            self.coeffs[6] + other.coeffs[6],
            self.coeffs[7] + other.coeffs[7],
        ])
    }

    /// Subtract two elements.
    #[inline]
    pub fn sub(&self, other: &Self) -> Self {
        Self::new([
            self.coeffs[0] - other.coeffs[0],
            self.coeffs[1] - other.coeffs[1],
            self.coeffs[2] - other.coeffs[2],
            self.coeffs[3] - other.coeffs[3],
            self.coeffs[4] - other.coeffs[4],
            self.coeffs[5] - other.coeffs[5],
            self.coeffs[6] - other.coeffs[6],
            self.coeffs[7] - other.coeffs[7],
        ])
    }

    /// Compute the dot product (scalar part of geometric product).
    #[inline]
    pub fn dot(&self, other: &Self) -> f64 {
        self.geometric_product(other).coeffs[Self::S]
    }

    /// Create a random element with unit norm.
    pub fn random_unit() -> Self {
        let mut coeffs = [0.0; 8];
        let mut norm_sq = 0.0;

        for c in &mut coeffs {
            *c = fastrand::f64() * 2.0 - 1.0;
            norm_sq += *c * *c;
        }

        if norm_sq > 1e-10 {
            let scale = 1.0 / norm_sq.sqrt();
            for c in &mut coeffs {
                *c *= scale;
            }
        }

        Self::new(coeffs)
    }

    /// Create a random versor (product of vectors).
    pub fn random_versor(num_factors: usize) -> Self {
        if num_factors == 0 {
            return Self::one();
        }

        let mut result = Self::random_unit_vector();
        for _ in 1..num_factors {
            let v = Self::random_unit_vector();
            result = result.geometric_product(&v);
        }
        result
    }

    /// Create a random unit vector.
    pub fn random_unit_vector() -> Self {
        let x = fastrand::f64() * 2.0 - 1.0;
        let y = fastrand::f64() * 2.0 - 1.0;
        let z = fastrand::f64() * 2.0 - 1.0;
        let norm = (x * x + y * y + z * z).sqrt();

        if norm > 1e-10 {
            Self::vector(x / norm, y / norm, z / norm)
        } else {
            Self::vector(1.0, 0.0, 0.0)
        }
    }

    /// Compute the dual (multiply by pseudoscalar inverse).
    #[inline]
    pub fn dual(&self) -> Self {
        // In Cl(3), I = e123, I² = -1, I⁻¹ = -I
        // dual(x) = x * I⁻¹ = -x * I
        self.geometric_product(&Self::pseudoscalar(-1.0))
    }

    /// Project onto grade k.
    pub fn grade_project(&self, grade: usize) -> Self {
        let mut result = [0.0; 8];
        match grade {
            0 => result[Self::S] = self.coeffs[Self::S],
            1 => {
                result[Self::E1] = self.coeffs[Self::E1];
                result[Self::E2] = self.coeffs[Self::E2];
                result[Self::E3] = self.coeffs[Self::E3];
            }
            2 => {
                result[Self::E12] = self.coeffs[Self::E12];
                result[Self::E13] = self.coeffs[Self::E13];
                result[Self::E23] = self.coeffs[Self::E23];
            }
            3 => result[Self::E123] = self.coeffs[Self::E123],
            _ => {}
        }
        Self::new(result)
    }

    /// Get the grade spectrum (norm of each grade).
    pub fn grade_spectrum(&self) -> Vec<f64> {
        let a = &self.coeffs;
        alloc::vec![
            a[Self::S].abs(),
            (a[Self::E1].powi(2) + a[Self::E2].powi(2) + a[Self::E3].powi(2)).sqrt(),
            (a[Self::E12].powi(2) + a[Self::E13].powi(2) + a[Self::E23].powi(2)).sqrt(),
            a[Self::E123].abs(),
        ]
    }

    /// Cyclic permutation of vector indices.
    ///
    /// This permutes e1 -> e2 -> e3 -> e1 and extends to higher grades.
    pub fn permute_cyclic(&self) -> Self {
        let a = &self.coeffs;
        Self::new([
            a[Self::S],
            a[Self::E3],   // e1 <- e3
            a[Self::E1],   // e2 <- e1
            a[Self::E2],   // e3 <- e2
            a[Self::E23],  // e12 <- e23
            a[Self::E12],  // e13 <- e12
            a[Self::E13],  // e23 <- e13
            a[Self::E123], // e123 unchanged
        ])
    }
}

// ============================================================================
// BindingAlgebra Implementation
// ============================================================================

impl BindingAlgebra for Cl3 {
    fn dimension(&self) -> usize {
        Self::DIMENSION
    }

    fn identity() -> Self {
        Self::one()
    }

    fn zero() -> Self {
        Self::new_zero()
    }

    fn bind(&self, other: &Self) -> Self {
        self.geometric_product(other)
    }

    fn inverse(&self) -> AlgebraResult<Self> {
        Self::inverse(self).ok_or_else(|| AlgebraError::NotInvertible {
            reason: "norm squared too small".into(),
        })
    }

    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self> {
        if beta.is_infinite() {
            // Hard bundling
            let self_norm = self.norm_squared().abs().sqrt();
            let other_norm = other.norm_squared().abs().sqrt();
            if self_norm >= other_norm {
                Ok(*self)
            } else {
                Ok(*other)
            }
        } else {
            // Soft bundling: weighted average
            let self_norm = self.norm_squared().abs().sqrt();
            let other_norm = other.norm_squared().abs().sqrt();

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

    fn similarity(&self, other: &Self) -> f64 {
        let self_norm = self.norm_squared().abs().sqrt();
        let other_norm = other.norm_squared().abs().sqrt();

        if self_norm < 1e-10 || other_norm < 1e-10 {
            return 0.0;
        }

        // <A B̃>₀ / (|A| |B|)
        let inner = self.geometric_product(&other.reverse()).coeffs[Self::S];
        inner / (self_norm * other_norm)
    }

    fn norm(&self) -> f64 {
        self.norm_squared().abs().sqrt()
    }

    fn normalize(&self) -> AlgebraResult<Self> {
        let n = self.norm();
        if n < 1e-10 {
            return Err(AlgebraError::NormalizationFailed { norm: n });
        }
        Ok(self.scale(1.0 / n))
    }

    fn permute(&self, shift: i32) -> Self {
        // Apply cyclic permutation `shift` times
        let shift = shift.rem_euclid(3);
        let mut result = *self;
        for _ in 0..shift {
            result = result.permute_cyclic();
        }
        result
    }

    fn get(&self, index: usize) -> AlgebraResult<f64> {
        if index >= 8 {
            return Err(AlgebraError::IndexOutOfBounds { index, size: 8 });
        }
        Ok(self.coeffs[index])
    }

    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()> {
        if index >= 8 {
            return Err(AlgebraError::IndexOutOfBounds { index, size: 8 });
        }
        self.coeffs[index] = value;
        Ok(())
    }

    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self> {
        if coeffs.len() != 8 {
            return Err(AlgebraError::DimensionMismatch {
                expected: 8,
                actual: coeffs.len(),
            });
        }
        let mut arr = [0.0; 8];
        arr.copy_from_slice(coeffs);
        Ok(Self::new(arr))
    }

    fn to_coefficients(&self) -> Vec<f64> {
        self.coeffs.to_vec()
    }

    fn algebra_name() -> &'static str {
        "Cl3"
    }
}

// ============================================================================
// GeometricAlgebra Implementation
// ============================================================================

impl GeometricAlgebra for Cl3 {
    fn max_grade(&self) -> usize {
        3
    }

    fn grade_project(&self, grade: usize) -> Self {
        Self::grade_project(self, grade)
    }

    fn reverse(&self) -> Self {
        Self::reverse(self)
    }

    fn grade_spectrum(&self) -> Vec<f64> {
        Self::grade_spectrum(self)
    }

    fn dual(&self) -> Self {
        Self::dual(self)
    }

    fn inner_product(&self, other: &Self) -> Self {
        // Inner product: sum of grade contractions
        // Simplified: just return scalar and vector parts
        let prod = self.geometric_product(other);
        let mut result = [0.0; 8];
        result[Self::S] = prod.coeffs[Self::S];
        result[Self::E1] = prod.coeffs[Self::E1];
        result[Self::E2] = prod.coeffs[Self::E2];
        result[Self::E3] = prod.coeffs[Self::E3];
        Self::new(result)
    }

    fn outer_product(&self, other: &Self) -> Self {
        // Outer product: keep only terms that increase grade
        // Simplified implementation
        let prod = self.geometric_product(other);
        let mut result = [0.0; 8];
        result[Self::E12] = prod.coeffs[Self::E12];
        result[Self::E13] = prod.coeffs[Self::E13];
        result[Self::E23] = prod.coeffs[Self::E23];
        result[Self::E123] = prod.coeffs[Self::E123];
        Self::new(result)
    }
}

// ============================================================================
// Standard Traits
// ============================================================================

impl Default for Cl3 {
    fn default() -> Self {
        Self::new_zero()
    }
}

impl core::ops::Add for Cl3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Cl3::add(&self, &other)
    }
}

impl core::ops::Sub for Cl3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Cl3::sub(&self, &other)
    }
}

impl core::ops::Mul for Cl3 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        self.geometric_product(&other)
    }
}

impl core::ops::Mul<f64> for Cl3 {
    type Output = Self;

    fn mul(self, scalar: f64) -> Self {
        self.scale(scalar)
    }
}

impl core::ops::Neg for Cl3 {
    type Output = Self;

    fn neg(self) -> Self {
        self.scale(-1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cl3_basis_squares() {
        // e1² = e2² = e3² = 1
        let e1 = Cl3::unit_vector(0);
        let e2 = Cl3::unit_vector(1);
        let e3 = Cl3::unit_vector(2);

        let e1_sq = e1.geometric_product(&e1);
        let e2_sq = e2.geometric_product(&e2);
        let e3_sq = e3.geometric_product(&e3);

        assert!((e1_sq.coeff(0) - 1.0).abs() < 1e-10);
        assert!((e2_sq.coeff(0) - 1.0).abs() < 1e-10);
        assert!((e3_sq.coeff(0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cl3_anticommutation() {
        // e1*e2 = -e2*e1 = e12
        let e1 = Cl3::unit_vector(0);
        let e2 = Cl3::unit_vector(1);

        let e12 = e1.geometric_product(&e2);
        let e21 = e2.geometric_product(&e1);

        assert!((e12.coeff(4) - 1.0).abs() < 1e-10); // e12 coefficient
        assert!((e21.coeff(4) + 1.0).abs() < 1e-10); // -e12
    }

    #[test]
    fn test_cl3_inverse() {
        let a = Cl3::random_versor(2);
        let a_inv = a.inverse().expect("versor should be invertible");
        let product = a.geometric_product(&a_inv);

        // a * a^-1 should be close to 1
        assert!((product.coeff(0) - 1.0).abs() < 1e-8);
        for i in 1..8 {
            assert!(product.coeff(i).abs() < 1e-8);
        }
    }

    #[test]
    fn test_cl3_reverse() {
        let e12 = Cl3::bivector(1.0, 0.0, 0.0);
        let rev = e12.reverse();

        assert!((rev.coeff(4) + 1.0).abs() < 1e-10); // e12 reversed is -e12
    }

    #[test]
    fn test_cl3_binding_identity() {
        let a = Cl3::random_versor(1);
        let identity = Cl3::identity();
        let bound = a.bind(&identity);

        let sim = a.similarity(&bound);
        assert!(sim > 0.99, "similarity with identity: {}", sim);
    }

    #[test]
    fn test_cl3_pseudoscalar() {
        let e1 = Cl3::unit_vector(0);
        let e2 = Cl3::unit_vector(1);
        let e3 = Cl3::unit_vector(2);

        // e123 = e1 * e2 * e3
        let e123 = e1.geometric_product(&e2).geometric_product(&e3);
        assert!((e123.coeff(7) - 1.0).abs() < 1e-10);

        // e123² = -1
        let e123_sq = e123.geometric_product(&e123);
        assert!((e123_sq.coeff(0) + 1.0).abs() < 1e-10);
    }
}
