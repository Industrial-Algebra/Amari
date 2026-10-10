//! Clifford algebra implementation using amari-core::Multivector.
//!
//! This module wraps the existing `Multivector<P, Q, R>` type to implement
//! the `BindingAlgebra` trait, enabling its use in holographic memory and
//! vector symbolic architecture applications.
//!
//! # Signature
//!
//! The Clifford algebra `Cl(P, Q, R)` has:
//! - P basis vectors with e_i^2 = +1
//! - Q basis vectors with e_i^2 = -1
//! - R basis vectors with e_i^2 = 0 (degenerate)
//!
//! For holographic memory, we typically use `Cl(n, 0, 0)` (positive-definite).

use alloc::string::ToString;
use alloc::vec::Vec;

use amari_core::Multivector;

use super::{AlgebraError, AlgebraResult, BindingAlgebra, GeometricAlgebra};

/// Clifford algebra element wrapping `amari_core::Multivector`.
///
/// This provides a `BindingAlgebra` implementation for general Clifford algebras,
/// using the geometric product as the binding operation.
///
/// # Type Parameters
///
/// - `P`: Number of positive signature basis vectors
/// - `Q`: Number of negative signature basis vectors
/// - `R`: Number of null/degenerate basis vectors
///
/// # Example
///
/// ```ignore
/// use amari_fusion::algebra::CliffordAlgebra;
///
/// // Cl(3, 0, 0) - 3D Euclidean geometric algebra
/// let a = CliffordAlgebra::<3, 0, 0>::from_coefficients(&[1.0, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0])?;
/// let b = CliffordAlgebra::<3, 0, 0>::from_coefficients(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])?;
///
/// // Binding via geometric product
/// let bound = a.bind(&b);
///
/// // Unbinding to recover b
/// let recovered = a.unbind(&bound)?;
/// ```
#[derive(Clone, Debug)]
pub struct CliffordAlgebra<const P: usize, const Q: usize, const R: usize> {
    /// The underlying multivector from amari-core
    inner: Multivector<P, Q, R>,
}

impl<const P: usize, const Q: usize, const R: usize> CliffordAlgebra<P, Q, R> {
    /// The total vector space dimension (n = P + Q + R).
    pub const VECTOR_DIM: usize = P + Q + R;

    /// The algebra dimension (2^n basis elements).
    pub const ALGEBRA_DIM: usize = 1 << Self::VECTOR_DIM;

    /// Create a new Clifford algebra element from a multivector.
    pub fn new(inner: Multivector<P, Q, R>) -> Self {
        Self { inner }
    }

    /// Get a reference to the underlying multivector.
    pub fn inner(&self) -> &Multivector<P, Q, R> {
        &self.inner
    }

    /// Get a mutable reference to the underlying multivector.
    pub fn inner_mut(&mut self) -> &mut Multivector<P, Q, R> {
        &mut self.inner
    }

    /// Consume self and return the underlying multivector.
    pub fn into_inner(self) -> Multivector<P, Q, R> {
        self.inner
    }

    /// Create a scalar element.
    pub fn scalar(value: f64) -> Self {
        Self::new(Multivector::scalar(value))
    }

    /// Create a unit vector in direction `i`.
    ///
    /// Returns `None` if `i >= P + Q + R`.
    pub fn unit_vector(i: usize) -> Option<Self> {
        if i >= Self::VECTOR_DIM {
            return None;
        }

        let mut coeffs = alloc::vec![0.0; Self::ALGEBRA_DIM];
        let index = 1 << i;
        if index < coeffs.len() {
            coeffs[index] = 1.0;
        }
        Some(Self::new(Multivector::from_coefficients(coeffs)))
    }

    /// Create a random unit versor (product of unit vectors).
    ///
    /// Versors are always invertible, making them ideal for binding keys.
    pub fn random_versor(num_factors: usize) -> Self {
        if num_factors == 0 {
            return Self::scalar(1.0);
        }

        // Start with a random unit vector
        let mut result = Self::random_unit_vector();

        // Multiply by additional random unit vectors
        for _ in 1..num_factors {
            let v = Self::random_unit_vector();
            result = Self::new(result.inner.geometric_product(&v.inner));
        }

        result
    }

    /// Create a random unit vector.
    fn random_unit_vector() -> Self {
        let mut coeffs = alloc::vec![0.0; Self::ALGEBRA_DIM];
        let mut norm_sq = 0.0;

        for i in 0..Self::VECTOR_DIM {
            let index = 1 << i;
            if index < coeffs.len() {
                let val = (fastrand::f64() - 0.5) * 2.0;
                coeffs[index] = val;
                norm_sq += val * val;
            }
        }

        // Normalize
        if norm_sq > 1e-10 {
            let scale = 1.0 / norm_sq.sqrt();
            for i in 0..Self::VECTOR_DIM {
                let index = 1 << i;
                if index < coeffs.len() {
                    coeffs[index] *= scale;
                }
            }
        }

        Self::new(Multivector::from_coefficients(coeffs))
    }
}

// ============================================================================
// BindingAlgebra Implementation
// ============================================================================

impl<const P: usize, const Q: usize, const R: usize> BindingAlgebra for CliffordAlgebra<P, Q, R> {
    fn dimension(&self) -> usize {
        Self::ALGEBRA_DIM
    }

    fn identity() -> Self {
        Self::scalar(1.0)
    }

    fn zero() -> Self {
        Self::new(Multivector::zero())
    }

    fn bind(&self, other: &Self) -> Self {
        Self::new(self.inner.geometric_product(&other.inner))
    }

    fn inverse(&self) -> AlgebraResult<Self> {
        self.inner
            .inverse()
            .map(Self::new)
            .ok_or_else(|| AlgebraError::NotInvertible {
                reason: "norm too small for inversion".to_string(),
            })
    }

    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self> {
        if beta.is_infinite() {
            // Hard bundling: winner-take-all
            let self_norm = self.inner.coefficient_norm();
            let other_norm = other.inner.coefficient_norm();
            if self_norm >= other_norm {
                Ok(self.clone())
            } else {
                Ok(other.clone())
            }
        } else {
            // Soft bundling: weighted average
            let self_norm = self.inner.coefficient_norm();
            let other_norm = other.inner.coefficient_norm();

            let (w1, w2) = if beta <= 0.0 || (self_norm < 1e-10 && other_norm < 1e-10) {
                (0.5, 0.5)
            } else {
                let max_norm = self_norm.max(other_norm);
                let exp1 = (beta * (self_norm - max_norm)).exp();
                let exp2 = (beta * (other_norm - max_norm)).exp();
                let sum = exp1 + exp2;
                (exp1 / sum, exp2 / sum)
            };

            let result = self.inner.clone() * w1 + other.inner.clone() * w2;
            Ok(Self::new(result))
        }
    }

    fn similarity(&self, other: &Self) -> f64 {
        let self_norm = self.inner.coefficient_norm();
        let other_norm = other.inner.coefficient_norm();

        if self_norm < 1e-10 || other_norm < 1e-10 {
            return 0.0;
        }

        // Use scalar product with reverse: <A B̃>₀
        let inner = self.inner.scalar_product(&other.inner.reverse());
        inner / (self_norm * other_norm)
    }

    fn norm(&self) -> f64 {
        self.inner.coefficient_norm()
    }

    fn normalize(&self) -> AlgebraResult<Self> {
        let n = self.inner.coefficient_norm();
        (n > 0.0)
            .then(|| Self::new(self.inner.clone() / n))
            .ok_or(AlgebraError::NormalizationFailed { norm: n })
    }

    fn permute(&self, shift: i32) -> Self {
        // Cyclic permutation of coefficients
        let coeffs = self.inner.to_vec();
        let n = coeffs.len();
        if n == 0 {
            return self.clone();
        }

        let shift = ((shift % n as i32) + n as i32) as usize % n;
        let mut new_coeffs = alloc::vec![0.0; n];

        for i in 0..n {
            let new_i = (i + shift) % n;
            new_coeffs[new_i] = coeffs[i];
        }

        Self::new(Multivector::from_coefficients(new_coeffs))
    }

    fn get(&self, index: usize) -> AlgebraResult<f64> {
        if index >= Self::ALGEBRA_DIM {
            return Err(AlgebraError::IndexOutOfBounds {
                index,
                size: Self::ALGEBRA_DIM,
            });
        }
        Ok(self.inner.get(index))
    }

    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()> {
        let n = Self::ALGEBRA_DIM;
        if index >= n {
            return Err(AlgebraError::IndexOutOfBounds { index, size: n });
        }

        self.inner.set(index, value);
        Ok(())
    }

    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self> {
        if coeffs.len() != Self::ALGEBRA_DIM {
            return Err(AlgebraError::DimensionMismatch {
                expected: Self::ALGEBRA_DIM,
                actual: coeffs.len(),
            });
        }
        Ok(Self::new(Multivector::from_coefficients(coeffs.to_vec())))
    }

    fn to_coefficients(&self) -> Vec<f64> {
        self.inner.to_vec()
    }

    fn algebra_name() -> &'static str {
        // Can't easily format with const generics, so use a generic name
        "Clifford"
    }

    fn theoretical_capacity(&self) -> usize {
        // Capacity is O(dim / ln(dim)) where dim = 2^n
        let dim = Self::ALGEBRA_DIM as f64;
        if dim <= 1.0 {
            return 1;
        }
        (dim / dim.ln()).max(1.0) as usize
    }
}

// ============================================================================
// GeometricAlgebra Implementation
// ============================================================================

impl<const P: usize, const Q: usize, const R: usize> GeometricAlgebra for CliffordAlgebra<P, Q, R> {
    fn max_grade(&self) -> usize {
        Self::VECTOR_DIM
    }

    fn grade_project(&self, grade: usize) -> Self {
        Self::new(self.inner.grade_project(grade))
    }

    fn reverse(&self) -> Self {
        Self::new(self.inner.reverse())
    }

    fn grade_spectrum(&self) -> Vec<f64> {
        self.inner.grade_spectrum()
    }

    fn dual(&self) -> Self {
        // The dual (Hodge star) multiplies by the pseudoscalar inverse
        // For Cl(n,0,0), the pseudoscalar is e_{12...n} and I^2 = (-1)^(n(n-1)/2)
        // Simplified implementation: just use geometric product with pseudoscalar
        let mut pseudo_coeffs = alloc::vec![0.0; Self::ALGEBRA_DIM];
        if Self::ALGEBRA_DIM > 0 {
            // The pseudoscalar is the last basis element
            pseudo_coeffs[Self::ALGEBRA_DIM - 1] = 1.0;
        }
        let pseudoscalar = Multivector::from_coefficients(pseudo_coeffs);
        Self::new(self.inner.geometric_product(&pseudoscalar))
    }

    fn inner_product(&self, other: &Self) -> Self {
        Self::new(self.inner.inner_product(&other.inner))
    }

    fn outer_product(&self, other: &Self) -> Self {
        Self::new(self.inner.outer_product(&other.inner))
    }
}

// ============================================================================
// Type Aliases for Common Algebras
// ============================================================================

/// 2D Euclidean Clifford algebra Cl(2,0,0).
pub type Cl2 = CliffordAlgebra<2, 0, 0>;

/// 3D Euclidean Clifford algebra Cl(3,0,0).
pub type Cl3Full = CliffordAlgebra<3, 0, 0>;

/// 4D Euclidean Clifford algebra Cl(4,0,0).
pub type Cl4 = CliffordAlgebra<4, 0, 0>;

/// 8D Euclidean Clifford algebra Cl(8,0,0).
pub type Cl8 = CliffordAlgebra<8, 0, 0>;

/// Spacetime algebra Cl(1,3,0).
pub type Spacetime = CliffordAlgebra<1, 3, 0>;

/// Projective geometric algebra Cl(3,0,1).
pub type PGA = CliffordAlgebra<3, 0, 1>;

/// Conformal geometric algebra Cl(4,1,0).
pub type CGA = CliffordAlgebra<4, 1, 0>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clifford_identity() {
        let identity = Cl3Full::identity();
        assert!((identity.get(0).unwrap() - 1.0).abs() < 1e-10);

        for i in 1..8 {
            assert!(identity.get(i).unwrap().abs() < 1e-10);
        }
    }

    #[test]
    fn test_clifford_binding_identity() {
        let a = Cl3Full::random_versor(1);
        let identity = Cl3Full::identity();
        let bound = a.bind(&identity);

        // a * identity should equal a
        let sim = a.similarity(&bound);
        assert!(sim > 0.99, "similarity with identity binding: {}", sim);
    }

    #[test]
    fn test_clifford_inverse() {
        let a = Cl3Full::random_versor(2);
        let a_inv = a.inverse().expect("versor should be invertible");
        let product = a.bind(&a_inv);

        // a * a^-1 should be close to identity
        let identity = Cl3Full::identity();
        let sim = product.similarity(&identity);
        assert!(sim > 0.99, "inverse product similarity: {}", sim);
    }

    #[test]
    fn test_clifford_dissimilarity() {
        let a = Cl3Full::random_versor(1);
        let b = Cl3Full::random_versor(1);
        let bound = a.bind(&b);

        // bound should be dissimilar to both a and b
        let sim_a = bound.similarity(&a).abs();
        let sim_b = bound.similarity(&b).abs();

        // In high dimensions, similarity should be low (close to 0)
        // For Cl(3), dimension is only 8, so we're more lenient
        assert!(sim_a < 0.8, "similarity with a: {}", sim_a);
        assert!(sim_b < 0.8, "similarity with b: {}", sim_b);
    }

    #[test]
    fn test_clifford_bundling() {
        let a = Cl3Full::random_versor(1);
        let b = Cl3Full::random_versor(1);
        let bundled = a.bundle(&b, 1.0).expect("bundling should succeed");

        // bundled should be similar to both a and b
        let sim_a = bundled.similarity(&a);
        let sim_b = bundled.similarity(&b);

        // For soft bundling, result should have some similarity to both
        assert!(
            sim_a > 0.0 || sim_b > 0.0,
            "bundled should have some similarity"
        );
    }

    #[test]
    fn test_clifford_capacity() {
        let a = Cl3Full::identity();
        let capacity = a.theoretical_capacity();

        // Cl(3) has dimension 8, capacity should be 8/ln(8) ≈ 3.8 → 3
        assert!(capacity >= 3 && capacity <= 5, "capacity: {}", capacity);
    }
}
