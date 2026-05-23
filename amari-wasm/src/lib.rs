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

use amari_core::{rotor::Rotor, Bivector, Multivector};
use wasm_bindgen::prelude::*;

// Optional modules - some enabled for expanded WASM functionality
pub mod automata;
pub mod calculus;
pub mod cgt;
pub mod dual;
pub mod enumerative;
pub mod flynn;
pub mod functional;
pub mod fusion;
pub mod generic;
pub mod gf2;
pub mod info_geom;
pub mod measure;
pub mod network;
pub mod optical;
pub mod optimization;
pub mod probabilistic;
pub mod relativistic;
pub mod surreal;
pub mod topology;
pub mod tropical;

/// Console logging utility
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// ========================================================================
// WASM Multivector macro — generates a concrete Multivector wrapper for a
// given Clifford-algebra signature Cl(P,Q,R).
// ========================================================================

/// Generate a `#[wasm_bindgen]` multivector struct for signature Cl(P,Q,R).
///
/// Parameters:
/// - `$struct_name`: the Rust struct name (e.g., `WasmMultivector`)
/// - `$P`, `$Q`, `$R`: metric signature counts (+ / − / 0)
/// - `$dim_label`: human label used in error messages (e.g., `"3D Euclidean"`)
macro_rules! wasm_multivector {
    ($struct_name:ident, $P:literal, $Q:literal, $R:literal, $dim_label:literal) => {
        #[wasm_bindgen]
        pub struct $struct_name {
            inner: Multivector<$P, $Q, $R>,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[wasm_bindgen]
        impl $struct_name {
            /// Create a new zero multivector
            #[wasm_bindgen(constructor)]
            pub fn new() -> Self {
                Self {
                    inner: Multivector::zero(),
                }
            }

            /// Create from a Float64Array of coefficients
            #[wasm_bindgen(js_name = fromCoefficients)]
            pub fn from_coefficients(coefficients: &[f64]) -> Result<Self, JsValue> {
                let expected = Multivector::<$P, $Q, $R>::BASIS_COUNT;
                if coefficients.len() != expected {
                    return Err(JsValue::from_str(&format!(
                        "{} Clifford algebra requires exactly {} coefficients",
                        $dim_label, expected
                    )));
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
            pub fn basis_vector(index: usize) -> Result<Self, JsValue> {
                let dim = Multivector::<$P, $Q, $R>::DIM;
                if index >= dim {
                    return Err(JsValue::from_str(&format!(
                        "Basis vector index must be 0..{} for this signature",
                        dim
                    )));
                }
                Ok(Self {
                    inner: Multivector::basis_vector(index),
                })
            }

            /// Get coefficients as a Float64Array
            #[wasm_bindgen(js_name = getCoefficients)]
            pub fn get_coefficients(&self) -> Vec<f64> {
                let count = Multivector::<$P, $Q, $R>::BASIS_COUNT;
                let mut coeffs = vec![0.0; count];
                for i in 0..count {
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

            /// Geometric product — delegates to amari-core's generic implementation
            #[wasm_bindgen(js_name = geometricProduct)]
            pub fn geometric_product(&self, other: &Self) -> Self {
                Self {
                    inner: self.inner.geometric_product(&other.inner),
                }
            }

            /// Inner product (dot product for vectors)
            #[wasm_bindgen(js_name = innerProduct)]
            pub fn inner_product(&self, other: &Self) -> Self {
                Self {
                    inner: self.inner.inner_product(&other.inner),
                }
            }

            /// Outer product (wedge product)
            #[wasm_bindgen(js_name = outerProduct)]
            pub fn outer_product(&self, other: &Self) -> Self {
                Self {
                    inner: self.inner.outer_product(&other.inner),
                }
            }

            /// Scalar product
            #[wasm_bindgen(js_name = scalarProduct)]
            pub fn scalar_product(&self, other: &Self) -> f64 {
                self.inner.scalar_product(&other.inner)
            }

            /// Reverse
            pub fn reverse(&self) -> Self {
                Self {
                    inner: self.inner.reverse(),
                }
            }

            /// Grade projection
            #[wasm_bindgen(js_name = gradeProjection)]
            pub fn grade_projection(&self, grade: usize) -> Self {
                Self {
                    inner: self.inner.grade_projection(grade),
                }
            }

            /// Exponential (for bivectors to create rotors)
            pub fn exp(&self) -> Self {
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
            pub fn normalize(&self) -> Result<Self, JsValue> {
                self.inner
                    .normalize()
                    .map(|mv| Self { inner: mv })
                    .ok_or_else(|| JsValue::from_str("Cannot normalize zero multivector"))
            }

            /// Compute inverse
            pub fn inverse(&self) -> Result<Self, JsValue> {
                self.inner
                    .inverse()
                    .map(|mv| Self { inner: mv })
                    .ok_or_else(|| JsValue::from_str("Multivector is not invertible"))
            }

            /// Add two multivectors
            pub fn add(&self, other: &Self) -> Self {
                Self {
                    inner: &self.inner + &other.inner,
                }
            }

            /// Subtract two multivectors
            pub fn sub(&self, other: &Self) -> Self {
                Self {
                    inner: &self.inner - &other.inner,
                }
            }

            /// Scale by a scalar
            pub fn scale(&self, scalar: f64) -> Self {
                Self {
                    inner: &self.inner * scalar,
                }
            }
        }
    };
}

// Invoke the macro for the two supported signatures.

wasm_multivector!(WasmMultivector, 3, 0, 0, "3D Euclidean");
wasm_multivector!(WasmSpacetimeMultivector, 2, 1, 0, "2+1 spacetime");

// ========================================================================
// WASM Rotor macro — generates a Rotor wrapper for a given signature.
// ========================================================================

/// Generate a `#[wasm_bindgen]` rotor struct matching a specific multivector
/// wrapper and Clifford-algebra signature.
macro_rules! wasm_rotor {
    ($rotor_name:ident, $mv_name:ident, $P:literal, $Q:literal, $R:literal) => {
        #[wasm_bindgen]
        pub struct $rotor_name {
            inner: Rotor<$P, $Q, $R>,
        }

        #[wasm_bindgen]
        impl $rotor_name {
            /// Create a rotor from a bivector and angle
            #[wasm_bindgen(js_name = fromBivector)]
            pub fn from_bivector(bivector: &$mv_name, angle: f64) -> Self {
                let biv = Bivector::from_multivector(&bivector.inner);
                Self {
                    inner: Rotor::from_bivector(&biv, angle),
                }
            }

            /// Apply rotor to a multivector
            pub fn apply(&self, mv: &$mv_name) -> $mv_name {
                $mv_name {
                    inner: self.inner.apply(&mv.inner),
                }
            }

            /// Compose two rotors
            pub fn compose(&self, other: &Self) -> Self {
                Self {
                    inner: self.inner.compose(&other.inner),
                }
            }

            /// Get inverse rotor
            pub fn inverse(&self) -> Self {
                Self {
                    inner: self.inner.inverse(),
                }
            }
        }
    };
}

wasm_rotor!(WasmRotor, WasmMultivector, 3, 0, 0);
wasm_rotor!(WasmSpacetimeRotor, WasmSpacetimeMultivector, 2, 1, 0);

// ========================================================================
// Batch operations — use the generic Multivector::geometric_product.
// ========================================================================

/// Helper: perform a single generic geometric product on coefficient slices.
/// Constructs temporary Multivectors, computes the product, and writes result.
fn generic_geometric_product<const P: usize, const Q: usize, const R: usize>(
    a: &[f64],
    b: &[f64],
    result: &mut [f64],
) {
    let basis_count = Multivector::<P, Q, R>::BASIS_COUNT;
    let mv_a = Multivector::<P, Q, R>::from_coefficients(a[..basis_count].to_vec());
    let mv_b = Multivector::<P, Q, R>::from_coefficients(b[..basis_count].to_vec());
    let mv_result = mv_a.geometric_product(&mv_b);
    for i in 0..basis_count {
        result[i] = mv_result.get(i);
    }
}

/// Helper: batch-product for a given signature.
fn batch_product<const P: usize, const Q: usize, const R: usize>(
    a_batch: &[f64],
    b_batch: &[f64],
) -> Result<Vec<f64>, JsValue> {
    let coef_count = Multivector::<P, Q, R>::BASIS_COUNT;
    let batch_size = a_batch.len() / coef_count;

    if !a_batch.len().is_multiple_of(coef_count) || !b_batch.len().is_multiple_of(coef_count) {
        return Err(JsValue::from_str(
            "Batch arrays must have length divisible by multivector coefficients",
        ));
    }
    if a_batch.len() != b_batch.len() {
        return Err(JsValue::from_str("Batch arrays must have the same length"));
    }

    let mut result = vec![0.0; a_batch.len()];
    for i in 0..batch_size {
        let start = i * coef_count;
        let a = &a_batch[start..start + coef_count];
        let b = &b_batch[start..start + coef_count];
        generic_geometric_product::<P, Q, R>(a, b, &mut result[start..start + coef_count]);
    }
    Ok(result)
}

/// Helper: fast single-product for a given signature.
fn fast_product<const P: usize, const Q: usize, const R: usize>(
    lhs: &[f64],
    rhs: &[f64],
) -> Vec<f64> {
    let basis_count = Multivector::<P, Q, R>::BASIS_COUNT;
    if lhs.len() != basis_count || rhs.len() != basis_count {
        return vec![0.0; basis_count];
    }
    let mut result = vec![0.0; basis_count];
    generic_geometric_product::<P, Q, R>(lhs, rhs, &mut result);
    result
}

/// Batch operations for multi-multivector workloads.
#[wasm_bindgen]
pub struct BatchOperations;

#[wasm_bindgen]
impl BatchOperations {
    /// Batch geometric product for Cl(3,0,0) — compute a[i] * b[i] for all i.
    #[wasm_bindgen(js_name = batchGeometricProduct)]
    pub fn batch_geometric_product(a_batch: &[f64], b_batch: &[f64]) -> Result<Vec<f64>, JsValue> {
        batch_product::<3, 0, 0>(a_batch, b_batch)
    }

    /// Batch geometric product for Cl(2,1,0) — compute a[i] * b[i] for all i.
    #[wasm_bindgen(js_name = batchGeometricProductSpacetime)]
    pub fn batch_geometric_product_spacetime(
        a_batch: &[f64],
        b_batch: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        batch_product::<2, 1, 0>(a_batch, b_batch)
    }

    /// Batch addition (independent of signature).
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

// ========================================================================
// High-performance WASM operations
// ========================================================================

/// High-performance WASM operations with memory pooling.
#[wasm_bindgen]
pub struct PerformanceOperations;

#[wasm_bindgen]
impl PerformanceOperations {
    /// Fast geometric product for hot paths — Cl(3,0,0) Euclidean.
    #[wasm_bindgen(js_name = fastGeometricProduct)]
    pub fn fast_geometric_product(lhs: &[f64], rhs: &[f64]) -> Vec<f64> {
        fast_product::<3, 0, 0>(lhs, rhs)
    }

    /// Fast geometric product for hot paths — Cl(2,1,0) spacetime.
    #[wasm_bindgen(js_name = fastGeometricProductSpacetime)]
    pub fn fast_geometric_product_spacetime(lhs: &[f64], rhs: &[f64]) -> Vec<f64> {
        fast_product::<2, 1, 0>(lhs, rhs)
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
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log!("Amari WASM module initialized with complete mathematical computing support");
}

// ========================================================================
// Tests
// ========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WasmMultivector (Cl(3,0,0)) Tests
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

        assert!((result.get_coefficient(1) - 1.0).abs() < 1e-10);
    }

    // ========================================================================
    // WasmSpacetimeMultivector (Cl(2,1,0)) Tests
    // ========================================================================

    #[test]
    fn test_spacetime_multivector_new() {
        let mv = WasmSpacetimeMultivector::new();
        for i in 0..8 {
            assert_eq!(mv.get_coefficient(i), 0.0);
        }
    }

    #[test]
    fn test_spacetime_multivector_scalar() {
        let mv = WasmSpacetimeMultivector::scalar(5.0);
        assert_eq!(mv.get_coefficient(0), 5.0);
        for i in 1..8 {
            assert_eq!(mv.get_coefficient(i), 0.0);
        }
    }

    #[test]
    fn test_spacetime_multivector_basis_vector() {
        let e1 = WasmSpacetimeMultivector::basis_vector(0).unwrap();
        assert_eq!(e1.get_coefficient(1), 1.0);

        let e2 = WasmSpacetimeMultivector::basis_vector(1).unwrap();
        assert_eq!(e2.get_coefficient(2), 1.0);

        let e3 = WasmSpacetimeMultivector::basis_vector(2).unwrap();
        assert_eq!(e3.get_coefficient(4), 1.0);
    }

    #[test]
    fn test_spacetime_basis_vector_geometric_product_self() {
        // In Cl(2,1,0): e1^2 = +1, e2^2 = +1, e3^2 = -1
        let e1 = WasmSpacetimeMultivector::basis_vector(0).unwrap();
        let e2 = WasmSpacetimeMultivector::basis_vector(1).unwrap();
        let e3 = WasmSpacetimeMultivector::basis_vector(2).unwrap();

        // Positive signature basis vectors square to +1
        let e1_sq = e1.geometric_product(&e1);
        assert!((e1_sq.get_coefficient(0) - 1.0).abs() < 1e-10);

        let e2_sq = e2.geometric_product(&e2);
        assert!((e2_sq.get_coefficient(0) - 1.0).abs() < 1e-10);

        // Negative signature basis vector squares to -1
        let e3_sq = e3.geometric_product(&e3);
        assert!((e3_sq.get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_spacetime_geometric_product_basis() {
        let e1 = WasmSpacetimeMultivector::basis_vector(0).unwrap();
        let e2 = WasmSpacetimeMultivector::basis_vector(1).unwrap();
        // e1 * e2 = e12
        let e12 = e1.geometric_product(&e2);
        assert_eq!(e12.get_coefficient(3), 1.0);
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
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

        let result = BatchOperations::batch_add(&a, &b).unwrap();
        assert_eq!(result, vec![9.0; 8]);
    }

    #[test]
    fn test_batch_geometric_product_single() {
        let a = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e1
        let b = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e2

        let result = BatchOperations::batch_geometric_product(&a, &b).unwrap();
        // e1 * e2 = e12 (index 3)
        assert_eq!(result[3], 1.0);
    }

    #[test]
    fn test_batch_geometric_product_spacetime_single() {
        let a = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e1 (+)
        let b = vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]; // e3 (−)

        let result = BatchOperations::batch_geometric_product_spacetime(&a, &b).unwrap();
        // e1 * e3 = e13 (index 5) — same product as Euclidean since metric sign only matters for squares
        assert_eq!(result[5], 1.0);
    }

    // ========================================================================
    // PerformanceOperations Tests
    // ========================================================================

    #[test]
    fn test_fast_geometric_product() {
        let e1 = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let e2 = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let result = PerformanceOperations::fast_geometric_product(&e1, &e2);
        assert_eq!(result[3], 1.0);
    }

    #[test]
    fn test_fast_geometric_product_spacetime_squares() {
        // In Cl(2,1,0): e3 (index 2) squares to -1
        let e3 = vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

        let result = PerformanceOperations::fast_geometric_product_spacetime(&e3, &e3);
        // e3^2 = -1
        assert!((result[0] + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_fast_geometric_product_wrong_size() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];

        let result = PerformanceOperations::fast_geometric_product(&a, &b);
        // Returns zeros for invalid input (basis_count=8)
        assert_eq!(result, vec![0.0; 8]);
    }

    #[test]
    fn test_vector_cross_product() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];

        let cross = PerformanceOperations::vector_cross_product(&v1, &v2);
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
        assert!(original_e1_component.abs() < 0.5);
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
    fn test_spacetime_clifford_identity() {
        let e1 = WasmSpacetimeMultivector::basis_vector(0).unwrap();
        let e2 = WasmSpacetimeMultivector::basis_vector(1).unwrap();
        let e3 = WasmSpacetimeMultivector::basis_vector(2).unwrap();

        // e1^2 = +1, e2^2 = +1, e3^2 = -1
        assert!((e1.geometric_product(&e1).get_coefficient(0) - 1.0).abs() < 1e-10);
        assert!((e2.geometric_product(&e2).get_coefficient(0) - 1.0).abs() < 1e-10);
        assert!((e3.geometric_product(&e3).get_coefficient(0) + 1.0).abs() < 1e-10);

        // e12^2 = -1 (two positive vectors: +1 * +1 * sign(e12^2) = +1 * sign = -1 → sign = -1)
        let e12 = e1.outer_product(&e2);
        assert!((e12.geometric_product(&e12).get_coefficient(0) + 1.0).abs() < 1e-10);

        // e13^2: e1(+) * e3(−) → geometric: (+1)*(-1) = -1, and the bivector square sign is -1, so (-1)*(-1)=+1? No... let me check.
        // The Multivector::geometric_product should correctly compute this.
        // For Cl(2,1,0): e13^2 = e1*e3*e1*e3 = e1*(-e1*e3)*e3 = -e1*e1*e3*e3 = -(+1)*(-1) = +1
        let e13 = e1.outer_product(&e3);
        let e13_sq = e13.geometric_product(&e13).get_coefficient(0);
        assert!((e13_sq - 1.0).abs() < 1e-10,
            "Expected e13^2 = +1 in Cl(2,1,0), got {}", e13_sq);
    }

    #[test]
    fn test_grade_decomposition_sums_to_original() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector::from_coefficients(&coeffs).unwrap();

        let grade0 = mv.grade_projection(0);
        let grade1 = mv.grade_projection(1);
        let grade2 = mv.grade_projection(2);
        let grade3 = mv.grade_projection(3);

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
