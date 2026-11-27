//! MAP (Multiply-Add-Permute) bipolar algebra.
//!
//! MAP is a simplified Vector Symbolic Architecture using bipolar vectors
//! (elements in {-1, +1}^D) with element-wise XOR (multiplication) for binding.
//!
//! # Representation
//!
//! MAP elements are bipolar vectors where each component is +1 or -1.
//! For computation, we use floating-point (-1.0, +1.0) but binding
//! operations preserve bipolarity for properly initialized elements.
//!
//! # Operations
//!
//! - **Binding**: Element-wise multiplication (XOR in {-1,+1} is multiplication)
//! - **Inverse**: Self (since x * x = 1 for all bipolar vectors)
//! - **Bundling**: Component-wise sum followed by sign() or threshold
//! - **Similarity**: Normalized dot product (cosine similarity)
//! - **Permute**: Cyclic shift of components
//!
//! # Advantages
//!
//! - Extremely simple: O(D) binding with just multiplication
//! - Self-inverse: Every element is its own inverse
//! - Hardware-friendly: Can be implemented with XOR on binary vectors
//! - Memory-efficient: Can be stored as bits
//!
//! # Disadvantages
//!
//! - Lower capacity than FHRR or Clifford for same dimension
//! - Hard bundling (sign function) is non-differentiable
//!
//! # References
//!
//! - Gayler, R. (1998). Multiplicative binding, representation operators,
//!   and analogy. Advances in analogy research, 1-27.

use alloc::vec::Vec;

use super::{AlgebraError, AlgebraResult, BindingAlgebra};

/// MAP (Multiply-Add-Permute) bipolar algebra element.
///
/// Components are ideally +1 or -1, though bundling produces
/// intermediate values that may need cleanup.
///
/// # Type Parameter
///
/// - `D`: Dimension of the bipolar vector
///
/// # Example
///
/// ```ignore
/// use amari_fusion::algebra::MAPAlgebra;
///
/// // Create a 1024-dimensional MAP element
/// let a = MAPAlgebra::<1024>::random_bipolar();
/// let b = MAPAlgebra::<1024>::random_bipolar();
///
/// // Binding via element-wise multiplication
/// let bound = a.bind(&b);
///
/// // MAP elements are self-inverse: a * a = identity
/// let identity = a.bind(&a);
/// ```
#[derive(Clone, Debug)]
pub struct MAPAlgebra<const D: usize> {
    /// Components (ideally ±1, but may be real during bundling)
    components: [f64; D],
}

impl<const D: usize> MAPAlgebra<D> {
    /// Create a new MAP element from components.
    pub fn new(components: [f64; D]) -> Self {
        Self { components }
    }

    /// Create from a Vec.
    pub fn from_vec(components: Vec<f64>) -> AlgebraResult<Self> {
        if components.len() != D {
            return Err(AlgebraError::DimensionMismatch {
                expected: D,
                actual: components.len(),
            });
        }

        let mut arr = [0.0; D];
        arr.copy_from_slice(&components);
        Ok(Self::new(arr))
    }

    /// Create the identity element (all +1).
    pub fn map_identity() -> Self {
        Self::new([1.0; D])
    }

    /// Create the zero element (for bundling).
    pub fn map_zero() -> Self {
        Self::new([0.0; D])
    }

    /// Create a random bipolar element (each component ±1).
    pub fn random_bipolar() -> Self {
        let mut components = [0.0; D];
        for c in &mut components {
            *c = if fastrand::bool() { 1.0 } else { -1.0 };
        }
        Self::new(components)
    }

    /// Create from a seed (deterministic random).
    ///
    /// Useful for creating consistent keys from identifiers.
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let mut components = [0.0; D];
        for c in &mut components {
            *c = if rng.bool() { 1.0 } else { -1.0 };
        }
        Self::new(components)
    }

    /// Get component at index i.
    pub fn get_component(&self, i: usize) -> Option<f64> {
        if i < D {
            Some(self.components[i])
        } else {
            None
        }
    }

    /// Set component at index i.
    pub fn set_component(&mut self, i: usize, value: f64) -> bool {
        if i < D {
            self.components[i] = value;
            true
        } else {
            false
        }
    }

    /// Get all components.
    pub fn components(&self) -> &[f64; D] {
        &self.components
    }

    /// Real pointwise correlation (the adjoint), not general inverse recovery.
    pub fn correlation(&self, bound: &Self) -> Self {
        self.elementwise_multiply(bound)
    }

    /// Element-wise multiplication (binding).
    ///
    /// For bipolar elements, this implements XOR in {-1, +1}.
    pub fn elementwise_multiply(&self, other: &Self) -> Self {
        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = self.components[i] * other.components[i];
        }
        Self::new(result)
    }

    /// Element-wise addition.
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = self.components[i] + other.components[i];
        }
        Self::new(result)
    }

    /// Scale by a scalar.
    pub fn scale(&self, s: f64) -> Self {
        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = self.components[i] * s;
        }
        Self::new(result)
    }

    /// Apply the sign function to all components.
    ///
    /// This "cleans up" a bundled vector back to bipolar.
    pub fn sign(&self) -> Self {
        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = if self.components[i] >= 0.0 { 1.0 } else { -1.0 };
        }
        Self::new(result)
    }

    /// Apply soft sign (tanh) for differentiable cleanup.
    ///
    /// The `beta` parameter controls sharpness:
    /// - beta → ∞: approaches hard sign
    /// - beta = 1: standard tanh
    /// - beta → 0: linear (no cleanup)
    pub fn soft_sign(&self, beta: f64) -> Self {
        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = (self.components[i] * beta).tanh();
        }
        Self::new(result)
    }

    /// Compute the squared L2 norm.
    pub fn norm_squared(&self) -> f64 {
        self.components.iter().map(|x| x * x).sum()
    }

    /// Compute the L2 norm.
    pub fn compute_norm(&self) -> AlgebraResult<f64> {
        self.norm()
    }

    /// Dot product.
    pub fn dot(&self, other: &Self) -> f64 {
        self.components
            .iter()
            .zip(other.components.iter())
            .map(|(a, b)| a * b)
            .sum()
    }

    /// Hamming distance (number of differing bits).
    ///
    /// For bipolar elements, this counts positions where the signs differ.
    pub fn hamming_distance(&self, other: &Self) -> usize {
        self.components
            .iter()
            .zip(other.components.iter())
            .filter(|(a, b)| (**a > 0.0) != (**b > 0.0))
            .count()
    }

    /// Cyclic shift (permutation).
    pub fn cyclic_shift(&self, shift: i32) -> Self {
        let n = D as i32;
        let shift = ((shift % n) + n) % n;

        let mut result = [0.0; D];
        for i in 0..D {
            let j = (i as i32 + shift) as usize % D;
            result[j] = self.components[i];
        }

        Self::new(result)
    }

    /// Check if element is truly bipolar (all ±1).
    pub fn is_bipolar(&self) -> bool {
        self.components
            .iter()
            .all(|&x| (x.abs() - 1.0).abs() < 1e-10)
    }

    /// Count positive components.
    pub fn count_positive(&self) -> usize {
        self.components.iter().filter(|&&x| x > 0.0).count()
    }

    /// Count negative components.
    pub fn count_negative(&self) -> usize {
        self.components.iter().filter(|&&x| x < 0.0).count()
    }

    /// Majority element from a bundle (returns ±1 for each component).
    ///
    /// This implements hard bundling: for each position, take the
    /// sign of the sum. Ties go to +1.
    pub fn majority(elements: &[Self]) -> Self {
        if elements.is_empty() {
            return Self::map_identity();
        }

        let mut sum = [0.0; D];
        for elem in elements {
            for i in 0..D {
                sum[i] += elem.components[i];
            }
        }

        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = if sum[i] >= 0.0 { 1.0 } else { -1.0 };
        }

        Self::new(result)
    }

    /// Soft majority with temperature parameter.
    ///
    /// Uses tanh instead of sign for differentiable bundling.
    pub fn soft_majority(elements: &[Self], beta: f64) -> Self {
        if elements.is_empty() {
            return Self::map_identity();
        }

        let mut sum = [0.0; D];
        for elem in elements {
            for i in 0..D {
                sum[i] += elem.components[i];
            }
        }

        // Normalize by count before applying tanh
        let n = elements.len() as f64;
        let mut result = [0.0; D];
        for i in 0..D {
            result[i] = (sum[i] / n * beta).tanh();
        }

        Self::new(result)
    }
}

// ============================================================================
// BindingAlgebra Implementation
// ============================================================================

impl<const D: usize> BindingAlgebra for MAPAlgebra<D> {
    fn dimension(&self) -> usize {
        D
    }

    fn identity() -> Self {
        Self::map_identity()
    }

    fn zero() -> Self {
        Self::map_zero()
    }

    fn bind(&self, other: &Self) -> Self {
        self.elementwise_multiply(other)
    }

    fn inverse(&self) -> AlgebraResult<Self> {
        let mut inv = [0.; D];
        for (out, &x) in inv.iter_mut().zip(&self.components) {
            if !x.is_finite() {
                return Err(AlgebraError::NonFinite);
            }
            if x == 0. {
                return Err(AlgebraError::KeyDomain);
            }
            *out = 1. / x;
            if !out.is_finite() {
                return Err(AlgebraError::Unrepresentable);
            }
        }
        super::keys::certify(self, Self::new(inv))
    }

    fn unbind(&self, other: &Self) -> AlgebraResult<Self> {
        super::unbind_general(self, other)
    }

    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self> {
        if beta.is_infinite() {
            // Hard bundling: add and take sign
            let sum = self.add(other);
            Ok(sum.sign())
        } else {
            // Soft bundling: weighted average with soft sign
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

            let weighted = self.scale(w1).add(&other.scale(w2));
            // Apply soft sign cleanup
            Ok(weighted.soft_sign(beta))
        }
    }

    fn bundle_all(items: &[Self], beta: f64) -> AlgebraResult<Self> {
        if items.is_empty() {
            return Ok(Self::zero());
        }
        if items.len() == 1 {
            return Ok(items[0].clone());
        }

        if beta.is_infinite() {
            // Use efficient majority function
            Ok(Self::majority(items))
        } else {
            // Use soft majority
            Ok(Self::soft_majority(items, beta))
        }
    }

    fn permute(&self, shift: i32) -> Self {
        self.cyclic_shift(shift)
    }

    fn get(&self, index: usize) -> AlgebraResult<f64> {
        if index >= D {
            return Err(AlgebraError::IndexOutOfBounds { index, size: D });
        }
        Ok(self.components[index])
    }

    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()> {
        if index >= D {
            return Err(AlgebraError::IndexOutOfBounds { index, size: D });
        }
        self.components[index] = value;
        Ok(())
    }

    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self> {
        if coeffs.len() != D {
            return Err(AlgebraError::DimensionMismatch {
                expected: D,
                actual: coeffs.len(),
            });
        }

        let mut arr = [0.0; D];
        arr.copy_from_slice(coeffs);
        Ok(Self::new(arr))
    }

    fn to_coefficients(&self) -> Vec<f64> {
        self.components.to_vec()
    }

    fn algebra_name() -> &'static str {
        "MAP"
    }
}

// ============================================================================
// Type Aliases for Common Dimensions
// ============================================================================

/// 64-dimensional MAP
pub type MAP64 = MAPAlgebra<64>;

/// 128-dimensional MAP
pub type MAP128 = MAPAlgebra<128>;

/// 256-dimensional MAP
pub type MAP256 = MAPAlgebra<256>;

/// 512-dimensional MAP
pub type MAP512 = MAPAlgebra<512>;

/// 1024-dimensional MAP
pub type MAP1024 = MAPAlgebra<1024>;

/// 2048-dimensional MAP
pub type MAP2048 = MAPAlgebra<2048>;

/// 4096-dimensional MAP
pub type MAP4096 = MAPAlgebra<4096>;

/// 10000-dimensional MAP (common in literature)
pub type MAP10000 = MAPAlgebra<10000>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_identity() {
        let identity = MAP64::map_identity();
        for i in 0..64 {
            assert!((identity.get_component(i).unwrap() - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_map_self_inverse() {
        let a = MAP64::random_bipolar();
        let a_sq = a.bind(&a);

        // a * a should equal identity (all +1)
        for i in 0..64 {
            assert!(
                (a_sq.get_component(i).unwrap() - 1.0).abs() < 1e-10,
                "component {} is not 1",
                i
            );
        }
    }

    #[test]
    fn test_map_binding_inverse() {
        let a = MAP256::random_bipolar();
        let identity = MAP256::identity();
        let bound = a.bind(&identity);

        // a * identity = a
        for i in 0..256 {
            assert!((a.get_component(i).unwrap() - bound.get_component(i).unwrap()).abs() < 1e-10);
        }
    }

    #[test]
    fn test_map_unbind_recover() {
        let key = MAP256::random_bipolar();
        let value = MAP256::random_bipolar();

        let bound = key.bind(&value);
        let recovered = key.unbind(&bound).expect("unbind should succeed");

        // recovered should equal value exactly (for bipolar)
        for i in 0..256 {
            assert!(
                (recovered.get_component(i).unwrap() - value.get_component(i).unwrap()).abs()
                    < 1e-10
            );
        }
    }

    #[test]
    fn test_map_dissimilarity() {
        let a = MAP256::random_bipolar();
        let b = MAP256::random_bipolar();
        let bound = a.bind(&b);

        // bound should be dissimilar to both a and b
        let sim_a = bound.similarity(&a).unwrap().abs();
        let sim_b = bound.similarity(&b).unwrap().abs();

        // With 256 dimensions, random bipolar vectors should be nearly orthogonal
        assert!(sim_a < 0.3, "similarity with a: {}", sim_a);
        assert!(sim_b < 0.3, "similarity with b: {}", sim_b);
    }

    #[test]
    fn test_map_majority() {
        // Create three vectors with known values
        let a = MAP64::from_coefficients(&[1.0; 64]).unwrap();
        let b = MAP64::from_coefficients(&[1.0; 64]).unwrap();
        let mut c_arr = [-1.0; 64];
        c_arr[0] = 1.0; // One component different
        let c = MAP64::from_coefficients(&c_arr).unwrap();

        let majority = MAP64::majority(&[a, b, c]);

        // Majority should be all +1 (two +1s beat one -1)
        for i in 0..64 {
            assert!((majority.get_component(i).unwrap() - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_map_cyclic_shift() {
        let a = MAP64::random_bipolar();
        let shifted = a.cyclic_shift(10);
        let back = shifted.cyclic_shift(-10);

        // Should recover original
        for i in 0..64 {
            assert!((a.get_component(i).unwrap() - back.get_component(i).unwrap()).abs() < 1e-10);
        }
    }

    #[test]
    fn test_map_hamming() {
        let a = MAP64::from_coefficients(&[1.0; 64]).unwrap();
        let mut b_arr = [1.0; 64];
        b_arr[0] = -1.0;
        b_arr[1] = -1.0;
        b_arr[2] = -1.0;
        let b = MAP64::from_coefficients(&b_arr).unwrap();

        let dist = a.hamming_distance(&b);
        assert_eq!(dist, 3);
    }

    #[test]
    fn test_map_is_bipolar() {
        let bipolar = MAP64::random_bipolar();
        assert!(bipolar.is_bipolar());

        let not_bipolar = MAP64::from_coefficients(&[0.5; 64]).unwrap();
        assert!(!not_bipolar.is_bipolar());
    }

    #[test]
    fn test_map_capacity() {
        let a = MAP256::random_bipolar();
        let capacity = a.theoretical_capacity();

        // 256 dimensions, capacity ≈ 256/ln(256) ≈ 46
        assert!(capacity > 40 && capacity < 55, "capacity: {}", capacity);
    }
}
