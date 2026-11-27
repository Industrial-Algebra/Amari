//! Product Clifford algebra Cl(3,0,0)^K for linear dimension scaling.
//!
//! This module implements the key innovation for high-dimensional holographic
//! memory: using a product of K copies of Cl(3,0,0) instead of a single Cl(n,0,0).
//!
//! # Motivation
//!
//! For holographic memory with reliable retrieval, we need high dimensions.
//! But Clifford algebra dimensions grow exponentially: Cl(n) has 2^n basis elements.
//!
//! | n | dim(Cl(n)) | Compute per product |
//! |---|------------|---------------------|
//! | 8 | 256        | 65,536              |
//! | 10| 1,024      | 1,048,576           |
//! | 12| 4,096      | 16,777,216          |
//!
//! Instead, we use Cl(3)^K = Cl(3) × Cl(3) × ... × Cl(3) (K copies):
//!
//! | K | dim | Compute per product |
//! |---|-----|---------------------|
//! | 32| 256 | 32 × 64 = 2,048     |
//! | 128| 1,024 | 128 × 64 = 8,192  |
//! | 512| 4,096 | 512 × 64 = 32,768 |
//!
//! This gives linear compute scaling O(64K) instead of O(4^n).
//!
//! # Operations
//!
//! For x = (x₁, x₂, ..., xₖ) and y = (y₁, y₂, ..., yₖ):
//!
//! - **Binding**: x ⊛ y = (x₁y₁, x₂y₂, ..., xₖyₖ)
//! - **Inverse**: x⁻¹ = (x₁⁻¹, x₂⁻¹, ..., xₖ⁻¹)
//! - **Bundling**: x ⊕ y = (x₁+y₁, x₂+y₂, ..., xₖ+yₖ) with normalization
//! - **Similarity**: (Σᵢ <xᵢ, yᵢ>) / (K × |x| × |y|)
//!
//! # Capacity
//!
//! The theoretical capacity is O(8K / ln(8K)), identical to FHRR at dimension 8K.

use alloc::vec::Vec;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use super::cl3::Cl3;
use super::{AlgebraError, AlgebraResult, BindingAlgebra};

/// Product Clifford algebra Cl(3,0,0)^K.
///
/// This represents a K-tuple of Cl3 elements, where operations are
/// performed component-wise. This provides linear dimension scaling
/// while maintaining the algebraic properties of Clifford algebras.
///
/// # Type Parameter
///
/// - `K`: Number of Cl3 factors
///
/// # Example
///
/// ```ignore
/// use amari_fusion::algebra::ProductCliffordAlgebra;
///
/// // Create a 256-dimensional space using 32 Cl3 factors
/// let a = ProductCliffordAlgebra::<32>::random_versor(2);
/// let b = ProductCliffordAlgebra::<32>::random_versor(2);
///
/// // Binding is component-wise geometric product
/// let bound = a.bind(&b);
///
/// // Unbinding recovers (approximately) the original
/// let recovered = a.unbind(&bound)?;
/// ```
#[derive(Clone, Debug)]
pub struct ProductCliffordAlgebra<const K: usize> {
    /// The K components, each a Cl3 element
    components: [Cl3; K],
}

impl<const K: usize> ProductCliffordAlgebra<K> {
    /// Total dimension: 8K
    pub const DIMENSION: usize = 8 * K;

    /// Create a new product algebra element from components.
    pub fn new(components: [Cl3; K]) -> Self {
        Self { components }
    }

    /// Create from a Vec of Cl3 components.
    ///
    /// Returns `Err` if the Vec length doesn't match K.
    pub fn from_vec(components: Vec<Cl3>) -> AlgebraResult<Self> {
        if components.len() != K {
            return Err(AlgebraError::DimensionMismatch {
                expected: K,
                actual: components.len(),
            });
        }

        // Convert Vec to array
        let mut arr = [Cl3::new_zero(); K];
        for (i, c) in components.into_iter().enumerate() {
            arr[i] = c;
        }
        Ok(Self::new(arr))
    }

    /// Get a reference to component i.
    pub fn component(&self, i: usize) -> Option<&Cl3> {
        self.components.get(i)
    }

    /// Get a mutable reference to component i.
    pub fn component_mut(&mut self, i: usize) -> Option<&mut Cl3> {
        self.components.get_mut(i)
    }

    /// Get all components.
    pub fn components(&self) -> &[Cl3; K] {
        &self.components
    }

    /// Create the product identity (each component is scalar 1).
    pub fn product_identity() -> Self {
        Self::new([Cl3::one(); K])
    }

    /// Create the product zero (each component is zero).
    pub fn product_zero() -> Self {
        Self::new([Cl3::new_zero(); K])
    }

    /// Create a random versor (each component is a random versor).
    pub fn random_versor(num_factors: usize) -> Self {
        let mut components = [Cl3::new_zero(); K];
        for c in &mut components {
            *c = Cl3::random_versor(num_factors);
        }
        Self::new(components)
    }

    /// Create a random unit element.
    pub fn random_unit() -> Self {
        let mut components = [Cl3::new_zero(); K];
        for c in &mut components {
            *c = Cl3::random_unit();
        }
        // Normalize the entire structure
        let result = Self::new(components);
        result.normalize().unwrap_or(result)
    }

    /// Component-wise geometric product (binding).
    pub fn component_product(&self, other: &Self) -> Self {
        let mut result = [Cl3::new_zero(); K];
        for i in 0..K {
            result[i] = self.components[i].geometric_product(&other.components[i]);
        }
        Self::new(result)
    }

    /// Component-wise addition.
    pub fn component_add(&self, other: &Self) -> Self {
        let mut result = [Cl3::new_zero(); K];
        for i in 0..K {
            result[i] = self.components[i].add(&other.components[i]);
        }
        Self::new(result)
    }

    /// Component-wise scaling.
    pub fn component_scale(&self, scalar: f64) -> Self {
        let mut result = [Cl3::new_zero(); K];
        for i in 0..K {
            result[i] = self.components[i].scale(scalar);
        }
        Self::new(result)
    }

    /// Compute the squared norm (sum of component squared norms).
    pub fn norm_squared(&self) -> f64 {
        self.components.iter().map(|c| c.norm_squared()).sum()
    }

    /// Compute the norm.
    pub fn compute_norm(&self) -> AlgebraResult<f64> {
        self.norm()
    }

    /// Compute component-wise inverse.
    pub fn component_inverse(&self) -> Option<Self> {
        let mut result = [Cl3::new_zero(); K];
        for i in 0..K {
            result[i] = self.components[i].inverse()?;
        }
        Some(Self::new(result))
    }

    /// Compute the inner product (sum of component inner products).
    pub fn inner_product(&self, other: &Self) -> f64 {
        self.components
            .iter()
            .zip(other.components.iter())
            .map(|(a, b)| {
                let prod = a.geometric_product(&b.reverse());
                prod.coeff(0) // Scalar part
            })
            .sum()
    }

    /// Apply cyclic permutation to each component.
    pub fn permute_components(&self, shift: i32) -> Self {
        let mut result = [Cl3::new_zero(); K];
        for i in 0..K {
            result[i] = self.components[i].permute(shift);
        }
        Self::new(result)
    }

    /// Shuffle components (permute which component goes where).
    ///
    /// This is a different operation from permuting within each Cl3.
    /// It reorders the K components themselves.
    pub fn shuffle_components(&self, permutation: &[usize; K]) -> Self {
        let mut result = [Cl3::new_zero(); K];
        for (i, &p) in permutation.iter().enumerate() {
            if p < K {
                result[i] = self.components[p];
            }
        }
        Self::new(result)
    }

    /// Create a default cyclic permutation of components.
    fn default_component_permutation(shift: i32) -> [usize; K] {
        let shift = ((shift % K as i32) + K as i32) as usize % K;
        let mut perm = [0usize; K];
        for i in 0..K {
            perm[i] = (i + shift) % K;
        }
        perm
    }
}

// ============================================================================
// Parallel Operations (when rayon feature is enabled)
// ============================================================================

#[cfg(feature = "rayon")]
impl<const K: usize> ProductCliffordAlgebra<K>
where
    [Cl3; K]: Send + Sync,
{
    /// Parallel component-wise geometric product.
    pub fn par_component_product(&self, other: &Self) -> Self {
        let result: Vec<Cl3> = self
            .components
            .par_iter()
            .zip(other.components.par_iter())
            .map(|(a, b)| a.geometric_product(b))
            .collect();

        Self::from_vec(result).expect("parallel product should preserve dimension")
    }

    /// Parallel bundling of multiple elements.
    pub fn par_bundle_all(items: &[Self], beta: f64) -> AlgebraResult<Self> {
        if items.is_empty() {
            return Ok(Self::product_zero());
        }
        if items.len() == 1 {
            return Ok(items[0].clone());
        }

        items
            .par_iter()
            .cloned()
            .reduce(Self::product_zero, |a, b| a.bundle(&b, beta).unwrap_or(a))
            .pipe(Ok)
    }
}

// Helper trait for method chaining
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R;
}

impl<T> Pipe for T {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

// ============================================================================
// BindingAlgebra Implementation
// ============================================================================

impl<const K: usize> BindingAlgebra for ProductCliffordAlgebra<K> {
    fn dimension(&self) -> usize {
        Self::DIMENSION
    }

    fn identity() -> Self {
        Self::product_identity()
    }

    fn zero() -> Self {
        Self::product_zero()
    }

    fn bind(&self, other: &Self) -> Self {
        self.component_product(other)
    }

    fn inverse(&self) -> AlgebraResult<Self> {
        self.component_inverse()
            .ok_or_else(|| AlgebraError::NotInvertible {
                reason: "one or more components not invertible".into(),
            })
    }

    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self> {
        if beta.is_infinite() {
            // Hard bundling: winner-take-all
            let self_norm = self.compute_norm()?;
            let other_norm = other.compute_norm()?;
            if self_norm >= other_norm {
                Ok(self.clone())
            } else {
                Ok(other.clone())
            }
        } else {
            // Soft bundling: weighted average
            let self_norm = self.compute_norm()?;
            let other_norm = other.compute_norm()?;

            let (w1, w2) = if beta <= 0.0 || (self_norm < 1e-10 && other_norm < 1e-10) {
                (0.5, 0.5)
            } else {
                let max_norm = self_norm.max(other_norm);
                let exp1 = (beta * (self_norm - max_norm)).exp();
                let exp2 = (beta * (other_norm - max_norm)).exp();
                let sum = exp1 + exp2;
                (exp1 / sum, exp2 / sum)
            };

            Ok(self
                .component_scale(w1)
                .component_add(&other.component_scale(w2)))
        }
    }

    fn bundle_all(items: &[Self], beta: f64) -> AlgebraResult<Self> {
        if items.is_empty() {
            return Ok(Self::zero());
        }
        if items.len() == 1 {
            return Ok(items[0].clone());
        }

        // For product algebras, we can bundle more efficiently
        // by accumulating directly instead of pairwise
        if beta.is_infinite() {
            // Hard bundling: find element with maximum norm
            let mut best = items[0].clone();
            let mut best_norm = best.compute_norm()?;

            for item in items.iter().skip(1) {
                let norm = item.compute_norm()?;
                if norm > best_norm {
                    best = item.clone();
                    best_norm = norm;
                }
            }
            Ok(best)
        } else {
            // Soft bundling: compute weighted sum
            let mut result = items[0].clone();
            for item in items.iter().skip(1) {
                result = result.bundle(item, beta)?;
            }
            Ok(result)
        }
    }

    fn permute(&self, shift: i32) -> Self {
        // Combine two types of permutation:
        // 1. Permute within each Cl3 component
        // 2. Shuffle which component goes where
        let shuffled = self.shuffle_components(&Self::default_component_permutation(shift));
        shuffled.permute_components(shift)
    }

    fn get(&self, index: usize) -> AlgebraResult<f64> {
        if index >= Self::DIMENSION {
            return Err(AlgebraError::IndexOutOfBounds {
                index,
                size: Self::DIMENSION,
            });
        }

        let component_idx = index / 8;
        let local_idx = index % 8;
        Ok(self.components[component_idx].coeff(local_idx))
    }

    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()> {
        if index >= Self::DIMENSION {
            return Err(AlgebraError::IndexOutOfBounds {
                index,
                size: Self::DIMENSION,
            });
        }

        let component_idx = index / 8;
        let local_idx = index % 8;
        self.components[component_idx].set_coeff(local_idx, value);
        Ok(())
    }

    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self> {
        if coeffs.len() != Self::DIMENSION {
            return Err(AlgebraError::DimensionMismatch {
                expected: Self::DIMENSION,
                actual: coeffs.len(),
            });
        }

        let mut components = [Cl3::new_zero(); K];
        for i in 0..K {
            let start = i * 8;
            let mut arr = [0.0; 8];
            arr.copy_from_slice(&coeffs[start..start + 8]);
            components[i] = Cl3::new(arr);
        }
        Ok(Self::new(components))
    }

    fn to_coefficients(&self) -> Vec<f64> {
        let mut result = Vec::with_capacity(Self::DIMENSION);
        for c in &self.components {
            result.extend_from_slice(c.coefficients());
        }
        result
    }

    fn algebra_name() -> &'static str {
        "ProductClifford"
    }

    fn theoretical_capacity(&self) -> usize {
        // Capacity is O(8K / ln(8K))
        let dim = Self::DIMENSION as f64;
        if dim <= 1.0 {
            return 1;
        }
        (dim / dim.ln()).max(1.0) as usize
    }

    fn estimate_snr(&self, item_count: usize) -> f64 {
        if item_count == 0 {
            return f64::INFINITY;
        }
        // SNR scales as sqrt(8K / item_count)
        let dim = Self::DIMENSION as f64;
        (dim / item_count as f64).sqrt()
    }
}

// ============================================================================
// Type Aliases for Common Configurations
// ============================================================================

/// Cl(3)^4 = 32-dimensional product algebra
pub type ProductCl3x4 = ProductCliffordAlgebra<4>;

/// Cl(3)^8 = 64-dimensional product algebra
pub type ProductCl3x8 = ProductCliffordAlgebra<8>;

/// Cl(3)^16 = 128-dimensional product algebra
pub type ProductCl3x16 = ProductCliffordAlgebra<16>;

/// Cl(3)^32 = 256-dimensional product algebra
pub type ProductCl3x32 = ProductCliffordAlgebra<32>;

/// Cl(3)^64 = 512-dimensional product algebra
pub type ProductCl3x64 = ProductCliffordAlgebra<64>;

/// Cl(3)^128 = 1024-dimensional product algebra
pub type ProductCl3x128 = ProductCliffordAlgebra<128>;

/// Cl(3)^256 = 2048-dimensional product algebra
pub type ProductCl3x256 = ProductCliffordAlgebra<256>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_identity() {
        let identity = ProductCl3x4::product_identity();
        for i in 0..4 {
            let c = identity.component(i).unwrap();
            assert!((c.coeff(0) - 1.0).abs() < 1e-10);
            for j in 1..8 {
                assert!(c.coeff(j).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_product_binding_identity() {
        let a = ProductCl3x4::random_versor(2);
        let identity = ProductCl3x4::identity();
        let bound = a.bind(&identity);

        let sim = a.similarity(&bound).unwrap();
        assert!(sim > 0.99, "similarity with identity: {}", sim);
    }

    #[test]
    fn test_product_inverse() {
        let a = ProductCl3x8::random_versor(2);
        let a_inv = a.inverse().expect("versor should be invertible");
        let product = a.bind(&a_inv);

        // a * a^-1 should be close to identity
        let identity = ProductCl3x8::identity();
        let sim = product.similarity(&identity).unwrap();
        assert!(sim > 0.99, "inverse product similarity: {}", sim);
    }

    #[test]
    fn test_product_dimension() {
        let a = ProductCl3x16::random_unit();
        assert_eq!(a.dimension(), 128);

        let coeffs = a.to_coefficients();
        assert_eq!(coeffs.len(), 128);
    }

    #[test]
    fn test_product_dissimilarity() {
        let a = ProductCl3x8::random_versor(1);
        let b = ProductCl3x8::random_versor(1);
        let bound = a.bind(&b);

        // bound should be dissimilar to both a and b
        let sim_a = bound.similarity(&a).unwrap().abs();
        let sim_b = bound.similarity(&b).unwrap().abs();

        // With 64 dimensions, we expect low similarity
        assert!(sim_a < 0.5, "similarity with a: {}", sim_a);
        assert!(sim_b < 0.5, "similarity with b: {}", sim_b);
    }

    #[test]
    fn test_product_capacity() {
        let a = ProductCl3x32::random_unit();
        let capacity = a.theoretical_capacity();

        // 256 dimensions, capacity ≈ 256/ln(256) ≈ 46
        assert!(capacity > 40 && capacity < 55, "capacity: {}", capacity);
    }

    #[test]
    fn test_product_unbind_recover() {
        let key = ProductCl3x16::random_versor(2);
        let value = ProductCl3x16::random_versor(2);

        let bound = key.bind(&value);
        let recovered = key.unbind(&bound).expect("unbind should succeed");

        let sim = recovered.similarity(&value).unwrap();
        assert!(sim > 0.99, "recovery similarity: {}", sim);
    }

    #[test]
    fn test_product_bundling() {
        let a = ProductCl3x8::random_versor(1);
        let b = ProductCl3x8::random_versor(1);
        let bundled = a.bundle(&b, 1.0).expect("bundling should succeed");

        // bundled should have some similarity to both
        let sim_a = bundled.similarity(&a).unwrap();
        let sim_b = bundled.similarity(&b).unwrap();

        assert!(sim_a > 0.0 || sim_b > 0.0, "bundled should have similarity");
    }

    #[test]
    fn test_product_from_coefficients() {
        let coeffs: Vec<f64> = (0..64).map(|i| i as f64 * 0.1).collect();
        let a = ProductCl3x8::from_coefficients(&coeffs).expect("from_coefficients should succeed");

        let recovered = a.to_coefficients();
        for (orig, rec) in coeffs.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() < 1e-10);
        }
    }
}
