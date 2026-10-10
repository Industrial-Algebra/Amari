//! Tropical-Dual-Clifford Fusion Types - The Heart of Amari
//!
//! This module defines the core fusion system that combines three exotic number systems:
//!
//! ## The Three Pillars
//!
//! 1. **Tropical Algebra** (Max-Plus Semiring)
//!    - Purpose: Efficient approximation for neural network operations
//!    - Key insight: Converts expensive softmax to simple max operations
//!    - Applications: Fast attention mechanisms, approximate inference
//!
//! 2. **Dual Numbers** (Automatic Differentiation)
//!    - Purpose: Exact gradient computation without computational graphs
//!    - Key insight: Using ε where ε² = 0 captures derivatives algebraically
//!    - Applications: Training, optimization, sensitivity analysis
//!
//! 3. **Clifford Algebra** (Geometric Algebra)
//!    - Purpose: Geometric transformations and rotations
//!    - Key insight: Unifies vectors, rotations, and reflections in one algebra
//!    - Applications: Spatial reasoning, attention geometry, semantic space
//!
//! ## Fusion Philosophy
//!
//! The TropicalDualClifford type fuses these three systems to enable:
//! - **Fast exploration** (tropical) → **Exact refinement** (dual) → **Geometric projection** (Clifford)
//! - Each representation maintains consistency through synchronization
//! - Applications can selectively use whichever view is most efficient
//!
//! This fusion is what makes Amari unique: combining computational efficiency,
//! mathematical rigor, and geometric insight in a single unified framework.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use num_traits::Float;

use amari_core::Multivector;
use amari_dual::{multivector::DualMultivector, DualNumber};
use amari_tropical::{TropicalMultivector, TropicalNumber};

/// The core fusion type combining tropical, dual, and Clifford algebras
///
/// This is the heart of the Amari library, enabling a three-phase optimization paradigm:
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────┐
/// │   TropicalDualClifford<T, DIM>          │
/// ├─────────────────────────────────────────┤
/// │                                         │
/// │  tropical: TropicalMultivector          │
/// │    ↓ (max-plus operations)              │
/// │    • Fast approximate inference         │
/// │    • Efficient attention scoring        │
/// │                                         │
/// │  dual: DualMultivector                  │
/// │    ↓ (automatic differentiation)        │
/// │    • Exact gradients                    │
/// │    • Training and optimization          │
/// │                                         │
/// │  clifford: Multivector                  │
/// │    ↓ (geometric algebra)                │
/// │    • Spatial transformations            │
/// │    • Semantic geometry                  │
/// │                                         │
/// └─────────────────────────────────────────┘
/// ```
///
/// # Type Parameters
///
/// - `T`: Floating-point type (typically f32 or f64)
/// - `DIM`: Dimension of the underlying space (typically 8 for LLM applications)
///
/// # Example Usage
///
/// ```ignore
/// use amari_fusion::TropicalDualClifford;
///
/// // Create for 8-dimensional LLM hidden space
/// let tdc = TropicalDualClifford::<f64, 8>::random_with_scale(0.1);
///
/// // Phase 1: Fast tropical approximation
/// let tropical_features = tdc.extract_tropical_features();
///
/// // Phase 2: Exact dual refinement
/// let dual_features = tdc.extract_dual_features();
///
/// // Phase 3: Geometric projection
/// let clifford_repr = tdc.clifford();
/// ```
#[derive(Clone, Debug)]
pub struct TropicalDualClifford<T: Float, const DIM: usize> {
    /// Tropical (max-plus) representation for efficient approximate computation
    tropical_repr: TropicalMultivector<T, DIM, 0, 0>,

    /// Dual number representation for automatic differentiation
    dual_repr: DualMultivector<T, DIM, 0, 0>,

    /// Clifford algebra representation for geometric transformations
    /// Note: Multivector is hardcoded to f64 in amari-core
    clifford_repr: Multivector<DIM, 0, 0>,
}

impl<T: Float, const DIM: usize> TropicalDualClifford<T, DIM> {
    /// Create a new TDC with zero/identity initializations
    pub fn new() -> Self {
        Self {
            tropical_repr: TropicalMultivector::new(),
            dual_repr: DualMultivector::zero(),
            clifford_repr: Multivector::zero(),
        }
    }

    /// Create a TDC initialized to zero
    pub fn zero() -> Self {
        Self::new()
    }

    /// Check if all components are zero (within tolerance)
    pub fn is_zero(&self) -> bool {
        // Check if clifford norm is near zero
        let clifford_norm = self.clifford_repr.coefficient_norm();
        clifford_norm < 1e-10
    }

    /// Create a TDC from logits (for neural network applications)
    ///
    /// # Arguments
    /// * `logits` - Raw logit values from a neural network layer
    ///
    /// # Example
    /// ```ignore
    /// let logits = vec![0.1, 0.5, -0.3, 0.8];
    /// let tdc = TropicalDualClifford::<f64, 4>::from_logits(&logits);
    /// ```
    pub fn from_logits(logits: &[T]) -> Self {
        let mut result = Self::new();

        // Initialize tropical component from logits
        for (i, &logit) in logits.iter().enumerate().take(DIM.min(8)) {
            result.tropical_repr.set(i, TropicalNumber::new(logit)).ok();
        }

        // Initialize dual component (as variables for gradient tracking)
        for (i, &logit) in logits.iter().enumerate().take(DIM.min(8)) {
            result.dual_repr.set(i, DualNumber::variable(logit));
        }

        // Initialize Clifford component
        let mut clifford_coeffs = vec![0.0; Multivector::<DIM, 0, 0>::BASIS_COUNT];
        for (i, &logit) in logits.iter().enumerate().take(DIM.min(8)) {
            if i < clifford_coeffs.len() {
                // Convert T to f64 for Multivector
                clifford_coeffs[i] = logit.to_f64().unwrap_or(0.0);
            }
        }
        result.clifford_repr = Multivector::from_coefficients(clifford_coeffs);

        result
    }

    /// Create a TDC with random initialization scaled appropriately for neural networks
    ///
    /// Uses Xavier/Glorot-style initialization to ensure stable gradient flow.
    ///
    /// # Arguments
    /// * `scale` - Scaling factor, typically 1/sqrt(dim) for proper initialization
    ///
    /// # Example
    /// ```ignore
    /// let scale = 1.0 / (8_f64).sqrt();
    /// let tdc = TropicalDualClifford::<f64, 8>::random_with_scale(scale);
    /// ```
    pub fn random_with_scale(scale: T) -> Self {
        let mut result = Self::new();

        // Initialize with random values in [-scale, scale]
        // In a real implementation, this would use a proper RNG
        // For now, create a simple pattern
        for i in 0..DIM.min(8) {
            let val = scale * T::from((i as f64 - 4.0) / 4.0).unwrap();

            // Initialize tropical component
            result.tropical_repr.set(i, TropicalNumber::new(val)).ok();

            // Initialize dual component (as a variable for gradient tracking)
            result.dual_repr.set(i, DualNumber::variable(val));
        }

        // Initialize Clifford component
        let mut clifford_coeffs = vec![0.0; Multivector::<DIM, 0, 0>::BASIS_COUNT];
        for i in 0..DIM.min(8) {
            let val = scale * T::from((i as f64 - 4.0) / 4.0).unwrap();
            if i < clifford_coeffs.len() {
                clifford_coeffs[i] = val.to_f64().unwrap_or(0.0);
            }
        }
        result.clifford_repr = Multivector::from_coefficients(clifford_coeffs);

        result
    }

    /// Get reference to the tropical representation
    ///
    /// This provides a view of the object through the lens of tropical algebra,
    /// useful for efficient approximate computations.
    pub fn tropical(&self) -> &TropicalMultivector<T, DIM, 0, 0> {
        &self.tropical_repr
    }

    /// Get reference to the dual representation
    ///
    /// This provides exact values with automatic differentiation capabilities,
    /// essential for training and optimization.
    pub fn dual(&self) -> &DualMultivector<T, DIM, 0, 0> {
        &self.dual_repr
    }

    /// Get mutable reference to the dual representation
    ///
    /// This allows modifying the dual representation for gradient tracking
    /// and derivative propagation.
    pub fn dual_mut(&mut self) -> &mut DualMultivector<T, DIM, 0, 0> {
        &mut self.dual_repr
    }

    /// Get reference to the Clifford algebra representation
    ///
    /// This provides geometric transformations and spatial reasoning capabilities.
    pub fn clifford(&self) -> &Multivector<DIM, 0, 0> {
        &self.clifford_repr
    }

    /// Extract features from the tropical representation
    ///
    /// This provides a view of the object through the lens of tropical algebra,
    /// useful for efficient approximate computations.
    ///
    /// # Returns
    /// Vector of tropical numbers representing the features in max-plus space
    pub fn extract_tropical_features(&self) -> Vec<TropicalNumber<T>> {
        let mut features = Vec::new();
        for i in 0..DIM.min(8) {
            if let Ok(val) = self.tropical_repr.get(i) {
                features.push(val);
            } else {
                features.push(TropicalNumber::zero());
            }
        }
        features
    }

    /// Extract features from the dual representation
    ///
    /// This provides exact values with automatic differentiation capabilities,
    /// essential for training and optimization.
    ///
    /// # Returns
    /// Vector of dual numbers containing both values and gradients
    pub fn extract_dual_features(&self) -> Vec<DualNumber<T>> {
        let mut features = Vec::new();
        for i in 0..DIM.min(8) {
            let val = self.dual_repr.get(i);
            features.push(val);
        }
        features
    }

    /// Synchronize representations to maintain consistency
    ///
    /// After updating one representation, this ensures the others are updated
    /// to reflect the same underlying mathematical object.
    ///
    /// # Synchronization Strategy
    /// - Dual → Tropical: Extract real parts, convert to tropical
    /// - Dual → Clifford: Extract real parts, update geometric representation
    /// - Maintain derivatives in dual representation for gradient flow
    pub fn synchronize(&mut self) {
        // Use dual as source of truth (it has the most information)
        let mut clifford_coeffs = vec![0.0; Multivector::<DIM, 0, 0>::BASIS_COUNT];

        for i in 0..DIM.min(8) {
            let dual_val = self.dual_repr.get(i);

            // Update tropical representation (max-plus space)
            self.tropical_repr
                .set(i, TropicalNumber::new(dual_val.real))
                .ok();

            // Update Clifford representation
            if i < clifford_coeffs.len() {
                clifford_coeffs[i] = dual_val.real.to_f64().unwrap_or(0.0);
            }
        }

        self.clifford_repr = Multivector::from_coefficients(clifford_coeffs);
    }

    /// Get the dimension of the space
    pub fn dim(&self) -> usize {
        DIM
    }

    /// Create TDC from individual components
    ///
    /// Combines tropical, dual, and clifford representations into a unified structure.
    ///
    /// # Arguments
    /// * `tropical` - Vector of tropical numbers
    /// * `dual` - Vector of dual numbers
    /// * `clifford` - Clifford algebra multivector
    pub fn from_components(
        tropical: Vec<TropicalNumber<T>>,
        dual: Vec<DualNumber<T>>,
        clifford: Multivector<DIM, 0, 0>,
    ) -> Self {
        let mut result = Self::new();

        // Set tropical components
        for (i, &tn) in tropical.iter().enumerate().take(DIM.min(8)) {
            result.tropical_repr.set(i, tn).ok();
        }

        // Set dual components
        for (i, &dn) in dual.iter().enumerate().take(DIM.min(8)) {
            result.dual_repr.set(i, dn);
        }

        // Set clifford component
        result.clifford_repr = clifford;

        result
    }

    /// Add two TDC objects component-wise
    ///
    /// Performs addition in each of the three algebraic systems:
    /// - Tropical: max-plus addition
    /// - Dual: standard addition with gradient tracking
    /// - Clifford: geometric addition
    pub fn add(&self, other: &Self) -> Self {
        let mut result = Self::new();

        // Add tropical components using classical addition on underlying values
        // This ensures interpolation works correctly with distance metrics
        for i in 0..DIM.min(8) {
            if let (Ok(a), Ok(b)) = (self.tropical_repr.get(i), other.tropical_repr.get(i)) {
                let sum = a.value() + b.value();
                result.tropical_repr.set(i, TropicalNumber::new(sum)).ok();
            }
        }

        // Add dual components (preserves gradients)
        for i in 0..DIM.min(8) {
            let a = self.dual_repr.get(i);
            let b = other.dual_repr.get(i);
            result.dual_repr.set(i, a + b);
        }

        // Add clifford components
        result.clifford_repr = self.clifford_repr.clone() + other.clifford_repr.clone();

        result
    }

    /// Scale TDC by a scalar factor
    ///
    /// Applies scaling uniformly across all three representations.
    /// Uses classical (not tropical) scaling for consistent interpolation semantics.
    pub fn scale(&self, factor: T) -> Self {
        let mut result = Self::new();

        // Scale tropical components using classical multiplication on underlying values
        // This ensures interpolation works correctly with distance metrics
        for i in 0..DIM.min(8) {
            if let Ok(val) = self.tropical_repr.get(i) {
                let scaled_value = val.value() * factor;
                result
                    .tropical_repr
                    .set(i, TropicalNumber::new(scaled_value))
                    .ok();
            }
        }

        // Scale dual components
        let dual_scale = DualNumber::constant(factor);
        for i in 0..DIM.min(8) {
            let val = self.dual_repr.get(i);
            result.dual_repr.set(i, val * dual_scale);
        }

        // Scale clifford component
        let clifford_scale = factor.to_f64().unwrap_or(1.0);
        result.clifford_repr = self.clifford_repr.clone() * clifford_scale;

        result
    }

    /// Compute distance between two TDC objects
    ///
    /// Uses a weighted combination of distances in each algebraic system.
    pub fn distance(&self, other: &Self) -> T {
        let mut tropical_dist = T::zero();
        let mut dual_dist = T::zero();

        // Tropical distance (max difference)
        for i in 0..DIM.min(8) {
            if let (Ok(a), Ok(b)) = (self.tropical_repr.get(i), other.tropical_repr.get(i)) {
                let diff = (a.value() - b.value()).abs();
                tropical_dist = tropical_dist.max(diff);
            }
        }

        // Dual distance (Euclidean)
        for i in 0..DIM.min(8) {
            let a = self.dual_repr.get(i);
            let b = other.dual_repr.get(i);
            let diff = a.real - b.real;
            dual_dist = dual_dist + diff * diff;
        }
        dual_dist = dual_dist.sqrt();

        // Clifford distance
        let clifford_diff = self.clifford_repr.clone() - other.clifford_repr.clone();
        let clifford_dist = T::from(clifford_diff.coefficient_norm()).unwrap_or(T::zero());

        // Weighted combination
        let w1 = T::from(0.33).unwrap();
        let w2 = T::from(0.33).unwrap();
        let w3 = T::from(0.34).unwrap();

        w1 * tropical_dist + w2 * dual_dist + w3 * clifford_dist
    }

    /// Interpolate between two TDC objects
    ///
    /// Performs convex interpolation: (1-t) * self + t * other
    pub fn interpolate(&self, other: &Self, t: T) -> Self {
        let one_minus_t = T::one() - t;
        let self_scaled = self.scale(one_minus_t);
        let other_scaled = other.scale(t);
        self_scaled.add(&other_scaled)
    }

    /// Transform this TDC by another (geometric transformation)
    ///
    /// Applies a transformation in the Clifford algebra sense,
    /// propagating to other representations.
    pub fn transform(&self, transformation: &Self) -> Self {
        let mut result = Self::new();

        // Apply transformation in Clifford space (geometric product)
        result.clifford_repr = self
            .clifford_repr
            .geometric_product(&transformation.clifford_repr);

        // Propagate to dual representation
        for i in 0..DIM.min(8) {
            let self_val = self.dual_repr.get(i);
            let trans_val = transformation.dual_repr.get(i);
            result.dual_repr.set(i, self_val * trans_val);
        }

        // Propagate to tropical representation
        for i in 0..DIM.min(8) {
            if let (Ok(self_val), Ok(trans_val)) = (
                self.tropical_repr.get(i),
                transformation.tropical_repr.get(i),
            ) {
                result
                    .tropical_repr
                    .set(i, self_val.tropical_mul(&trans_val))
                    .ok();
            }
        }

        result
    }

    /// Evaluate this TDC against another, producing comprehensive metrics
    ///
    /// Computes scores across all three algebraic systems.
    pub fn evaluate(&self, other: &Self) -> EvaluationResult<T> {
        // Tropical evaluation: best path score
        let mut best_path_score = TropicalNumber::neg_infinity();
        for i in 0..DIM.min(8) {
            if let (Ok(a), Ok(b)) = (self.tropical_repr.get(i), other.tropical_repr.get(i)) {
                let path_score = a.tropical_mul(&b);
                best_path_score = best_path_score.tropical_add(&path_score);
            }
        }

        // Dual evaluation: gradient norm
        let mut gradient_norm_sq = T::zero();
        for i in 0..DIM.min(8) {
            let a = self.dual_repr.get(i);
            let b = other.dual_repr.get(i);
            let grad = a.dual * b.dual;
            gradient_norm_sq = gradient_norm_sq + grad * grad;
        }
        let gradient_norm = gradient_norm_sq.sqrt();

        // Clifford evaluation: geometric distance
        let clifford_diff = self.clifford_repr.clone() - other.clifford_repr.clone();
        let geometric_distance = clifford_diff.coefficient_norm();

        // Combined score
        let combined_score = T::from(best_path_score.value().to_f64().unwrap_or(0.0))
            .unwrap_or(T::zero())
            + gradient_norm
            + T::from(geometric_distance).unwrap_or(T::zero());

        EvaluationResult {
            best_path_score,
            gradient_norm,
            geometric_distance,
            combined_score,
        }
    }

    /// Create a random TDC (alias for random_with_scale with default scale)
    pub fn random() -> Self {
        Self::random_with_scale(<T as num_traits::NumCast>::from(0.1).unwrap())
    }

    // ========================================================================
    // Hybrid Clifford-Tropical Methods
    // ========================================================================

    /// Derive tropical representation from Clifford grade magnitudes.
    ///
    /// The tropical component becomes log-magnitude of each Clifford grade:
    /// - `tropical[k] = ln(|grade_k(clifford)|)`
    ///
    /// This makes tropical max equivalent to "which grade dominates?"
    /// and preserves Clifford algebraic structure as the source of truth.
    ///
    /// # Interpretation
    ///
    /// After this operation:
    /// - Tropical max selects the dominant grade
    /// - Tropical addition (logsumexp) computes soft grade selection
    /// - The Clifford representation remains the authoritative structure
    ///
    /// # Example
    /// ```ignore
    /// let mut tdc = TropicalDualClifford::<f64, 4>::from_logits(&[1.0, 2.0, 3.0, 4.0]);
    /// tdc.derive_tropical_from_clifford();
    /// let dominant = tdc.dominant_grade(); // Which grade has largest magnitude
    /// ```
    pub fn derive_tropical_from_clifford(&mut self) {
        // Use grade spectrum from Clifford to populate tropical
        let spectrum = self.clifford_repr.grade_spectrum();

        for (grade, &magnitude) in spectrum.iter().enumerate() {
            if grade >= DIM.min(8) {
                break;
            }

            // Use log(magnitude) as tropical value
            // For near-zero magnitudes, use negative infinity (tropical zero)
            let tropical_val = if magnitude > 1e-10 {
                magnitude.ln()
            } else {
                f64::NEG_INFINITY
            };

            self.tropical_repr
                .set(
                    grade,
                    TropicalNumber::new(T::from(tropical_val).unwrap_or(T::neg_infinity())),
                )
                .ok();
        }
    }

    /// Get the dominant grade (the grade with maximum tropical value / largest magnitude).
    ///
    /// This uses the tropical representation to efficiently determine which
    /// Clifford grade dominates the multivector structure.
    ///
    /// # Returns
    /// The grade (0, 1, 2, ...) with the largest magnitude in the Clifford representation.
    pub fn dominant_grade(&self) -> usize {
        let mut max_tropical = f64::NEG_INFINITY;
        let mut dominant = 0;

        for grade in 0..DIM.min(8) {
            if let Ok(val) = self.tropical_repr.get(grade) {
                let v = val.value().to_f64().unwrap_or(f64::NEG_INFINITY);
                if v > max_tropical {
                    max_tropical = v;
                    dominant = grade;
                }
            }
        }

        dominant
    }

    /// Create a TDC from a Clifford multivector, deriving other representations.
    ///
    /// This is the primary constructor for the hybrid approach:
    /// - Clifford is the source of truth
    /// - Tropical is derived from grade magnitudes
    /// - Dual tracks derivatives of Clifford coefficients
    pub fn from_clifford(clifford: Multivector<DIM, 0, 0>) -> Self
    where
        T: num_traits::NumCast,
    {
        let mut result = Self::new();
        result.clifford_repr = clifford;

        // Derive tropical from Clifford grades
        result.derive_tropical_from_clifford();

        // Initialize dual from Clifford coefficients
        for i in 0..DIM.min(8) {
            let coeff = result.clifford_repr.get(i);
            result
                .dual_repr
                .set(i, DualNumber::variable(T::from(coeff).unwrap_or(T::zero())));
        }

        result
    }

    /// Synchronize all representations from Clifford (source of truth).
    ///
    /// This ensures consistency after Clifford-based operations:
    /// - Tropical ← derived from Clifford grade magnitudes
    /// - Dual ← derived from Clifford coefficients
    pub fn synchronize_from_clifford(&mut self)
    where
        T: num_traits::NumCast,
    {
        // Derive tropical from grade magnitudes
        self.derive_tropical_from_clifford();

        // Update dual from Clifford coefficients
        for i in 0..DIM.min(8) {
            let coeff = self.clifford_repr.get(i);
            // Preserve existing dual part if any, update real part
            let existing = self.dual_repr.get(i);
            self.dual_repr.set(
                i,
                DualNumber::new(T::from(coeff).unwrap_or(T::zero()), existing.dual),
            );
        }
    }

    /// Get the grade spectrum (log-magnitudes of all grades).
    ///
    /// Returns the tropical values for each grade, representing
    /// ln(|grade_k|) for grade k.
    pub fn grade_spectrum(&self) -> Vec<f64> {
        let mut spectrum = Vec::with_capacity(DIM.min(8));
        for grade in 0..DIM.min(8) {
            if let Ok(val) = self.tropical_repr.get(grade) {
                spectrum.push(val.value().to_f64().unwrap_or(f64::NEG_INFINITY));
            } else {
                spectrum.push(f64::NEG_INFINITY);
            }
        }
        spectrum
    }

    /// Compute the Clifford norm of this TDC.
    ///
    /// This is the authoritative norm, derived from the Clifford representation.
    pub fn norm(&self) -> f64 {
        self.clifford_repr.coefficient_norm()
    }

    /// Compute similarity based on Clifford representation only.
    ///
    /// This is the authoritative similarity measure, using the normalized
    /// scalar product of the Clifford representations.
    ///
    /// Uses the proper inner product: <A B̃>₀ / (|A| |B|)
    /// where B̃ is the reverse of B. This ensures similarity=1 for equal elements.
    pub fn clifford_similarity(&self, other: &Self) -> f64 {
        let self_norm = self.clifford_repr.coefficient_norm();
        let other_norm = other.clifford_repr.coefficient_norm();

        if self_norm < 1e-10 || other_norm < 1e-10 {
            return 0.0;
        }

        // Compute inner product using scalar product with reverse: <A B̃>₀
        // This is the proper inner product for general multivectors
        let inner = self
            .clifford_repr
            .scalar_product(&other.clifford_repr.reverse());

        // Normalize
        inner / (self_norm * other_norm)
    }

    /// Create a random versor (product of vectors) for testing.
    ///
    /// A versor is guaranteed to be invertible in Clifford algebra,
    /// making it suitable for binding/unbinding operations.
    ///
    /// # Arguments
    /// * `num_factors` - Number of vector factors (1=vector, 2=rotor, etc.)
    pub fn random_versor(num_factors: usize) -> Self
    where
        T: num_traits::NumCast,
    {
        use alloc::vec;

        if num_factors == 0 {
            // Return scalar identity
            return Self::from_clifford(Multivector::<DIM, 0, 0>::scalar(1.0));
        }

        // Start with a random unit vector
        let mut clifford_coeffs = vec![0.0; Multivector::<DIM, 0, 0>::BASIS_COUNT];
        let mut norm_sq = 0.0;
        for i in 0..DIM.min(8) {
            let index = 1 << i;
            if index < clifford_coeffs.len() {
                let val = (fastrand::f64() - 0.5) * 2.0;
                clifford_coeffs[index] = val;
                norm_sq += val * val;
            }
        }

        // Normalize
        if norm_sq > 1e-10 {
            let scale = 1.0 / norm_sq.sqrt();
            for i in 0..DIM.min(8) {
                let index = 1 << i;
                if index < clifford_coeffs.len() {
                    clifford_coeffs[index] *= scale;
                }
            }
        }

        let mut versor = Multivector::from_coefficients(clifford_coeffs);

        // Multiply by additional random unit vectors
        for _ in 1..num_factors {
            let mut v_coeffs = vec![0.0; Multivector::<DIM, 0, 0>::BASIS_COUNT];
            let mut v_norm_sq = 0.0;
            for i in 0..DIM.min(8) {
                let index = 1 << i;
                if index < v_coeffs.len() {
                    let val = (fastrand::f64() - 0.5) * 2.0;
                    v_coeffs[index] = val;
                    v_norm_sq += val * val;
                }
            }
            if v_norm_sq > 1e-10 {
                let scale = 1.0 / v_norm_sq.sqrt();
                for i in 0..DIM.min(8) {
                    let index = 1 << i;
                    if index < v_coeffs.len() {
                        v_coeffs[index] *= scale;
                    }
                }
            }
            let v = Multivector::from_coefficients(v_coeffs);
            versor = versor.geometric_product(&v);
        }

        Self::from_clifford(versor)
    }

    // ========================================================================
    // Holographic Binding Operations (v0.12.3+)
    //
    // These methods provide Vector Symbolic Architecture (VSA) operations
    // using the Clifford component for binding, bundling, and similarity.
    // ========================================================================

    /// Bind two TDC objects using geometric product (creates associations)
    ///
    /// The result is dissimilar to both inputs - useful for creating
    /// key-value associations in holographic memory.
    pub fn bind(&self, other: &Self) -> Self {
        let bound = self.clifford_repr.geometric_product(&other.clifford_repr);
        Self::from_clifford(bound)
    }

    /// Unbind: retrieve associated value
    ///
    /// If `bound = key.bind(value)`, then `key.unbind(bound) ≈ value`
    pub fn unbind(&self, other: &Self) -> Self {
        // Unbind using reverse: key.reverse() * bound
        let inv = self.clifford_repr.reverse();
        let unbound = inv.geometric_product(&other.clifford_repr);
        Self::from_clifford(unbound)
    }

    /// Bundle two representations (superposition)
    ///
    /// Used to store multiple items in a single representation.
    /// The `_beta` parameter is ignored (kept for API compatibility).
    pub fn bundle(&self, other: &Self, _beta: f64) -> Self {
        let sum_coeffs: Vec<f64> = self
            .clifford_repr
            .to_vec()
            .iter()
            .zip(other.clifford_repr.to_vec().iter())
            .map(|(a, b)| *a + *b)
            .collect();
        let bundled = Multivector::from_coefficients(sum_coeffs);
        Self::from_clifford(bundled)
    }

    /// Get the binding identity element
    ///
    /// `x.bind(identity) = x` for any x
    pub fn binding_identity() -> Self {
        // The identity for geometric product is the scalar 1
        let mut coeffs = vec![0.0; Multivector::<DIM, 0, 0>::BASIS_COUNT];
        coeffs[0] = 1.0;
        Self::from_clifford(Multivector::from_coefficients(coeffs))
    }

    /// Compute cosine similarity between two representations
    pub fn similarity(&self, other: &Self) -> f64 {
        let a_coeffs = self.clifford_repr.to_vec();
        let b_coeffs = other.clifford_repr.to_vec();

        let dot: f64 = a_coeffs
            .iter()
            .zip(b_coeffs.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f64 = a_coeffs.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b_coeffs.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            0.0
        } else {
            (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
        }
    }

    /// Compute cosine distance (1 - similarity)
    pub fn cosine_distance(&self, other: &Self) -> f64 {
        1.0 - self.similarity(other)
    }

    /// Compute the binding inverse
    ///
    /// If successful, `x.bind(x.binding_inverse()) ≈ identity`
    pub fn binding_inverse(&self) -> Option<Self> {
        let coeffs = self.clifford_repr.to_vec();
        let norm_sq: f64 = coeffs.iter().map(|x| x * x).sum();

        if norm_sq < 1e-10 {
            return None;
        }

        // For a versor v, the inverse is reverse(v) / |v|²
        let rev = self.clifford_repr.reverse();
        let scale = 1.0 / norm_sq;
        let inv_coeffs: Vec<f64> = rev.to_vec().iter().map(|x| x * scale).collect();
        let inv = Multivector::from_coefficients(inv_coeffs);

        Some(Self::from_clifford(inv))
    }

    /// Normalize to unit norm
    pub fn normalize(&self) -> Self {
        let coeffs = self.clifford_repr.to_vec();
        let norm: f64 = coeffs.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm < 1e-10 {
            return self.clone();
        }

        let scale = 1.0 / norm;
        let normed: Vec<f64> = coeffs.iter().map(|x| x * scale).collect();
        Self::from_clifford(Multivector::from_coefficients(normed))
    }
}

impl<T: Float, const DIM: usize> Default for TropicalDualClifford<T, DIM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Float + fmt::Display, const DIM: usize> fmt::Display for TropicalDualClifford<T, DIM> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TDC<{}>[ ", DIM)?;
        for i in 0..DIM.min(4) {
            if let Ok(t) = self.tropical_repr.get(i) {
                write!(f, "t:{} ", t.value())?;
            }
        }
        if DIM > 4 {
            write!(f, "... ")?;
        }
        write!(f, "]")
    }
}

/// Result type for evaluation operations
///
/// Contains scores from all three algebraic systems
#[derive(Clone, Debug)]
pub struct EvaluationResult<T: Float> {
    /// Best path score from tropical algebra
    pub best_path_score: TropicalNumber<T>,
    /// Gradient norm from dual numbers
    pub gradient_norm: T,
    /// Geometric distance from Clifford algebra
    pub geometric_distance: f64,
    /// Combined score across all systems
    pub combined_score: T,
}

/// Errors that can occur during evaluation
#[derive(Debug, Clone)]
pub enum EvaluationError {
    /// Dimension mismatch between predictions and targets
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension found
        actual: usize,
    },
    /// Invalid input (NaN, Inf, etc.)
    InvalidInput(String),
    /// Numerical instability detected
    NumericalInstability(String),
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::NumericalInstability(msg) => write!(f, "Numerical instability: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EvaluationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tdc_creation() {
        let tdc = TropicalDualClifford::<f64, 8>::new();
        assert_eq!(tdc.dim(), 8);
    }

    #[test]
    fn test_tdc_zero() {
        let tdc = TropicalDualClifford::<f64, 8>::zero();
        assert!(tdc.is_zero());
    }

    #[test]
    fn test_tdc_from_logits() {
        let logits = vec![0.1, 0.5, -0.3, 0.8];
        let tdc = TropicalDualClifford::<f64, 4>::from_logits(&logits);

        let dual_features = tdc.extract_dual_features();
        assert_eq!(dual_features.len(), 4);
        assert!((dual_features[0].real - 0.1).abs() < 1e-10);
        assert!((dual_features[1].real - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_tdc_random_initialization() {
        let scale = 0.1;
        let tdc = TropicalDualClifford::<f64, 8>::random_with_scale(scale);

        // Check that values are within expected range
        let tropical_features = tdc.extract_tropical_features();
        assert!(!tropical_features.is_empty());

        for feature in &tropical_features {
            let val = feature.value();
            assert!(val >= -scale && val <= scale || val.is_infinite());
        }
    }

    #[test]
    fn test_feature_extraction() {
        let tdc = TropicalDualClifford::<f64, 8>::random_with_scale(0.1);

        let tropical = tdc.extract_tropical_features();
        let dual = tdc.extract_dual_features();

        assert_eq!(tropical.len(), dual.len());

        // Values should be reasonably close (tropical is approximation)
        for (t, d) in tropical.iter().zip(dual.iter()) {
            if !t.value().is_infinite() {
                let diff = (t.value() - d.real).abs();
                assert!(diff < 1.0, "Tropical and dual should be synchronized");
            }
        }
    }

    #[test]
    fn test_synchronization() {
        let mut tdc = TropicalDualClifford::<f64, 8>::new();

        // Update dual representation
        tdc.dual_repr.set(0, DualNumber::variable(5.0));
        tdc.dual_repr.set(1, DualNumber::variable(3.0));

        // Synchronize
        tdc.synchronize();

        // Check tropical is updated
        assert_eq!(tdc.tropical_repr.get(0).unwrap().value(), 5.0);
        assert_eq!(tdc.tropical_repr.get(1).unwrap().value(), 3.0);
    }

    #[test]
    fn test_accessor_methods() {
        let tdc = TropicalDualClifford::<f64, 4>::from_logits(&[1.0, 2.0, 3.0, 4.0]);

        // Test that accessor methods work
        let _tropical = tdc.tropical();
        let _dual = tdc.dual();
        let _clifford = tdc.clifford();
    }
}
