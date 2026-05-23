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
//! - **Arbitrary-signature GA** (amari-core::generic) - Runtime (p,q,r) multivectors and rotors (v0.23.0+)

use wasm_bindgen::prelude::*;

// Re-export the generic multivector and rotor types.
pub use generic::{WasmGenericMultivector, WasmGenericRotor};

// Optional modules
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
// Fast-path type aliases for common Clifford-algebra signatures.
//
// Each is a thin `#[wasm_bindgen]` wrapper around `WasmGenericMultivector`
// (or `WasmGenericRotor`) that pre-sets (p, q, r) and provides named
// basis-vector constructors for the TypeScript layer.
// ========================================================================

macro_rules! wasm_fastpath_multivector {
    ($struct_name:ident, $P:literal, $Q:literal, $R:literal, $dim_label:literal) => {
        #[wasm_bindgen]
        pub struct $struct_name {
            inner: WasmGenericMultivector,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[wasm_bindgen]
        impl $struct_name {
            /// Create a new zero multivector.
            #[wasm_bindgen(constructor)]
            pub fn new() -> Self {
                Self {
                    inner: WasmGenericMultivector::new($P, $Q, $R),
                }
            }

            /// Create from a Float64Array of coefficients.
            #[wasm_bindgen(js_name = fromCoefficients)]
            pub fn from_coefficients(coefficients: &[f64]) -> Result<Self, JsValue> {
                let inner = WasmGenericMultivector::from_coefficients($P, $Q, $R, coefficients)?;
                Ok(Self { inner })
            }

            /// Create a scalar multivector.
            #[wasm_bindgen(js_name = scalar)]
            pub fn scalar(value: f64) -> Self {
                Self {
                    inner: WasmGenericMultivector::scalar($P, $Q, $R, value),
                }
            }

            /// Create a basis vector (0-indexed).
            #[wasm_bindgen(js_name = basisVector)]
            pub fn basis_vector(index: usize) -> Result<Self, JsValue> {
                let inner = WasmGenericMultivector::basis_vector($P, $Q, $R, index)?;
                Ok(Self { inner })
            }

            // ---- coefficient access ----

            #[wasm_bindgen(js_name = getCoefficients)]
            pub fn get_coefficients(&self) -> Vec<f64> {
                self.inner.get_coefficients()
            }

            #[wasm_bindgen(js_name = getCoefficient)]
            pub fn get_coefficient(&self, index: usize) -> f64 {
                self.inner.get_coefficient(index)
            }

            #[wasm_bindgen(js_name = setCoefficient)]
            pub fn set_coefficient(&mut self, index: usize, value: f64) {
                self.inner.set_coefficient(index, value)
            }

            // ---- binary operations ----

            #[wasm_bindgen(js_name = geometricProduct)]
            pub fn geometric_product(&self, other: &Self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.geometric_product(&other.inner)?,
                })
            }

            #[wasm_bindgen(js_name = innerProduct)]
            pub fn inner_product(&self, other: &Self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.inner_product(&other.inner)?,
                })
            }

            #[wasm_bindgen(js_name = outerProduct)]
            pub fn outer_product(&self, other: &Self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.outer_product(&other.inner)?,
                })
            }

            #[wasm_bindgen(js_name = scalarProduct)]
            pub fn scalar_product(&self, other: &Self) -> Result<f64, JsValue> {
                self.inner.scalar_product(&other.inner)
            }

            // ---- unary operations ----

            pub fn reverse(&self) -> Self {
                Self {
                    inner: self.inner.reverse(),
                }
            }

            #[wasm_bindgen(js_name = gradeProjection)]
            pub fn grade_projection(&self, grade: usize) -> Self {
                Self {
                    inner: self.inner.grade_projection(grade),
                }
            }

            pub fn exp(&self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.exp()?,
                })
            }

            pub fn magnitude(&self) -> Result<f64, JsValue> {
                self.inner.magnitude()
            }

            pub fn norm(&self) -> Result<f64, JsValue> {
                self.magnitude()
            }

            pub fn normalize(&self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.normalize()?,
                })
            }

            pub fn inverse(&self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.inverse()?,
                })
            }

            // ---- arithmetic ----

            pub fn add(&self, other: &Self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.add(&other.inner)?,
                })
            }

            pub fn sub(&self, other: &Self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.sub(&other.inner)?,
                })
            }

            pub fn scale(&self, scalar: f64) -> Self {
                Self {
                    inner: self.inner.scale(scalar),
                }
            }
        }
    };
}

macro_rules! wasm_fastpath_rotor {
    ($rotor_name:ident, $mv_name:ident, $P:literal, $Q:literal, $R:literal) => {
        #[wasm_bindgen]
        pub struct $rotor_name {
            inner: WasmGenericRotor,
        }

        #[wasm_bindgen]
        impl $rotor_name {
            /// Create a rotor from a bivector and angle.
            #[wasm_bindgen(js_name = fromBivector)]
            pub fn from_bivector(bivector: &$mv_name, angle: f64) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: WasmGenericRotor::from_bivector(&bivector.inner, angle)?,
                })
            }

            /// Apply rotor to a multivector.
            pub fn apply(&self, mv: &$mv_name) -> Result<$mv_name, JsValue> {
                Ok($mv_name {
                    inner: self.inner.apply(&mv.inner)?,
                })
            }

            /// Compose two rotors.
            pub fn compose(&self, other: &Self) -> Result<Self, JsValue> {
                Ok(Self {
                    inner: self.inner.compose(&other.inner)?,
                })
            }

            /// Get inverse rotor.
            pub fn inverse(&self) -> Self {
                Self {
                    inner: self.inner.inverse(),
                }
            }
        }
    };
}

// ---- Fast-path aliases ----

// Euclidean 3D — Cl(3,0,0)
wasm_fastpath_multivector!(WasmMultivector300, 3, 0, 0, "3D Euclidean");
wasm_fastpath_rotor!(WasmRotor300, WasmMultivector300, 3, 0, 0);

// Spacetime 2+1 — Cl(2,1,0)
wasm_fastpath_multivector!(WasmMultivector210, 2, 1, 0, "2+1 spacetime");
wasm_fastpath_rotor!(WasmRotor210, WasmMultivector210, 2, 1, 0);

// Minkowski 3+1 — Cl(3,1,0)
wasm_fastpath_multivector!(WasmMultivector310, 3, 1, 0, "3+1 Minkowski");
wasm_fastpath_rotor!(WasmRotor310, WasmMultivector310, 3, 1, 0);

// Planar 2D — Cl(2,0,0)
wasm_fastpath_multivector!(WasmMultivector200, 2, 0, 0, "2D planar");
wasm_fastpath_rotor!(WasmRotor200, WasmMultivector200, 2, 0, 0);

// Quaternion — Cl(0,3,0)
wasm_fastpath_multivector!(WasmMultivector030, 0, 3, 0, "quaternion");
wasm_fastpath_rotor!(WasmRotor030, WasmMultivector030, 0, 3, 0);

// Conformal GA — Cl(4,1,0)
wasm_fastpath_multivector!(WasmMultivector410, 4, 1, 0, "CGA");
wasm_fastpath_rotor!(WasmRotor410, WasmMultivector410, 4, 1, 0);

// Euclidean 5D — Cl(5,0,0)
wasm_fastpath_multivector!(WasmMultivector500, 5, 0, 0, "5D Euclidean");
wasm_fastpath_rotor!(WasmRotor500, WasmMultivector500, 5, 0, 0);

// Split-complex / 1+1 spacetime — Cl(1,1,0)
wasm_fastpath_multivector!(WasmMultivector110, 1, 1, 0, "2D split");
wasm_fastpath_rotor!(WasmRotor110, WasmMultivector110, 1, 1, 0);

// ---- Backward-compatible type aliases ----

/// Alias for backward compatibility with pre-0.23.0 code.
pub type WasmMultivector = WasmMultivector300;
/// Alias for backward compatibility with pre-0.23.0 code.
pub type WasmRotor = WasmRotor300;

// ========================================================================
// Batch operations
// ========================================================================

/// Batch operations for multi-multivector workloads.
#[wasm_bindgen]
pub struct BatchOperations;

#[wasm_bindgen]
impl BatchOperations {
    /// Batch geometric product — generic signature.
    #[wasm_bindgen(js_name = batchGeometricProduct)]
    pub fn batch_geometric_product(
        p: usize, q: usize, r: usize,
        a_batch: &[f64],
        b_batch: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        let coef_count = 1 << (p + q + r);
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
            let mv_a = WasmGenericMultivector::from_coefficients(p, q, r, a)?;
            let mv_b = WasmGenericMultivector::from_coefficients(p, q, r, b)?;
            let mv_c = mv_a.geometric_product(&mv_b)?;
            let coeffs = mv_c.get_coefficients();
            result[start..start + coef_count].copy_from_slice(&coeffs);
        }
        Ok(result)
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

/// High-performance WASM operations.
#[wasm_bindgen]
pub struct PerformanceOperations;

#[wasm_bindgen]
impl PerformanceOperations {
    /// Fast geometric product for hot paths — generic signature.
    #[wasm_bindgen(js_name = fastGeometricProduct)]
    pub fn fast_geometric_product(
        p: usize, q: usize, r: usize,
        lhs: &[f64], rhs: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        let mv_a = WasmGenericMultivector::from_coefficients(p, q, r, lhs)?;
        let mv_b = WasmGenericMultivector::from_coefficients(p, q, r, rhs)?;
        Ok(mv_a.geometric_product(&mv_b)?.get_coefficients())
    }

    /// Optimized vector cross product for 3D space.
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

    /// Optimized vector dot product.
    #[wasm_bindgen(js_name = vectorDotProduct)]
    pub fn vector_dot_product(v1: &[f64], v2: &[f64]) -> f64 {
        let len = v1.len().min(v2.len());
        let mut result = 0.0;
        for i in 0..len {
            result += v1[i] * v2[i];
        }
        result
    }

    /// Batch normalize vectors for efficiency.
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

    // ---- WasmMultivector300 (Cl 3,0,0) ----

    #[test]
    fn test_multivector_new() {
        let mv = WasmMultivector300::new();
        for i in 0..8 {
            assert_eq!(mv.get_coefficient(i), 0.0);
        }
    }

    #[test]
    fn test_multivector_scalar() {
        let mv = WasmMultivector300::scalar(5.0);
        assert_eq!(mv.get_coefficient(0), 5.0);
        for i in 1..8 {
            assert_eq!(mv.get_coefficient(i), 0.0);
        }
    }

    #[test]
    fn test_multivector_basis_vector() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        assert_eq!(e1.get_coefficient(1), 1.0);

        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        assert_eq!(e2.get_coefficient(2), 1.0);

        let e3 = WasmMultivector300::basis_vector(2).unwrap();
        assert_eq!(e3.get_coefficient(4), 1.0);
    }

    #[test]
    fn test_multivector_all_basis_vectors_valid() {
        for i in 0..3 {
            assert!(WasmMultivector300::basis_vector(i).is_ok());
        }
    }

    #[test]
    fn test_multivector_from_coefficients() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector300::from_coefficients(&coeffs).unwrap();
        for i in 0..8 {
            assert_eq!(mv.get_coefficient(i), (i + 1) as f64);
        }
    }

    #[test]
    fn test_multivector_from_coefficients_correct_size() {
        let coeffs = vec![0.0; 8];
        assert!(WasmMultivector300::from_coefficients(&coeffs).is_ok());
    }

    #[test]
    fn test_multivector_get_coefficients() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector300::from_coefficients(&coeffs).unwrap();
        assert_eq!(mv.get_coefficients(), coeffs);
    }

    #[test]
    fn test_multivector_set_coefficient() {
        let mut mv = WasmMultivector300::new();
        mv.set_coefficient(3, 42.0);
        assert_eq!(mv.get_coefficient(3), 42.0);
    }

    #[test]
    fn test_multivector_geometric_product_basis() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.geometric_product(&e2).unwrap();
        assert_eq!(e12.get_coefficient(3), 1.0);
    }

    #[test]
    fn test_multivector_geometric_product_self() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let result = e1.geometric_product(&e1).unwrap();
        assert_eq!(result.get_coefficient(0), 1.0);
    }

    #[test]
    fn test_multivector_outer_product() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        assert_eq!(e12.get_coefficient(3), 1.0);
        let e21 = e2.outer_product(&e1).unwrap();
        assert_eq!(e21.get_coefficient(3), -1.0);
    }

    #[test]
    fn test_multivector_inner_product() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let result = e1.inner_product(&e2).unwrap();
        assert_eq!(result.get_coefficient(0), 0.0);
        let self_inner = e1.inner_product(&e1).unwrap();
        assert_eq!(self_inner.get_coefficient(0), 1.0);
    }

    #[test]
    fn test_multivector_scalar_product() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        assert_eq!(e1.scalar_product(&e2).unwrap(), 0.0);
        assert_eq!(e1.scalar_product(&e1).unwrap(), 1.0);
    }

    #[test]
    fn test_multivector_reverse() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let rev = e12.reverse();
        assert_eq!(rev.get_coefficient(3), -1.0);
        let e1_rev = e1.reverse();
        assert_eq!(e1_rev.get_coefficient(1), 1.0);
    }

    #[test]
    fn test_multivector_grade_projection() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector300::from_coefficients(&coeffs).unwrap();
        let scalar = mv.grade_projection(0);
        assert_eq!(scalar.get_coefficient(0), 1.0);
        assert_eq!(scalar.get_coefficient(1), 0.0);
        let vector = mv.grade_projection(1);
        assert_eq!(vector.get_coefficient(0), 0.0);
        assert_eq!(vector.get_coefficient(1), 2.0);
        assert_eq!(vector.get_coefficient(2), 3.0);
        assert_eq!(vector.get_coefficient(4), 5.0);
    }

    #[test]
    fn test_multivector_magnitude() {
        let scalar = WasmMultivector300::scalar(3.0);
        assert!((scalar.magnitude().unwrap() - 3.0).abs() < 1e-10);
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        assert!((e1.magnitude().unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multivector_normalize() {
        let coeffs = vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mv = WasmMultivector300::from_coefficients(&coeffs).unwrap();
        let normalized = mv.normalize().unwrap();
        let mag = normalized.magnitude().unwrap();
        assert!((mag - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multivector_exp_zero() {
        let zero = WasmMultivector300::new();
        let exp_zero = zero.exp().unwrap();
        assert!((exp_zero.get_coefficient(0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multivector_exp_scalar() {
        let scalar = WasmMultivector300::scalar(1.0);
        let exp_scalar = scalar.exp().unwrap();
        assert!((exp_scalar.get_coefficient(0) - std::f64::consts::E).abs() < 1e-10);
    }

    // ---- WasmRotor300 ----

    #[test]
    fn test_rotor_creation() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let rotor = WasmRotor300::from_bivector(&e12, std::f64::consts::PI / 2.0);
        assert!(rotor.is_ok());
    }

    #[test]
    fn test_rotor_apply() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let rotor = WasmRotor300::from_bivector(&e12, std::f64::consts::PI / 2.0).unwrap();
        let rotated = rotor.apply(&e1).unwrap();
        assert!(rotated.get_coefficient(2).abs() > 0.9);
    }

    #[test]
    fn test_rotor_compose() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let rotor45 = WasmRotor300::from_bivector(&e12, std::f64::consts::PI / 4.0).unwrap();
        let rotor90 = rotor45.compose(&rotor45).unwrap();
        let rotated = rotor90.apply(&e1).unwrap();
        assert!(rotated.get_coefficient(2).abs() > 0.9);
    }

    #[test]
    fn test_rotor_inverse() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let rotor = WasmRotor300::from_bivector(&e12, std::f64::consts::PI / 3.0).unwrap();
        let inv = rotor.inverse();
        let identity = rotor.compose(&inv).unwrap();
        let result = identity.apply(&e1).unwrap();
        assert!((result.get_coefficient(1) - 1.0).abs() < 1e-10);
    }

    // ---- Fast-path: Cl(2,1,0) ----

    #[test]
    fn test_multivector210_basis_squares() {
        let e1 = WasmMultivector210::basis_vector(0).unwrap();
        let e2 = WasmMultivector210::basis_vector(1).unwrap();
        let e3 = WasmMultivector210::basis_vector(2).unwrap();
        assert!((e1.geometric_product(&e1).unwrap().get_coefficient(0) - 1.0).abs() < 1e-10);
        assert!((e2.geometric_product(&e2).unwrap().get_coefficient(0) - 1.0).abs() < 1e-10);
        assert!((e3.geometric_product(&e3).unwrap().get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    // ---- Fast-path: Cl(3,1,0) Minkowski ----

    #[test]
    fn test_multivector310_basis_squares() {
        let e3 = WasmMultivector310::basis_vector(3).unwrap();
        assert!((e3.geometric_product(&e3).unwrap().get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    // ---- Fast-path: Cl(2,0,0) Planar ----

    #[test]
    fn test_multivector200_basis_squares() {
        let e1 = WasmMultivector200::basis_vector(0).unwrap();
        let e2 = WasmMultivector200::basis_vector(1).unwrap();
        assert!((e1.geometric_product(&e1).unwrap().get_coefficient(0) - 1.0).abs() < 1e-10);
        assert!((e2.geometric_product(&e2).unwrap().get_coefficient(0) - 1.0).abs() < 1e-10);
    }

    // ---- Fast-path: Cl(0,3,0) Quaternion ----

    #[test]
    fn test_multivector030_basis_squares() {
        let e1 = WasmMultivector030::basis_vector(0).unwrap();
        let e2 = WasmMultivector030::basis_vector(1).unwrap();
        let e3 = WasmMultivector030::basis_vector(2).unwrap();
        assert!((e1.geometric_product(&e1).unwrap().get_coefficient(0) + 1.0).abs() < 1e-10);
        assert!((e2.geometric_product(&e2).unwrap().get_coefficient(0) + 1.0).abs() < 1e-10);
        assert!((e3.geometric_product(&e3).unwrap().get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    // ---- Fast-path: Cl(1,1,0) Split-complex ----

    #[test]
    fn test_multivector110_basis_squares() {
        let e1 = WasmMultivector110::basis_vector(0).unwrap();
        let e2 = WasmMultivector110::basis_vector(1).unwrap();
        assert!((e1.geometric_product(&e1).unwrap().get_coefficient(0) - 1.0).abs() < 1e-10);
        assert!((e2.geometric_product(&e2).unwrap().get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    // ---- BatchOperations ----

    #[test]
    fn test_batch_add() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let result = BatchOperations::batch_add(&a, &b).unwrap();
        assert_eq!(result, vec![6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_batch_geometric_product_single() {
        let a = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e1
        let b = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e2
        let result = BatchOperations::batch_geometric_product(3, 0, 0, &a, &b).unwrap();
        assert_eq!(result[3], 1.0); // e1*e2 = e12
    }

    // ---- PerformanceOperations ----

    #[test]
    fn test_fast_geometric_product() {
        let e1 = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let e2 = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let result = PerformanceOperations::fast_geometric_product(3, 0, 0, &e1, &e2).unwrap();
        assert_eq!(result[3], 1.0);
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_fast_geometric_product_wrong_size() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = PerformanceOperations::fast_geometric_product(3, 0, 0, &a, &b);
        assert!(result.is_err());
    }

    #[test]
    fn test_vector_cross_product() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let cross = PerformanceOperations::vector_cross_product(&v1, &v2);
        assert_eq!(cross, vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_vector_dot_product() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];
        let dot = PerformanceOperations::vector_dot_product(&v1, &v2);
        assert_eq!(dot, 32.0);
    }

    #[test]
    fn test_batch_normalize() {
        let vectors = vec![3.0, 4.0, 0.0, 0.0, 0.0, 5.0];
        let result = PerformanceOperations::batch_normalize(&vectors, 3);
        assert!((result[0] - 0.6).abs() < 1e-10);
        assert!((result[1] - 0.8).abs() < 1e-10);
        assert_eq!(result[2], 0.0);
        assert_eq!(result[3], 0.0);
        assert_eq!(result[4], 0.0);
        assert!((result[5] - 1.0).abs() < 1e-10);
    }

    // ---- Integration ----

    #[test]
    fn test_full_rotation_chain() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e3 = WasmMultivector300::basis_vector(2).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let e13 = e1.outer_product(&e3).unwrap();
        let rotor_xy = WasmRotor300::from_bivector(&e12, std::f64::consts::PI / 2.0).unwrap();
        let rotor_xz = WasmRotor300::from_bivector(&e13, std::f64::consts::PI / 2.0).unwrap();
        let combined = rotor_xy.compose(&rotor_xz).unwrap();
        let result = combined.apply(&e1).unwrap();
        assert!(result.get_coefficient(1).abs() < 0.5);
    }

    #[test]
    fn test_clifford_algebra_identity() {
        let e1 = WasmMultivector300::basis_vector(0).unwrap();
        let e1_sq = e1.geometric_product(&e1).unwrap();
        assert!((e1_sq.get_coefficient(0) - 1.0).abs() < 1e-10);

        let e2 = WasmMultivector300::basis_vector(1).unwrap();
        let e12 = e1.outer_product(&e2).unwrap();
        let e12_sq = e12.geometric_product(&e12).unwrap();
        assert!((e12_sq.get_coefficient(0) + 1.0).abs() < 1e-10);

        let e3 = WasmMultivector300::basis_vector(2).unwrap();
        let e123 = e12.outer_product(&e3).unwrap();
        let e123_sq = e123.geometric_product(&e123).unwrap();
        assert!((e123_sq.get_coefficient(0) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_grade_decomposition_sums_to_original() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = WasmMultivector300::from_coefficients(&coeffs).unwrap();
        let grade0 = mv.grade_projection(0);
        let grade1 = mv.grade_projection(1);
        let grade2 = mv.grade_projection(2);
        let grade3 = mv.grade_projection(3);
        let sum: Vec<f64> = (0..8)
            .map(|i| {
                grade0.get_coefficient(i)
                    + grade1.get_coefficient(i)
                    + grade2.get_coefficient(i)
                    + grade3.get_coefficient(i)
            })
            .collect();
        for (i, &c) in sum.iter().enumerate() {
            assert!((c - coeffs[i]).abs() < 1e-10);
        }
    }
}
