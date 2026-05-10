//! WASM bindings for the Amari mathematical computing library
//!
//! This module provides WebAssembly bindings for:
//! - **Geometric algebra** (amari-core) - Multivectors, rotors, projections
//! - **Tropical algebra** (amari-tropical) - Optimization via max-plus operations
//! - **Automatic differentiation** (amari-dual) - Forward-mode AD for ML
//! - **Differential calculus** (amari-calculus) - Manifolds, Riemannian geometry
//! - **Measure theory** (amari-measure) - Lebesgue integration, Monte Carlo
//! - **Fusion systems** (amari-fusion) - TropicalDualClifford for LLM evaluation
//! - **Information geometry** (amari-info-geom) - Fisher metrics, statistical manifolds
//! - **Holographic memory** (amari-holographic) - Vector Symbolic Architectures (v0.12.3+)
//! - **Optical fields** (amari-holographic) - GA-native Lee hologram encoding (v0.15.1+)
//! - **Probabilistic** (amari-probabilistic) - Probability distributions on GA spaces (v0.13.0+)
//! - **Functional analysis** (amari-functional) - Hilbert spaces, operators, spectral theory (v0.15.0+)
//! - **Probabilistic Contracts** (amari-flynn) - SMT-LIB2 proof obligations, Monte Carlo verification (v0.19.0+)
//! - **Computational Topology** (amari-topology) - Simplicial complexes, homology, persistent homology (v0.16.0+)
//!
//! # Holographic Memory (Vector Symbolic Architectures)
//!
//! The fusion module exposes holographic memory operations for storing and
//! retrieving associations in high-dimensional distributed representations:
//!
//! ```javascript
//! import { WasmHolographicMemory, WasmResonator, initHolographic } from 'amari-wasm';
//!
//! initHolographic();
//!
//! // Create holographic memory (256-dimensional ProductClifford algebra)
//! const memory = new WasmHolographicMemory();
//!
//! // Generate random keys and values
//! const key = WasmHolographicMemory.randomVersor(2);  // Product of 2 vectors
//! const value = WasmHolographicMemory.randomVersor(2);
//!
//! // Store and retrieve
//! memory.store(key, value);
//! const retrieved = memory.retrieve(key);
//!
//! // Check capacity
//! console.log(`Items: ${memory.itemCount()} / ${memory.theoreticalCapacity()}`);
//! console.log(`Near capacity: ${memory.isNearCapacity()}`);
//!
//! // Create resonator for cleanup
//! const codebook = [key1, key2, key3].flat();  // Flattened codebook
//! const resonator = new WasmResonator(codebook);
//! const cleanedUp = resonator.cleanupWithInfo(noisyInput);
//! ```
//!
//! For TropicalDualClifford binding operations:
//!
//! ```javascript
//! import { WasmTropicalDualClifford } from 'amari-wasm';
//!
//! const key = WasmTropicalDualClifford.randomVector();
//! const value = WasmTropicalDualClifford.randomVector();
//!
//! // Binding operations
//! const bound = key.bind(value);      // Create association
//! const retrieved = key.unbind(bound); // Retrieve value
//! const bundled = key.bundle(value, 1.0); // Superposition
//!
//! // Similarity
//! const sim = key.similarity(value);
//! const cliffordSim = key.cliffordSimilarity(value);
//! ```

use amari_core::{rotor::Rotor, Bivector, Multivector};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// Optional modules - some enabled for expanded WASM functionality
pub mod automata; // Enabled for v0.9.4 - Cellular automata, inverse design, self-assembly for web
pub mod calculus; // Enabled for v0.11.0 - Differential calculus, manifolds, and Riemannian geometry for web
pub mod cgt; // Enabled for v0.22.0 - Short combinatorial games, nimbers, and inspection helpers
pub mod dual; // Enabled for v0.9.3 - automatic differentiation for machine learning in web
pub mod enumerative; // Enabled for v0.9.4 - Enumerative geometry and intersection theory for web
pub mod flynn; // Enabled for v0.19.0 - Probabilistic contracts, SMT-LIB2 proof obligations, Monte Carlo verification
pub mod functional; // Enabled for v0.15.0 - Functional analysis on multivector spaces
pub mod fusion; // Enabled for v0.9.4 - TropicalDualClifford system for LLM evaluation in web
pub mod gf2; // Enabled for v0.19.0 - GF(2) algebra, binary codes, and finite field combinatorics
pub mod info_geom; // Enabled for v0.9.4 - Information geometry and statistical manifolds for web
pub mod measure; // Enabled for v0.10.0 - Measure theory and Lebesgue integration for web
pub mod network; // Enabled for v0.9.4 - Geometric network analysis for web
pub mod optical; // Enabled for v0.15.1 - GA-native optical field operations and Lee hologram encoding
pub mod optimization; // Enabled for v0.9.7 - Advanced optimization algorithms for web
pub mod probabilistic; // Enabled for v0.13.0 - Probability distributions on multivector spaces
pub mod relativistic;
pub mod surcomplex; // Enabled for v0.23.0 - Exact rational surcomplex arithmetic
pub mod surreal; // Enabled for v0.22.0 - Exact short surreal dyadics and game conversion helpers
pub mod topology; // Enabled for v0.16.0 - Simplicial complexes, homology, persistent homology
pub mod tropical; // Enabled for v0.9.3 - critical for optimization algorithms in web

/// Number of coefficients in a 3D Clifford algebra multivector (2^3 = 8)
/// Basis elements: 1, e1, e2, e3, e12, e13, e23, e123
const MULTIVECTOR_COEFFICIENTS: usize = 8;

/// Console logging utility
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

/// WASM wrapper for Multivector with TypedArray support
#[wasm_bindgen]
pub struct WasmMultivector {
    inner: Multivector<3, 0, 0>, // Default to 3D Euclidean for now
}

impl Default for WasmMultivector {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmMultivector {
    /// Create a new zero multivector
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Multivector::zero(),
        }
    }

    /// Create from a Float64Array of coefficients
    #[wasm_bindgen(js_name = fromCoefficients)]
    pub fn from_coefficients(coefficients: &[f64]) -> Result<WasmMultivector, JsValue> {
        if coefficients.len() != MULTIVECTOR_COEFFICIENTS {
            return Err(JsValue::from_str(
                "3D Clifford algebra requires exactly 8 coefficients",
            ));
        }

        Ok(Self {
            inner: Multivector::from_coefficients(coefficients.to_vec()),
        })
    }

    /// Create a scalar multivector
    #[wasm_bindgen(js_name = scalar)]
    pub fn scalar(value: f64) -> Self {
        Self {
            inner: Multivector::scalar(value),
        }
    }

    /// Create a basis vector (0-indexed)
    #[wasm_bindgen(js_name = basisVector)]
    pub fn basis_vector(index: usize) -> Result<WasmMultivector, JsValue> {
        if index >= 3 {
            return Err(JsValue::from_str(
                "Basis vector index must be 0, 1, or 2 for 3D",
            ));
        }

        Ok(Self {
            inner: Multivector::basis_vector(index),
        })
    }

    /// Get coefficients as a Float64Array
    #[wasm_bindgen(js_name = getCoefficients)]
    pub fn get_coefficients(&self) -> Vec<f64> {
        let mut coeffs = vec![0.0; MULTIVECTOR_COEFFICIENTS];
        #[allow(clippy::needless_range_loop)]
        for i in 0..MULTIVECTOR_COEFFICIENTS {
            coeffs[i] = self.inner.get(i);
        }
        coeffs
    }

    /// Get a specific coefficient
    #[wasm_bindgen(js_name = getCoefficient)]
    pub fn get_coefficient(&self, index: usize) -> f64 {
        self.inner.get(index)
    }

    /// Set a specific coefficient
    #[wasm_bindgen(js_name = setCoefficient)]
    pub fn set_coefficient(&mut self, index: usize, value: f64) {
        self.inner.set(index, value);
    }

    /// Geometric product
    #[wasm_bindgen(js_name = geometricProduct)]
    pub fn geometric_product(&self, other: &WasmMultivector) -> WasmMultivector {
        Self {
            inner: self.inner.geometric_product(&other.inner),
        }
    }

    /// Inner product (dot product for vectors)
    #[wasm_bindgen(js_name = innerProduct)]
    pub fn inner_product(&self, other: &WasmMultivector) -> WasmMultivector {
        Self {
            inner: self.inner.inner_product(&other.inner),
        }
    }

    /// Outer product (wedge product)
    #[wasm_bindgen(js_name = outerProduct)]
    pub fn outer_product(&self, other: &WasmMultivector) -> WasmMultivector {
        Self {
            inner: self.inner.outer_product(&other.inner),
        }
    }

    /// Scalar product
    #[wasm_bindgen(js_name = scalarProduct)]
    pub fn scalar_product(&self, other: &WasmMultivector) -> f64 {
        self.inner.scalar_product(&other.inner)
    }

    /// Reverse
    pub fn reverse(&self) -> WasmMultivector {
        Self {
            inner: self.inner.reverse(),
        }
    }

    /// Grade projection
    #[wasm_bindgen(js_name = gradeProjection)]
    pub fn grade_projection(&self, grade: usize) -> WasmMultivector {
        Self {
            inner: self.inner.grade_projection(grade),
        }
    }

    /// Exponential (for bivectors to create rotors)
    pub fn exp(&self) -> WasmMultivector {
        Self {
            inner: self.inner.exp(),
        }
    }

    /// Compute magnitude
    pub fn magnitude(&self) -> f64 {
        self.inner.magnitude()
    }

    /// Compute norm (alias for magnitude, maintained for compatibility)
    pub fn norm(&self) -> f64 {
        self.magnitude()
    }

    /// Normalize
    pub fn normalize(&self) -> Result<WasmMultivector, JsValue> {
        self.inner
            .normalize()
            .map(|mv| Self { inner: mv })
            .ok_or_else(|| JsValue::from_str("Cannot normalize zero multivector"))
    }

    /// Compute inverse
    pub fn inverse(&self) -> Result<WasmMultivector, JsValue> {
        self.inner
            .inverse()
            .map(|mv| Self { inner: mv })
            .ok_or_else(|| JsValue::from_str("Multivector is not invertible"))
    }

    /// Add two multivectors
    pub fn add(&self, other: &WasmMultivector) -> WasmMultivector {
        Self {
            inner: &self.inner + &other.inner,
        }
    }

    /// Subtract two multivectors
    pub fn sub(&self, other: &WasmMultivector) -> WasmMultivector {
        Self {
            inner: &self.inner - &other.inner,
        }
    }

    /// Scale by a scalar
    pub fn scale(&self, scalar: f64) -> WasmMultivector {
        Self {
            inner: &self.inner * scalar,
        }
    }
}

/// Batch operations for performance
#[wasm_bindgen]
pub struct BatchOperations;

#[wasm_bindgen]
impl BatchOperations {
    /// Batch geometric product: compute a[i] * b[i] for all i
    /// Optimized for WebAssembly performance with reduced allocations
    #[wasm_bindgen(js_name = batchGeometricProduct)]
    pub fn batch_geometric_product(a_batch: &[f64], b_batch: &[f64]) -> Result<Vec<f64>, JsValue> {
        let batch_size = a_batch.len() / MULTIVECTOR_COEFFICIENTS;

        if !a_batch.len().is_multiple_of(MULTIVECTOR_COEFFICIENTS)
            || !b_batch.len().is_multiple_of(MULTIVECTOR_COEFFICIENTS)
        {
            return Err(JsValue::from_str(
                "Batch arrays must have length divisible by multivector coefficients",
            ));
        }

        if a_batch.len() != b_batch.len() {
            return Err(JsValue::from_str("Batch arrays must have the same length"));
        }

        // Pre-allocate result vector to avoid repeated allocations
        let mut result = vec![0.0; a_batch.len()];

        // Use optimized batch processing with minimal allocations
        for i in 0..batch_size {
            let start = i * MULTIVECTOR_COEFFICIENTS;

            // Direct coefficient access without intermediate vector allocation
            let a_coeffs = &a_batch[start..start + MULTIVECTOR_COEFFICIENTS];
            let b_coeffs = &b_batch[start..start + MULTIVECTOR_COEFFICIENTS];

            // Inline geometric product computation for WASM optimization
            Self::geometric_product_hot_path(
                a_coeffs,
                b_coeffs,
                &mut result[start..start + MULTIVECTOR_COEFFICIENTS],
            );
        }

        Ok(result)
    }

    /// Hot path optimized geometric product for WASM
    /// Computes geometric product directly on coefficient slices
    #[inline(always)]
    fn geometric_product_hot_path(a: &[f64], b: &[f64], result: &mut [f64]) {
        // Manually unrolled geometric product for 3D Euclidean space
        // Based on multiplication table for Cl(3,0,0)

        // Clear result
        result.fill(0.0);

        // Scalar * all
        let a0 = a[0];
        if a0 != 0.0 {
            for i in 0..8 {
                result[i] += a0 * b[i];
            }
        }

        // e1 products (index 1)
        let a1 = a[1];
        if a1 != 0.0 {
            result[0] += a1 * b[1]; // e1 * 1 = e1, 1 * e1 = e1
            result[1] += a1 * b[0]; // 1 * e1 = e1
            result[2] += a1 * b[3]; // e1 * e2 = e12
            result[3] += a1 * b[2]; // e1 * e12 = e2
            result[4] += a1 * b[5]; // e1 * e3 = e13
            result[5] += a1 * b[4]; // e1 * e13 = e3
            result[6] -= a1 * b[7]; // e1 * e23 = -e123
            result[7] -= a1 * b[6]; // e1 * e123 = -e23
        }

        // e2 products (index 2)
        let a2 = a[2];
        if a2 != 0.0 {
            result[0] += a2 * b[2]; // e2 * 1 = e2
            result[1] -= a2 * b[3]; // e2 * e1 = -e12
            result[2] += a2 * b[0]; // 1 * e2 = e2
            result[3] -= a2 * b[1]; // e2 * e12 = -e1
            result[4] += a2 * b[6]; // e2 * e3 = e23
            result[5] += a2 * b[7]; // e2 * e13 = e123
            result[6] += a2 * b[4]; // e2 * e23 = e3
            result[7] += a2 * b[5]; // e2 * e123 = e13
        }

        // e3 products (index 4)
        let a4 = a[4];
        if a4 != 0.0 {
            result[0] += a4 * b[4]; // e3 * 1 = e3
            result[1] -= a4 * b[5]; // e3 * e1 = -e13
            result[2] -= a4 * b[6]; // e3 * e2 = -e23
            result[3] -= a4 * b[7]; // e3 * e12 = -e123
            result[4] += a4 * b[0]; // 1 * e3 = e3
            result[5] -= a4 * b[1]; // e3 * e13 = -e1
            result[6] -= a4 * b[2]; // e3 * e23 = -e2
            result[7] -= a4 * b[3]; // e3 * e123 = -e12
        }

        // e12 products (index 3)
        let a3 = a[3];
        if a3 != 0.0 {
            result[0] -= a3 * b[3]; // e12 * 1 = e12, e12^2 = -1
            result[1] += a3 * b[2]; // e12 * e1 = e2
            result[2] -= a3 * b[1]; // e12 * e2 = -e1
            result[3] += a3 * b[0]; // 1 * e12 = e12
            result[4] += a3 * b[7]; // e12 * e3 = e123
            result[5] -= a3 * b[6]; // e12 * e13 = -e23
            result[6] += a3 * b[5]; // e12 * e23 = e13
            result[7] += a3 * b[4]; // e12 * e123 = e3
        }

        // e13 products (index 5)
        let a5 = a[5];
        if a5 != 0.0 {
            result[0] -= a5 * b[5]; // e13^2 = -1
            result[1] += a5 * b[4]; // e13 * e1 = e3
            result[2] -= a5 * b[7]; // e13 * e2 = -e123
            result[3] += a5 * b[6]; // e13 * e12 = e23
            result[4] -= a5 * b[1]; // e13 * e3 = -e1
            result[5] += a5 * b[0]; // 1 * e13 = e13
            result[6] -= a5 * b[3]; // e13 * e23 = -e12
            result[7] -= a5 * b[2]; // e13 * e123 = -e2
        }

        // e23 products (index 6)
        let a6 = a[6];
        if a6 != 0.0 {
            result[0] -= a6 * b[6]; // e23^2 = -1
            result[1] += a6 * b[7]; // e23 * e1 = e123
            result[2] += a6 * b[4]; // e23 * e2 = e3
            result[3] -= a6 * b[5]; // e23 * e12 = -e13
            result[4] -= a6 * b[2]; // e23 * e3 = -e2
            result[5] += a6 * b[3]; // e23 * e13 = e12
            result[6] += a6 * b[0]; // 1 * e23 = e23
            result[7] += a6 * b[1]; // e23 * e123 = e1
        }

        // e123 products (index 7)
        let a7 = a[7];
        if a7 != 0.0 {
            result[0] -= a7 * b[7]; // e123^2 = -1
            result[1] -= a7 * b[6]; // e123 * e1 = -e23
            result[2] += a7 * b[5]; // e123 * e2 = e13
            result[3] -= a7 * b[4]; // e123 * e12 = -e3
            result[4] += a7 * b[3]; // e123 * e3 = e12
            result[5] -= a7 * b[2]; // e123 * e13 = -e2
            result[6] += a7 * b[1]; // e123 * e23 = e1
            result[7] += a7 * b[0]; // 1 * e123 = e123
        }
    }

    /// Batch addition
    #[wasm_bindgen(js_name = batchAdd)]
    pub fn batch_add(a_batch: &[f64], b_batch: &[f64]) -> Result<Vec<f64>, JsValue> {
        if a_batch.len() != b_batch.len() {
            return Err(JsValue::from_str("Batch arrays must have the same length"));
        }

        let mut result = Vec::with_capacity(a_batch.len());
        for i in 0..a_batch.len() {
            result.push(a_batch[i] + b_batch[i]);
        }

        Ok(result)
    }
}

/// Rotor operations for WASM
#[wasm_bindgen]
pub struct WasmRotor {
    inner: Rotor<3, 0, 0>,
}

#[wasm_bindgen]
impl WasmRotor {
    /// Create a rotor from a bivector and angle
    #[wasm_bindgen(js_name = fromBivector)]
    pub fn from_bivector(bivector: &WasmMultivector, angle: f64) -> WasmRotor {
        // Convert the WasmMultivector to a Bivector wrapper for type safety.
        // The Bivector type ensures only grade-2 components are used, providing
        // compile-time guarantees that the rotor is constructed from valid bivector data.
        let biv = Bivector::from_multivector(&bivector.inner);
        Self {
            inner: Rotor::from_bivector(&biv, angle),
        }
    }

    /// Apply rotor to a multivector
    pub fn apply(&self, mv: &WasmMultivector) -> WasmMultivector {
        WasmMultivector {
            inner: self.inner.apply(&mv.inner),
        }
    }

    /// Compose two rotors
    pub fn compose(&self, other: &WasmRotor) -> WasmRotor {
        Self {
            inner: self.inner.compose(&other.inner),
        }
    }

    /// Get inverse rotor
    pub fn inverse(&self) -> WasmRotor {
        Self {
            inner: self.inner.inverse(),
        }
    }
}

/// Memory pool for reducing allocation overhead in WASM
struct MemoryPool {
    coefficient_buffers: Vec<Vec<f64>>,
}

impl MemoryPool {
    fn new() -> Self {
        Self {
            coefficient_buffers: Vec::new(),
        }
    }

    fn get_buffer(&mut self) -> Vec<f64> {
        self.coefficient_buffers
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(MULTIVECTOR_COEFFICIENTS))
    }

    #[allow(dead_code)]
    fn return_buffer(&mut self, mut buffer: Vec<f64>) {
        if buffer.capacity() >= MULTIVECTOR_COEFFICIENTS {
            buffer.clear();
            if self.coefficient_buffers.len() < 16 {
                // Limit pool size
                self.coefficient_buffers.push(buffer);
            }
        }
    }
}

thread_local! {
    static MEMORY_POOL: RefCell<MemoryPool> = RefCell::new(MemoryPool::new());
}

/// High-performance WASM operations with memory pooling
#[wasm_bindgen]
pub struct PerformanceOperations;

#[wasm_bindgen]
impl PerformanceOperations {
    /// Fast geometric product for hot paths with memory pooling
    #[wasm_bindgen(js_name = fastGeometricProduct)]
    pub fn fast_geometric_product(lhs: &[f64], rhs: &[f64]) -> Vec<f64> {
        if lhs.len() != MULTIVECTOR_COEFFICIENTS || rhs.len() != MULTIVECTOR_COEFFICIENTS {
            return vec![0.0; MULTIVECTOR_COEFFICIENTS];
        }

        MEMORY_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            let mut result = pool.get_buffer();
            result.resize(MULTIVECTOR_COEFFICIENTS, 0.0);

            BatchOperations::geometric_product_hot_path(lhs, rhs, &mut result);

            // Don't return buffer to pool since we're returning it to JS
            result
        })
    }

    /// Optimized vector operations for 3D space
    #[wasm_bindgen(js_name = vectorCrossProduct)]
    pub fn vector_cross_product(v1: &[f64], v2: &[f64]) -> Vec<f64> {
        if v1.len() < 3 || v2.len() < 3 {
            return vec![0.0; 3];
        }

        vec![
            v1[1] * v2[2] - v1[2] * v2[1],
            v1[2] * v2[0] - v1[0] * v2[2],
            v1[0] * v2[1] - v1[1] * v2[0],
        ]
    }

    /// Optimized vector dot product
    #[wasm_bindgen(js_name = vectorDotProduct)]
    pub fn vector_dot_product(v1: &[f64], v2: &[f64]) -> f64 {
        let len = v1.len().min(v2.len());
        let mut result = 0.0;
        for i in 0..len {
            result += v1[i] * v2[i];
        }
        result
    }

    /// Batch normalize vectors for efficiency
    #[wasm_bindgen(js_name = batchNormalize)]
    pub fn batch_normalize(vectors: &[f64], vector_size: usize) -> Vec<f64> {
        let num_vectors = vectors.len() / vector_size;
        let mut result = Vec::with_capacity(vectors.len());

        for i in 0..num_vectors {
            let start = i * vector_size;
            let end = start + vector_size;
            let vector = &vectors[start..end];

            // Calculate magnitude
            let mag_sq: f64 = vector.iter().map(|x| x * x).sum();
            let mag = mag_sq.sqrt();

            if mag > 1e-14 {
                let inv_mag = 1.0 / mag;
                for &component in vector {
                    result.push(component * inv_mag);
                }
            } else {
                result.extend(vec![0.0; vector_size]);
            }
        }

        result
    }
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    // Initialize console error panic hook
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    console_log!("Amari WASM module initialized with complete mathematical computing support");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WasmMultivector Tests
    // ========================================================================

    #[test]
    fn test_multivector_new() {
        let mv = WasmMultivector::new();
        for i in 0..8 {
            assert_eq!(mv.get_coefficient(i), 0.0);
        }
    }

    #[test]
    fn test_multivector_scalar() {
        let mv = WasmMultivector::scalar(5.0);
        assert_eq!(mv.get_coefficient(0), 5.0);
        for i in 1..8 {
            assert_eq!(mv.get_coefficient(i), 0.0);
        }
    }

    #[test]
    fn test_multivector_basis_vector() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        assert_eq!(e1.get_coefficient(1), 1.0);

        let e2 = WasmMultivector::basis_vector(1).unwrap();
        assert_eq!(e2.get_coefficient(2), 1.0);

        let e3 = WasmMultivector::basis_vector(2).unwrap();
        assert_eq!(e3.get_coefficient(4), 1.0);
    }

    #[test]
    fn test_multivector_all_basis_vectors_valid() {
        // All indices 0, 1, 2 should be valid for 3D
        for i in 0..3 {
            let result = WasmMultivector::basis_vector(i);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_multivector_from_coefficients() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector::from_coefficients(&coeffs).unwrap();
        for i in 0..8 {
            assert_eq!(mv.get_coefficient(i), (i + 1) as f64);
        }
    }

    #[test]
    fn test_multivector_from_coefficients_correct_size() {
        // Test that exactly 8 coefficients works
        let coeffs = vec![0.0; 8];
        let result = WasmMultivector::from_coefficients(&coeffs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multivector_get_coefficients() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector::from_coefficients(&coeffs).unwrap();
        let retrieved = mv.get_coefficients();
        assert_eq!(retrieved, coeffs);
    }

    #[test]
    fn test_multivector_set_coefficient() {
        let mut mv = WasmMultivector::new();
        mv.set_coefficient(3, 42.0);
        assert_eq!(mv.get_coefficient(3), 42.0);
    }

    #[test]
    fn test_multivector_geometric_product_basis() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();

        // e1 * e2 = e12 (bivector at index 3)
        let e12 = e1.geometric_product(&e2);
        assert_eq!(e12.get_coefficient(3), 1.0);
    }

    #[test]
    fn test_multivector_geometric_product_self() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();

        // e1 * e1 = 1 (scalar)
        let result = e1.geometric_product(&e1);
        assert_eq!(result.get_coefficient(0), 1.0);
    }

    #[test]
    fn test_multivector_outer_product() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();

        let e12 = e1.outer_product(&e2);
        assert_eq!(e12.get_coefficient(3), 1.0);

        // Outer product is antisymmetric
        let e21 = e2.outer_product(&e1);
        assert_eq!(e21.get_coefficient(3), -1.0);
    }

    #[test]
    fn test_multivector_inner_product() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();

        // Inner product of orthogonal vectors is 0
        let result = e1.inner_product(&e2);
        assert_eq!(result.get_coefficient(0), 0.0);

        // Inner product of vector with itself is 1
        let self_inner = e1.inner_product(&e1);
        assert_eq!(self_inner.get_coefficient(0), 1.0);
    }

    #[test]
    fn test_multivector_scalar_product() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();

        // Scalar product of orthogonal vectors is 0
        assert_eq!(e1.scalar_product(&e2), 0.0);

        // Scalar product of vector with itself is 1
        assert_eq!(e1.scalar_product(&e1), 1.0);
    }

    #[test]
    fn test_multivector_reverse() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2);

        // Reverse of bivector changes sign
        let rev = e12.reverse();
        assert_eq!(rev.get_coefficient(3), -1.0);

        // Reverse of vector is unchanged
        let e1_rev = e1.reverse();
        assert_eq!(e1_rev.get_coefficient(1), 1.0);
    }

    #[test]
    fn test_multivector_grade_projection() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector::from_coefficients(&coeffs).unwrap();

        // Grade 0 projection (scalar)
        let scalar = mv.grade_projection(0);
        assert_eq!(scalar.get_coefficient(0), 1.0);
        assert_eq!(scalar.get_coefficient(1), 0.0);

        // Grade 1 projection (vectors)
        let vector = mv.grade_projection(1);
        assert_eq!(vector.get_coefficient(0), 0.0);
        assert_eq!(vector.get_coefficient(1), 2.0);
        assert_eq!(vector.get_coefficient(2), 3.0);
        assert_eq!(vector.get_coefficient(4), 5.0);
    }

    #[test]
    fn test_multivector_magnitude() {
        let scalar = WasmMultivector::scalar(3.0);
        assert!((scalar.magnitude() - 3.0).abs() < 1e-10);

        let e1 = WasmMultivector::basis_vector(0).unwrap();
        assert!((e1.magnitude() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multivector_normalize() {
        let coeffs = vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mv = WasmMultivector::from_coefficients(&coeffs).unwrap();

        let normalized = mv.normalize().unwrap();
        let mag = normalized.magnitude();
        assert!((mag - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multivector_exp_zero() {
        // exp(0) = 1
        let zero = WasmMultivector::new();
        let exp_zero = zero.exp();
        assert!((exp_zero.get_coefficient(0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multivector_exp_scalar() {
        // exp(scalar) should work
        let scalar = WasmMultivector::scalar(1.0);
        let exp_scalar = scalar.exp();
        // exp(1) ≈ 2.718
        assert!((exp_scalar.get_coefficient(0) - std::f64::consts::E).abs() < 1e-10);
    }

    // ========================================================================
    // WasmRotor Tests
    // ========================================================================

    #[test]
    fn test_rotor_creation() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2);

        let rotor = WasmRotor::from_bivector(&e12, std::f64::consts::PI / 2.0);
        // Just verify it doesn't panic
        let _ = rotor;
    }

    #[test]
    fn test_rotor_apply() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2);

        // 90 degree rotation in e12 plane: e1 -> e2
        let rotor = WasmRotor::from_bivector(&e12, std::f64::consts::PI / 2.0);
        let rotated = rotor.apply(&e1);

        // After 90 deg rotation, e1 should become approximately e2
        assert!(rotated.get_coefficient(2).abs() > 0.9);
    }

    #[test]
    fn test_rotor_compose() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2);

        let rotor45 = WasmRotor::from_bivector(&e12, std::f64::consts::PI / 4.0);
        let rotor90 = rotor45.compose(&rotor45);

        let rotated = rotor90.apply(&e1);
        // After 90 deg rotation, e1 should become approximately e2
        assert!(rotated.get_coefficient(2).abs() > 0.9);
    }

    #[test]
    fn test_rotor_inverse() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2);

        let rotor = WasmRotor::from_bivector(&e12, std::f64::consts::PI / 3.0);
        let inv = rotor.inverse();

        // Rotor * inverse should give identity
        let identity = rotor.compose(&inv);
        let result = identity.apply(&e1);

        // Should return to original e1
        assert!((result.get_coefficient(1) - 1.0).abs() < 1e-10);
    }

    // ========================================================================
    // BatchOperations Tests
    // ========================================================================

    #[test]
    fn test_batch_add() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];

        let result = BatchOperations::batch_add(&a, &b).unwrap();
        assert_eq!(result, vec![6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_batch_add_same_length() {
        // Test that same-length arrays work correctly
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

        let result = BatchOperations::batch_add(&a, &b).unwrap();
        assert_eq!(result, vec![9.0; 8]);
    }

    #[test]
    fn test_batch_geometric_product() {
        let e1_coeffs = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let e2_coeffs = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let mut result = vec![0.0; 8];
        BatchOperations::geometric_product_hot_path(&e1_coeffs, &e2_coeffs, &mut result);

        // e1 * e2 = e12
        assert_eq!(result[3], 1.0);
    }

    // ========================================================================
    // PerformanceOperations Tests
    // ========================================================================

    #[test]
    fn test_fast_geometric_product() {
        let e1_coeffs = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let e2_coeffs = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let result = PerformanceOperations::fast_geometric_product(&e1_coeffs, &e2_coeffs);
        assert_eq!(result[3], 1.0);
    }

    #[test]
    fn test_fast_geometric_product_wrong_size() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];

        let result = PerformanceOperations::fast_geometric_product(&a, &b);
        // Returns zeros for invalid input
        assert_eq!(result, vec![0.0; 8]);
    }

    #[test]
    fn test_vector_cross_product() {
        let v1 = vec![1.0, 0.0, 0.0]; // x-axis
        let v2 = vec![0.0, 1.0, 0.0]; // y-axis

        let cross = PerformanceOperations::vector_cross_product(&v1, &v2);
        // x × y = z
        assert_eq!(cross, vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_vector_cross_product_anticommutative() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];

        let cross_12 = PerformanceOperations::vector_cross_product(&v1, &v2);
        let cross_21 = PerformanceOperations::vector_cross_product(&v2, &v1);

        for i in 0..3 {
            assert!((cross_12[i] + cross_21[i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_vector_dot_product() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];

        let dot = PerformanceOperations::vector_dot_product(&v1, &v2);
        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert_eq!(dot, 32.0);
    }

    #[test]
    fn test_vector_dot_product_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];

        let dot = PerformanceOperations::vector_dot_product(&v1, &v2);
        assert_eq!(dot, 0.0);
    }

    #[test]
    fn test_batch_normalize() {
        let vectors = vec![3.0, 4.0, 0.0, 0.0, 0.0, 5.0];

        let result = PerformanceOperations::batch_normalize(&vectors, 3);

        // First vector [3, 4, 0] has magnitude 5, normalized to [0.6, 0.8, 0]
        assert!((result[0] - 0.6).abs() < 1e-10);
        assert!((result[1] - 0.8).abs() < 1e-10);
        assert_eq!(result[2], 0.0);

        // Second vector [0, 0, 5] normalized to [0, 0, 1]
        assert_eq!(result[3], 0.0);
        assert_eq!(result[4], 0.0);
        assert!((result[5] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_batch_normalize_zero_vector() {
        let vectors = vec![0.0, 0.0, 0.0];

        let result = PerformanceOperations::batch_normalize(&vectors, 3);
        assert_eq!(result, vec![0.0, 0.0, 0.0]);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_full_rotation_chain() {
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e3 = WasmMultivector::basis_vector(2).unwrap();

        // Create rotation in xy-plane
        let e12 = e1.outer_product(&e2);
        let rotor_xy = WasmRotor::from_bivector(&e12, std::f64::consts::PI / 2.0);

        // Create rotation in xz-plane
        let e13 = e1.outer_product(&e3);
        let rotor_xz = WasmRotor::from_bivector(&e13, std::f64::consts::PI / 2.0);

        // Compose rotations
        let combined = rotor_xy.compose(&rotor_xz);

        // Apply to e1 and verify it moved
        let result = combined.apply(&e1);
        let original_e1_component = result.get_coefficient(1);
        assert!(original_e1_component.abs() < 0.5); // Should have moved away from e1
    }

    #[test]
    fn test_clifford_algebra_identity() {
        // e1 * e1 = 1
        let e1 = WasmMultivector::basis_vector(0).unwrap();
        let e1_sq = e1.geometric_product(&e1);
        assert!((e1_sq.get_coefficient(0) - 1.0).abs() < 1e-10);

        // e12 * e12 = -1
        let e2 = WasmMultivector::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2);
        let e12_sq = e12.geometric_product(&e12);
        assert!((e12_sq.get_coefficient(0) + 1.0).abs() < 1e-10);

        // e123 * e123 = -1
        let e3 = WasmMultivector::basis_vector(2).unwrap();
        let e123 = e12.outer_product(&e3);
        let e123_sq = e123.geometric_product(&e123);
        assert!((e123_sq.get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_grade_decomposition_sums_to_original() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector::from_coefficients(&coeffs).unwrap();

        let grade0 = mv.grade_projection(0);
        let grade1 = mv.grade_projection(1);
        let grade2 = mv.grade_projection(2);
        let grade3 = mv.grade_projection(3);

        // Sum all grades
        let sum_coeffs = grade0
            .get_coefficients()
            .iter()
            .zip(grade1.get_coefficients().iter())
            .zip(grade2.get_coefficients().iter())
            .zip(grade3.get_coefficients().iter())
            .map(|(((a, b), c), d)| a + b + c + d)
            .collect::<Vec<_>>();

        for (i, &c) in sum_coeffs.iter().enumerate() {
            assert!((c - coeffs[i]).abs() < 1e-10);
        }
    }
}
