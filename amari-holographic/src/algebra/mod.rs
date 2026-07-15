//! Generalized binding algebras for Vector Symbolic Architectures.
//!
//! This module provides a trait-based abstraction for various algebras
//! supporting holographic reduced representations and vector symbolic architectures.
//!
//! # Supported Algebras
//!
//! - **Clifford**: General Clifford algebras `Cl(n,0,0)` using `amari-core::Multivector`
//! - **Cl3**: Optimized 3D Clifford algebra `Cl(3,0,0)` with unrolled operations
//! - **ProductClifford**: Product of Clifford algebras `Cl(3,0,0)^K` for linear scaling
//! - **FHRR**: Fourier Holographic Reduced Representation (frequency domain)
//! - **MAP**: Multiply-Add-Permute bipolar algebra
//!
//! # Architecture
//!
//! The key insight is that all these algebras share common operations:
//!
//! - **Binding** (`⊛`): Associate two representations (like key-value pairs)
//! - **Superposition** (`+`): Accumulate representations additively
//! - **Bundling** (`⊕`): Select or clean up representations with bounded magnitude
//! - **Inverse** (`⁻¹`): Enable unbinding to retrieve associated values
//! - **Similarity**: Measure closeness between representations
//!
//! The [`BindingAlgebra`] trait abstracts these operations, allowing
//! [`TropicalDual<A>`] to be generic over the underlying algebra.
//!
//! # Capacity Scaling
//!
//! Different algebras provide different capacity characteristics:
//!
//! | Algebra | Dimension | Compute | Capacity |
//! |---------|-----------|---------|----------|
//! | Clifford Cl(n) | 2^n | O(4^n) | O(2^n / n) |
//! | ProductClifford Cl(3)^K | 8K | O(64K) | O(8K / ln(8K)) |
//! | FHRR | D | O(D log D) | O(D / ln D) |
//! | MAP | D | O(D) | O(D / ln D) |
//!
//! ProductClifford and FHRR offer linear dimension scaling with O(D / ln D) capacity.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

pub mod cl3;
pub mod clifford;
pub mod fhrr;
pub mod map;
pub mod product_clifford;

#[cfg(test)]
mod tests;

// Re-exports
pub use cl3::Cl3;
pub use clifford::CliffordAlgebra;
pub use fhrr::FHRRAlgebra;
pub use map::MAPAlgebra;
pub use product_clifford::ProductCliffordAlgebra;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during algebra operations.
#[derive(Debug, Clone, PartialEq)]
pub enum AlgebraError {
    /// Element is not invertible (singular)
    NotInvertible { reason: String },

    /// Dimension mismatch between operands
    DimensionMismatch { expected: usize, actual: usize },

    /// Numerical computation failed
    NumericalError { operation: String, details: String },

    /// Invalid parameter value
    InvalidParameter {
        name: String,
        value: String,
        reason: String,
    },

    /// Operation not supported by this algebra
    UnsupportedOperation { algebra: String, operation: String },

    /// Normalization failed (zero or near-zero norm)
    NormalizationFailed { norm: f64 },

    /// Index out of bounds
    IndexOutOfBounds { index: usize, size: usize },
}

impl fmt::Display for AlgebraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInvertible { reason } => {
                write!(f, "element not invertible: {}", reason)
            }
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::NumericalError { operation, details } => {
                write!(f, "numerical error in {}: {}", operation, details)
            }
            Self::InvalidParameter {
                name,
                value,
                reason,
            } => {
                write!(f, "invalid parameter {}={}: {}", name, value, reason)
            }
            Self::UnsupportedOperation { algebra, operation } => {
                write!(f, "{} does not support operation: {}", algebra, operation)
            }
            Self::NormalizationFailed { norm } => {
                write!(f, "normalization failed: norm {} is too small", norm)
            }
            Self::IndexOutOfBounds { index, size } => {
                write!(f, "index {} out of bounds for size {}", index, size)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AlgebraError {}

/// Result type for algebra operations.
pub type AlgebraResult<T> = Result<T, AlgebraError>;

// ============================================================================
// Core Trait: BindingAlgebra
// ============================================================================

/// Core trait for algebras supporting Vector Symbolic Architecture operations.
///
/// This trait abstracts the fundamental operations needed for holographic
/// reduced representations:
///
/// - **Binding** (`bind`): Creates associations between representations.
///   The result should be dissimilar to both inputs.
///
/// - **Bundling** (`bundle`): Superposes multiple representations.
///   The result should be similar to all inputs.
///
/// - **Inverse** (`inverse`): Enables unbinding to retrieve associated values.
///   Must satisfy `x.bind(x.inverse()?) ≈ identity`.
///
/// # Algebraic Laws
///
/// Implementations should satisfy (approximately, due to noise):
///
/// 1. **Binding inverse**: `x.bind(&x.inverse()?) ≈ Self::identity()`
/// 2. **Identity**: `x.bind(&Self::identity()) = x`
/// 3. **Distributivity**: `a.bind(&b.bundle(&c, β)?) ≈ a.bind(&b).bundle(&a.bind(&c), β)?`
/// 4. **Dissimilarity**: `similarity(a.bind(&b), a) ≈ 0` and `similarity(a.bind(&b), b) ≈ 0`
/// 5. **Bundle similarity**: `similarity(a.bundle(&b, β)?, a) > 0`
///
/// # Type Parameters
///
/// The trait is implemented by algebra element types, not algebra configuration types.
/// Each element carries its own dimensionality (via const generics or runtime size).
pub trait BindingAlgebra: Sized + Clone + Send + Sync {
    /// The dimension of the algebra (number of basis elements or components).
    ///
    /// For Clifford algebras, this is typically 2^n where n is the vector space dimension.
    /// For FHRR and MAP, this equals the representation dimension directly.
    fn dimension(&self) -> usize;

    /// Create the binding identity element.
    ///
    /// This is the element `e` such that `x.bind(&e) = x` for all `x`.
    /// For Clifford algebras, this is the scalar 1.
    fn identity() -> Self;

    /// Create the additive zero element.
    ///
    /// This is the element `0` such that `x.superpose(&0)? = x`.
    fn zero() -> Self;

    /// Bind two elements (association/structure-creation).
    ///
    /// The binding operation creates a new representation that:
    /// - Is dissimilar to both inputs
    /// - Distributes over bundling: `a ⊛ (b ⊕ c) ≈ (a ⊛ b) ⊕ (a ⊛ c)`
    /// - Has an inverse: `a ⊛ a⁻¹ ≈ identity`
    ///
    /// # Mathematical Interpretation
    ///
    /// For Clifford algebras, this is the geometric product.
    /// For FHRR, this is element-wise complex multiplication.
    /// For MAP, this is element-wise product with permutation.
    fn bind(&self, other: &Self) -> Self;

    /// Compute the binding inverse.
    ///
    /// Returns `Some(inv)` where `self.bind(&inv) ≈ Self::identity()`,
    /// or `None` if the element is not invertible.
    ///
    /// # Error Conditions
    ///
    /// Returns `Err` when:
    /// - The element has zero or near-zero norm
    /// - The element is a null vector in an algebra with such elements
    fn inverse(&self) -> AlgebraResult<Self>;

    /// Unbind: retrieve the associated value.
    ///
    /// Given `bound = key.bind(&value)`, calling `key.unbind(&bound)`
    /// returns an approximation of `value`.
    ///
    /// Default implementation: `self.inverse()?.bind(other)`
    fn unbind(&self, other: &Self) -> AlgebraResult<Self> {
        let inv = self.inverse()?;
        Ok(inv.bind(other))
    }

    /// Bundle two elements for attention or cleanup.
    ///
    /// Bundling keeps magnitude bounded and may normalize or select between
    /// candidates. Use [`superpose`](Self::superpose) instead when building an
    /// additive holographic trace whose magnitude must grow with accumulated
    /// evidence.
    ///
    /// The `beta` parameter controls soft vs hard bundling:
    /// - `beta = 1.0`: Soft bundling (weighted average / logsumexp)
    /// - `beta = ∞`: Hard bundling (winner-take-all / max)
    ///
    /// The result should be similar to both inputs.
    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self>;

    /// Additively superpose two elements for holographic accumulation.
    ///
    /// This operation represents coefficient-wise addition and is intended for
    /// memory traces such as `T = Σ keyᵢ ⊛ valueᵢ`. Unlike
    /// [`bundle`](Self::bundle), it does not normalize or apply attention
    /// weights, so repeated superposition preserves earlier contributions and
    /// allows the trace magnitude to grow.
    ///
    /// The provided implementation reconstructs `Self` from the sum of its
    /// coefficient representations. Existing implementors therefore receive
    /// additive behavior without defining another required trait method.
    ///
    /// # Errors
    ///
    /// Returns [`AlgebraError::DimensionMismatch`] when the operands or their
    /// coefficient representations have different dimensions. Other errors are
    /// propagated from [`from_coefficients`](Self::from_coefficients).
    fn superpose(&self, other: &Self) -> AlgebraResult<Self> {
        if self.dimension() != other.dimension() {
            return Err(AlgebraError::DimensionMismatch {
                expected: self.dimension(),
                actual: other.dimension(),
            });
        }

        let left = self.to_coefficients();
        let right = other.to_coefficients();
        if left.len() != right.len() {
            return Err(AlgebraError::DimensionMismatch {
                expected: left.len(),
                actual: right.len(),
            });
        }
        let coefficients: Vec<f64> = left
            .into_iter()
            .zip(right)
            .map(|(left, right)| left + right)
            .collect();
        Self::from_coefficients(&coefficients)
    }

    /// Multiply every coefficient by `factor` for weighted superposition.
    ///
    /// Weighted accumulation composes this helper with
    /// [`superpose`](Self::superpose). Trait-qualified calls such as
    /// `<A as BindingAlgebra>::scale(binding, weight)` avoid shadowing by
    /// concrete types that already expose an infallible inherent `scale`.
    ///
    /// # Errors
    ///
    /// Propagates coefficient reconstruction errors from
    /// [`from_coefficients`](Self::from_coefficients).
    fn scale(&self, factor: f64) -> AlgebraResult<Self> {
        let coefficients: Vec<f64> = self
            .to_coefficients()
            .into_iter()
            .map(|coefficient| coefficient * factor)
            .collect();
        Self::from_coefficients(&coefficients)
    }

    /// Bundle multiple elements efficiently.
    ///
    /// Default implementation uses pairwise bundling.
    /// Algebras may override with more efficient parallel implementations.
    fn bundle_all(items: &[Self], beta: f64) -> AlgebraResult<Self> {
        if items.is_empty() {
            return Ok(Self::zero());
        }
        if items.len() == 1 {
            return Ok(items[0].clone());
        }

        let mut result = items[0].clone();
        for item in items.iter().skip(1) {
            result = result.bundle(item, beta)?;
        }
        Ok(result)
    }

    /// Compute similarity between two elements.
    ///
    /// Returns a value in `[-1, 1]` representing normalized inner product.
    /// - `1.0`: Identical (up to normalization)
    /// - `0.0`: Orthogonal / dissimilar
    /// - `-1.0`: Opposite
    fn similarity(&self, other: &Self) -> f64;

    /// Compute the norm (magnitude) of the element.
    fn norm(&self) -> f64;

    /// Normalize the element to unit norm.
    ///
    /// Returns `Err(NormalizationFailed)` if the norm is too small.
    fn normalize(&self) -> AlgebraResult<Self>;

    /// Apply a random permutation for sequence encoding.
    ///
    /// The `shift` parameter determines which permutation to apply.
    /// Calling `permute(-shift)` on the result should approximately
    /// recover the original (for commutative permutation groups).
    ///
    /// Default implementation: uses a simple cyclic shift.
    /// Algebras may override with more sophisticated permutations.
    fn permute(&self, shift: i32) -> Self;

    /// Get a coefficient/component by index.
    ///
    /// Returns `Err(IndexOutOfBounds)` if index >= dimension().
    fn get(&self, index: usize) -> AlgebraResult<f64>;

    /// Set a coefficient/component by index.
    ///
    /// Returns `Err(IndexOutOfBounds)` if index >= dimension().
    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()>;

    /// Create from a vector of coefficients.
    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self>;

    /// Extract coefficients as a vector.
    fn to_coefficients(&self) -> Vec<f64>;

    /// Get the name of this algebra (for diagnostics).
    fn algebra_name() -> &'static str;

    /// Theoretical capacity: approximate number of items before retrieval degrades.
    ///
    /// For most algebras, this is approximately `dimension() / ln(dimension())`.
    fn theoretical_capacity(&self) -> usize {
        let dim = self.dimension() as f64;
        if dim <= 1.0 {
            return 1;
        }
        (dim / dim.ln()).max(1.0) as usize
    }

    /// Estimate signal-to-noise ratio given number of stored items.
    ///
    /// Higher SNR means more reliable retrieval.
    /// SNR ≈ sqrt(dimension / item_count)
    fn estimate_snr(&self, item_count: usize) -> f64 {
        if item_count == 0 {
            return f64::INFINITY;
        }
        let dim = self.dimension() as f64;
        (dim / item_count as f64).sqrt()
    }
}

// ============================================================================
// Marker Trait: GeometricAlgebra
// ============================================================================

/// Marker trait for algebras that are true Clifford/geometric algebras.
///
/// These algebras support additional geometric operations:
/// - Grade projection
/// - Reversion
/// - Conjugation
/// - Meet and join operations
///
/// Not all binding algebras are geometric algebras. For example,
/// FHRR and MAP do not have the full geometric algebra structure.
pub trait GeometricAlgebra: BindingAlgebra {
    /// The maximum grade in this algebra.
    ///
    /// For Cl(n,0,0), this equals n.
    fn max_grade(&self) -> usize;

    /// Extract the grade-k component.
    fn grade_project(&self, grade: usize) -> Self;

    /// Compute the reverse (grade involution).
    ///
    /// For a k-blade B, reverse(B) = (-1)^(k(k-1)/2) * B
    fn reverse(&self) -> Self;

    /// Compute the grade spectrum (norm of each grade).
    fn grade_spectrum(&self) -> Vec<f64>;

    /// Get the scalar (grade-0) part.
    fn scalar_part(&self) -> f64 {
        self.grade_project(0).get(0).unwrap_or(0.0)
    }

    /// Get the vector (grade-1) part.
    fn vector_part(&self) -> Self {
        self.grade_project(1)
    }

    /// Get the bivector (grade-2) part.
    fn bivector_part(&self) -> Self {
        self.grade_project(2)
    }

    /// Compute the dual (Hodge star operator).
    fn dual(&self) -> Self;

    /// Compute the inner product (contraction).
    fn inner_product(&self, other: &Self) -> Self;

    /// Compute the outer product (wedge).
    fn outer_product(&self, other: &Self) -> Self;
}

// ============================================================================
// Algebra Configuration
// ============================================================================

/// Configuration parameters for binding algebra operations.
///
/// This struct holds hyperparameters that affect how binding and bundling
/// behave, but is separate from the algebra implementation itself.
#[derive(Clone, Debug)]
pub struct AlgebraConfig {
    /// Temperature for bundling: 1.0 = soft (logsumexp), f64::INFINITY = hard (max)
    pub bundle_beta: f64,

    /// Temperature for retrieval
    pub retrieval_beta: f64,

    /// Whether to normalize after binding operations
    pub normalize_bindings: bool,

    /// Similarity threshold for "match" in retrieval
    pub similarity_threshold: f64,

    /// Minimum norm for considering an element invertible
    pub invertibility_threshold: f64,
}

impl Default for AlgebraConfig {
    fn default() -> Self {
        Self {
            bundle_beta: 1.0,              // soft encoding by default
            retrieval_beta: f64::INFINITY, // hard retrieval by default
            normalize_bindings: true,
            similarity_threshold: 0.5,
            invertibility_threshold: 1e-10,
        }
    }
}

impl AlgebraConfig {
    /// Create config optimized for soft encoding and hard retrieval.
    pub fn soft_encode_hard_retrieve() -> Self {
        Self::default()
    }

    /// Create config with fully soft operations.
    pub fn all_soft() -> Self {
        Self {
            bundle_beta: 1.0,
            retrieval_beta: 1.0,
            normalize_bindings: true,
            similarity_threshold: 0.5,
            invertibility_threshold: 1e-10,
        }
    }

    /// Create config with fully hard (tropical) operations.
    pub fn all_hard() -> Self {
        Self {
            bundle_beta: f64::INFINITY,
            retrieval_beta: f64::INFINITY,
            normalize_bindings: false,
            similarity_threshold: 0.5,
            invertibility_threshold: 1e-10,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute log-sum-exp with temperature: `(1/β) * ln(exp(β*a) + exp(β*b))`
///
/// This smoothly interpolates between:
/// - β → ∞: max(a, b)
/// - β → 0: (a + b) / 2
#[inline]
pub fn logsumexp(a: f64, b: f64, beta: f64) -> f64 {
    if beta.is_infinite() {
        return a.max(b);
    }
    if beta <= 0.0 {
        return (a + b) / 2.0;
    }

    // Numerically stable logsumexp
    let max_val = a.max(b);
    let sum = (beta * (a - max_val)).exp() + (beta * (b - max_val)).exp();
    max_val + sum.ln() / beta
}

/// Compute softmax weights for two values.
#[inline]
pub fn softmax_pair(a: f64, b: f64, beta: f64) -> (f64, f64) {
    if beta.is_infinite() {
        return if a >= b { (1.0, 0.0) } else { (0.0, 1.0) };
    }
    if beta <= 0.0 {
        return (0.5, 0.5);
    }

    let max_val = a.max(b);
    let exp_a = (beta * (a - max_val)).exp();
    let exp_b = (beta * (b - max_val)).exp();
    let sum = exp_a + exp_b;
    (exp_a / sum, exp_b / sum)
}
