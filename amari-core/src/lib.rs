//! High-performance Geometric Algebra/Clifford Algebra library
//!
//! This crate provides the core implementation of Clifford algebras with arbitrary signatures,
//! supporting the geometric product and related operations fundamental to geometric algebra.
//!
//! # Basis Blade Indexing
//!
//! Basis blades are indexed using binary representation where bit i indicates whether
//! basis vector e_i is present in the blade. For example:
//! - 0b001 (1) = e1
//! - 0b010 (2) = e2
//! - 0b011 (3) = e1 ∧ e2 (bivector)
//! - 0b111 (7) = e1 ∧ e2 ∧ e3 (trivector)

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Add, Mul, Neg, Sub};
use num_traits::Zero;

#[allow(dead_code)]
pub(crate) mod aligned_alloc;
pub mod basis;
pub mod cayley;
pub mod error;
pub mod generic;
pub mod precision;
pub mod rotor;
pub mod unicode_ops;

#[cfg(feature = "gf2")]
pub mod gf2;

#[allow(dead_code)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod simd;

// Re-export error types
pub use error::{CoreError, CoreResult};

// Re-export precision types for high-precision arithmetic
pub use precision::{PrecisionFloat, StandardFloat};

#[cfg(any(
    feature = "high-precision",
    feature = "wasm-precision",
    feature = "native-precision"
))]
pub use precision::ExtendedFloat;

// For backward compatibility, re-export ExtendedFloat as HighPrecisionFloat
#[cfg(any(
    feature = "high-precision",
    feature = "wasm-precision",
    feature = "native-precision"
))]
pub use precision::ExtendedFloat as HighPrecisionFloat;

#[cfg(feature = "phantom-types")]
pub mod verified;

#[cfg(feature = "formal-verification")]
pub mod verified_contracts;

#[cfg(test)]
pub mod property_tests;

#[cfg(test)]
pub mod comprehensive_tests;

pub use cayley::CayleyTable;

/// A multivector in a Clifford algebra Cl(P,Q,R)
///
/// # Type Parameters
/// - `P`: Number of basis vectors that square to +1
/// - `Q`: Number of basis vectors that square to -1  
/// - `R`: Number of basis vectors that square to 0
#[derive(Debug, Clone, PartialEq)]
#[repr(C, align(32))] // AVX2-optimal alignment for SIMD performance
pub struct Multivector<const P: usize, const Q: usize, const R: usize> {
    /// Coefficients for each basis blade, indexed by binary representation
    /// Memory layout optimized for cache lines and SIMD operations
    coefficients: Box<[f64]>,
}

impl<const P: usize, const Q: usize, const R: usize> Multivector<P, Q, R> {
    /// Total dimension of the algebra's vector space
    pub const DIM: usize = P + Q + R;

    /// Total number of basis blades (2^DIM)
    pub const BASIS_COUNT: usize = 1 << Self::DIM;

    /// Create a new zero multivector
    #[inline(always)]
    pub fn zero() -> Self {
        Self {
            coefficients: vec![0.0; Self::BASIS_COUNT].into_boxed_slice(),
        }
    }

    /// Create a scalar multivector
    #[inline(always)]
    pub fn scalar(value: f64) -> Self {
        let mut mv = Self::zero();
        mv.coefficients[0] = value;
        mv
    }

    /// Create multivector from vector
    pub fn from_vector(vector: &Vector<P, Q, R>) -> Self {
        vector.mv.clone()
    }

    /// Create multivector from bivector
    pub fn from_bivector(bivector: &Bivector<P, Q, R>) -> Self {
        bivector.mv.clone()
    }

    /// Create a basis vector e_i (i starts from 0)
    #[inline(always)]
    pub fn basis_vector(i: usize) -> Self {
        assert!(i < Self::DIM, "Basis vector index out of range");
        let mut mv = Self::zero();
        mv.coefficients[1 << i] = 1.0;
        mv
    }

    /// Create a multivector from coefficients
    #[inline(always)]
    pub fn from_coefficients(coefficients: Vec<f64>) -> Self {
        assert_eq!(
            coefficients.len(),
            Self::BASIS_COUNT,
            "Coefficient array must have {} elements",
            Self::BASIS_COUNT
        );
        Self {
            coefficients: coefficients.into_boxed_slice(),
        }
    }

    /// Create a multivector from a slice (convenience for tests)
    #[inline(always)]
    pub fn from_slice(coefficients: &[f64]) -> Self {
        Self::from_coefficients(coefficients.to_vec())
    }

    /// Get coefficient for a specific basis blade (by index)
    #[inline(always)]
    pub fn get(&self, index: usize) -> f64 {
        self.coefficients.get(index).copied().unwrap_or(0.0)
    }

    /// Set coefficient for a specific basis blade
    #[inline(always)]
    pub fn set(&mut self, index: usize, value: f64) {
        if index < self.coefficients.len() {
            self.coefficients[index] = value;
        }
    }

    /// Get the scalar part (grade 0)
    #[inline(always)]
    pub fn scalar_part(&self) -> f64 {
        self.coefficients[0]
    }

    /// Set the scalar part
    #[inline(always)]
    pub fn set_scalar(&mut self, value: f64) {
        self.coefficients[0] = value;
    }

    /// Get vector part as a Vector type
    pub fn vector_part(&self) -> Vector<P, Q, R> {
        Vector::from_multivector(&self.grade_projection(1))
    }

    /// Get bivector part as a Multivector (grade 2 projection)
    pub fn bivector_part(&self) -> Self {
        self.grade_projection(2)
    }

    /// Get bivector part as a Bivector type wrapper
    pub fn bivector_type(&self) -> Bivector<P, Q, R> {
        Bivector::from_multivector(&self.grade_projection(2))
    }

    /// Get trivector part (scalar for 3D)
    pub fn trivector_part(&self) -> f64 {
        if Self::DIM >= 3 {
            self.get(7) // e123 for 3D
        } else {
            0.0
        }
    }

    /// Set vector component
    pub fn set_vector_component(&mut self, index: usize, value: f64) {
        if index < Self::DIM {
            self.coefficients[1 << index] = value;
        }
    }

    /// Set bivector component
    pub fn set_bivector_component(&mut self, index: usize, value: f64) {
        // Map index to bivector blade indices
        let bivector_indices = match Self::DIM {
            3 => [3, 5, 6], // e12, e13, e23
            _ => [3, 5, 6], // Default mapping
        };
        if index < bivector_indices.len() {
            self.coefficients[bivector_indices[index]] = value;
        }
    }

    /// Get vector component
    pub fn vector_component(&self, index: usize) -> f64 {
        if index < Self::DIM {
            self.get(1 << index)
        } else {
            0.0
        }
    }

    /// Get coefficients as slice for comparison
    pub fn as_slice(&self) -> &[f64] {
        &self.coefficients
    }

    /// Convert coefficients to owned vector
    ///
    /// Useful for interoperability with other libraries and serialization.
    pub fn to_vec(&self) -> Vec<f64> {
        self.coefficients.to_vec()
    }

    /// Grade projection with alternative name for clarity
    ///
    /// Alias for `grade_projection` that emphasizes the projection operation.
    pub fn grade_project(&self, grade: usize) -> Self {
        self.grade_projection(grade)
    }

    /// Wedge product (alias for outer product)
    ///
    /// The wedge product ∧ is the antisymmetric outer product,
    /// fundamental to exterior algebra and differential forms.
    pub fn wedge(&self, other: &Self) -> Self {
        self.outer_product(other)
    }

    /// Dot product (alias for inner product)
    ///
    /// The dot product · is the grade-lowering inner product,
    /// generalizing the vector dot product to all grades.
    pub fn dot(&self, other: &Self) -> Self {
        self.inner_product(other)
    }

    /// Add method for convenience
    pub fn add(&self, other: &Self) -> Self {
        self + other
    }

    /// Get the grade of a multivector (returns the highest non-zero grade)
    pub fn grade(&self) -> usize {
        for grade in (0..=Self::DIM).rev() {
            let projection = self.grade_projection(grade);
            if !projection.is_zero() {
                return grade;
            }
        }
        0 // Zero multivector has grade 0
    }

    /// Outer product with a vector (convenience method)
    pub fn outer_product_with_vector(&self, other: &Vector<P, Q, R>) -> Self {
        self.outer_product(&other.mv)
    }

    /// Geometric product with another multivector
    ///
    /// The geometric product is the fundamental operation in geometric algebra,
    /// combining both the inner and outer products.
    pub fn geometric_product(&self, rhs: &Self) -> Self {
        self.geometric_product_scalar(rhs)
    }

    /// Scalar implementation of geometric product (fallback)
    pub(crate) fn geometric_product_scalar(&self, rhs: &Self) -> Self {
        let table = CayleyTable::<P, Q, R>::get();
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            if self.coefficients[i].abs() < 1e-14 {
                continue;
            }

            for j in 0..Self::BASIS_COUNT {
                if rhs.coefficients[j].abs() < 1e-14 {
                    continue;
                }

                let (sign, index) = table.get_product(i, j);
                result.coefficients[index] += sign * self.coefficients[i] * rhs.coefficients[j];
            }
        }

        result
    }

    /// Inner product (grade-lowering, dot product for vectors)
    pub fn inner_product(&self, rhs: &Self) -> Self {
        let self_grades = self.grade_decomposition();
        let rhs_grades = rhs.grade_decomposition();
        let mut result = Self::zero();

        // Inner product selects terms where grade(result) = |grade(a) - grade(b)|
        for (grade_a, mv_a) in self_grades.iter().enumerate() {
            for (grade_b, mv_b) in rhs_grades.iter().enumerate() {
                if !mv_a.is_zero() && !mv_b.is_zero() {
                    let target_grade = grade_a.abs_diff(grade_b);
                    let product = mv_a.geometric_product(mv_b);
                    let projected = product.grade_projection(target_grade);
                    result = result + projected;
                }
            }
        }

        result
    }

    /// Outer product (wedge product, grade-raising)
    pub fn outer_product(&self, rhs: &Self) -> Self {
        let self_grades = self.grade_decomposition();
        let rhs_grades = rhs.grade_decomposition();
        let mut result = Self::zero();

        // Outer product selects terms where grade(result) = grade(a) + grade(b)
        for (grade_a, mv_a) in self_grades.iter().enumerate() {
            for (grade_b, mv_b) in rhs_grades.iter().enumerate() {
                if !mv_a.is_zero() && !mv_b.is_zero() {
                    let target_grade = grade_a + grade_b;
                    if target_grade <= Self::DIM {
                        let product = mv_a.geometric_product(mv_b);
                        let projected = product.grade_projection(target_grade);
                        result = result + projected;
                    }
                }
            }
        }

        result
    }

    /// Scalar product (grade-0 part of geometric product)
    pub fn scalar_product(&self, rhs: &Self) -> f64 {
        self.geometric_product(rhs).scalar_part()
    }

    /// Calculate the sign change for reversing a blade of given grade
    ///
    /// The reverse operation introduces a sign change of (-1)^(grade*(grade-1)/2)
    /// This comes from the fact that reversing a k-blade requires k(k-1)/2 swaps
    /// of basis vectors, and each swap introduces a sign change.
    #[inline]
    fn reverse_sign_for_grade(grade: usize) -> f64 {
        if grade == 0 {
            return 1.0;
        }
        if (grade * (grade - 1) / 2).is_multiple_of(2) {
            1.0
        } else {
            -1.0
        }
    }

    /// Reverse operation (reverses order of basis vectors in each blade)
    pub fn reverse(&self) -> Self {
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            let grade = i.count_ones() as usize;
            let sign = Self::reverse_sign_for_grade(grade);
            result.coefficients[i] = sign * self.coefficients[i];
        }

        result
    }

    /// Grade projection - extract components of a specific grade
    pub fn grade_projection(&self, grade: usize) -> Self {
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            if i.count_ones() as usize == grade {
                result.coefficients[i] = self.coefficients[i];
            }
        }

        result
    }

    /// Compute the magnitude of a specific grade component.
    ///
    /// This extracts the k-vector part and computes its magnitude.
    /// Useful for determining grade dominance in holographic representations.
    ///
    /// # Arguments
    /// * `grade` - The grade to compute magnitude for (0=scalar, 1=vector, 2=bivector, etc.)
    ///
    /// # Returns
    /// The magnitude of the grade-k component, or 0.0 if no such component exists.
    ///
    /// # Example
    /// ```rust
    /// use amari_core::Multivector;
    /// let mv = Multivector::<3,0,0>::basis_vector(0) + Multivector::<3,0,0>::scalar(2.0);
    /// assert!((mv.grade_magnitude(0) - 2.0).abs() < 1e-10);  // scalar part
    /// assert!((mv.grade_magnitude(1) - 1.0).abs() < 1e-10);  // vector part
    /// ```
    pub fn grade_magnitude(&self, grade: usize) -> f64 {
        self.grade_projection(grade).magnitude()
    }

    /// Get all grade magnitudes as a vector.
    ///
    /// Returns the magnitude of each grade component from 0 to DIM.
    /// This provides a "grade spectrum" view of the multivector.
    pub fn grade_spectrum(&self) -> Vec<f64> {
        (0..=Self::DIM).map(|g| self.grade_magnitude(g)).collect()
    }

    /// Decompose into grade components
    fn grade_decomposition(&self) -> Vec<Self> {
        let mut grades = Vec::with_capacity(Self::DIM + 1);
        for _ in 0..=Self::DIM {
            grades.push(Self::zero());
        }

        for i in 0..Self::BASIS_COUNT {
            let grade = i.count_ones() as usize;
            grades[grade].coefficients[i] = self.coefficients[i];
        }

        grades
    }

    /// Check if this is (approximately) zero
    pub fn is_zero(&self) -> bool {
        self.coefficients.iter().all(|&c| c.abs() < 1e-14)
    }

    /// Compute the norm squared (scalar product with reverse)
    pub fn norm_squared(&self) -> f64 {
        self.scalar_product(&self.reverse())
    }

    /// Compute the magnitude (length) of this multivector
    ///
    /// The magnitude is defined as |a| = √(a·ã) where ã is the reverse of a.
    /// This provides the natural norm inherited from the underlying vector space.
    ///
    /// # Mathematical Properties
    /// - Always non-negative: |a| ≥ 0
    /// - Zero iff a = 0: |a| = 0 ⟺ a = 0
    /// - Sub-multiplicative: |ab| ≤ |a||b|
    ///
    /// # Examples
    /// ```rust
    /// use amari_core::Multivector;
    /// let v = Multivector::<3,0,0>::basis_vector(0);
    /// assert_eq!(v.magnitude(), 1.0);
    /// ```
    pub fn magnitude(&self) -> f64 {
        self.norm_squared().abs().sqrt()
    }

    /// Compute the norm (magnitude) of this multivector
    ///
    /// **Note**: This method is maintained for backward compatibility.
    /// New code should prefer [`magnitude()`](Self::magnitude) for clarity.
    pub fn norm(&self) -> f64 {
        self.magnitude()
    }

    /// Absolute value (same as magnitude/norm for multivectors)
    pub fn abs(&self) -> f64 {
        self.magnitude()
    }

    /// Approximate equality comparison
    pub fn approx_eq(&self, other: &Self, epsilon: f64) -> bool {
        (self.clone() - other.clone()).magnitude() < epsilon
    }

    /// Normalize this multivector
    pub fn normalize(&self) -> Option<Self> {
        let norm = self.norm();
        if norm > 1e-14 {
            Some(self * (1.0 / norm))
        } else {
            None
        }
    }

    /// Compute multiplicative inverse if it exists
    pub fn inverse(&self) -> Option<Self> {
        let rev = self.reverse();
        let norm_sq = self.scalar_product(&rev);

        if norm_sq.abs() > 1e-14 {
            Some(rev * (1.0 / norm_sq))
        } else {
            None
        }
    }

    /// Exponential map for bivectors (creates rotors)
    ///
    /// For a bivector B, exp(B) creates a rotor that performs rotation
    /// in the plane defined by B.
    pub fn exp(&self) -> Self {
        // Check if this is a bivector (grade 2)
        let grade2 = self.grade_projection(2);
        if (self - &grade2).norm() > 1e-10 {
            // For general multivectors, use series expansion
            return self.exp_series();
        }

        // For pure bivectors, use closed form
        let b_squared = grade2.geometric_product(&grade2).scalar_part();

        if b_squared > -1e-14 {
            // Hyperbolic case: exp(B) = cosh(|B|) + sinh(|B|) * B/|B|
            let norm = b_squared.abs().sqrt();
            if norm < 1e-14 {
                return Self::scalar(1.0);
            }
            let cosh_norm = norm.cosh();
            let sinh_norm = norm.sinh();
            Self::scalar(cosh_norm) + grade2 * (sinh_norm / norm)
        } else {
            // Circular case: exp(B) = cos(|B|) + sin(|B|) * B/|B|
            let norm = (-b_squared).sqrt();
            let cos_norm = norm.cos();
            let sin_norm = norm.sin();
            Self::scalar(cos_norm) + grade2 * (sin_norm / norm)
        }
    }

    /// Series expansion for exponential (fallback for general multivectors)
    fn exp_series(&self) -> Self {
        let mut result = Self::scalar(1.0);
        let mut term = Self::scalar(1.0);

        for n in 1..20 {
            term = term.geometric_product(self) * (1.0 / n as f64);
            let old_result = result.clone();
            result = result + term.clone();

            // Check convergence
            if (result.clone() - old_result).norm() < 1e-14 {
                break;
            }
        }

        result
    }

    /// Left contraction: a ⌟ b
    ///
    /// Generalized inner product where the grade of the result
    /// is |grade(b) - grade(a)|. The left operand's grade is reduced.
    pub fn left_contraction(&self, other: &Self) -> Self {
        let self_grades = self.grade_decomposition();
        let other_grades = other.grade_decomposition();
        let mut result = Self::zero();

        for (a_grade, mv_a) in self_grades.iter().enumerate() {
            if mv_a.is_zero() {
                continue;
            }

            for (b_grade, mv_b) in other_grades.iter().enumerate() {
                if mv_b.is_zero() {
                    continue;
                }

                // Left contraction: grade of result is |b_grade - a_grade|
                if b_grade >= a_grade {
                    let target_grade = b_grade - a_grade;
                    let product = mv_a.geometric_product(mv_b);
                    let projected = product.grade_projection(target_grade);
                    result = result + projected;
                }
            }
        }

        result
    }

    /// Right contraction: a ⌞ b  
    ///
    /// Generalized inner product where the grade of the result
    /// is |grade(a) - grade(b)|. The right operand's grade is reduced.
    pub fn right_contraction(&self, other: &Self) -> Self {
        let self_grades = self.grade_decomposition();
        let other_grades = other.grade_decomposition();
        let mut result = Self::zero();

        for (a_grade, mv_a) in self_grades.iter().enumerate() {
            if mv_a.is_zero() {
                continue;
            }

            for (b_grade, mv_b) in other_grades.iter().enumerate() {
                if mv_b.is_zero() {
                    continue;
                }

                // Right contraction: grade of result is |a_grade - b_grade|
                if a_grade >= b_grade {
                    let target_grade = a_grade - b_grade;
                    let product = mv_a.geometric_product(mv_b);
                    let projected = product.grade_projection(target_grade);
                    result = result + projected;
                }
            }
        }

        result
    }

    /// Hodge dual: ⋆a
    ///
    /// Maps k-vectors to (n-k)-vectors in n-dimensional space.
    /// Defined such that `e_A ∧ ⋆e_A = |e_A|² · I` where I is the pseudoscalar.
    ///
    /// In 3D Euclidean (Cl(3,0,0)): ⋆e1 = e23, ⋆e2 = -e13, ⋆e3 = e12
    ///
    /// For degenerate algebras (R > 0), the pseudoscalar squares to zero
    /// and the Hodge dual is not well-defined. In this case, the function
    /// computes a formal complement using only the permutation sign.
    pub fn hodge_dual(&self) -> Self {
        let n = Self::DIM;
        if n == 0 {
            return self.clone();
        }

        let mut result = Self::zero();
        let pseudoscalar_index = (1 << n) - 1; // All bits set

        for i in 0..Self::BASIS_COUNT {
            if self.coefficients[i].abs() < 1e-14 {
                continue;
            }

            let dual_index = i ^ pseudoscalar_index;

            // Compute permutation sign: e_A ∧ e_complement = sign * I
            // Count swaps needed to interleave bits of i with bits of dual_index
            // into canonical order (ascending bit positions)
            let mut swaps = 0u32;
            for bit_pos in 0..n {
                if (i >> bit_pos) & 1 == 1 {
                    // Count bits in dual_index at positions below bit_pos
                    let below_mask = (1 << bit_pos) - 1;
                    swaps += (dual_index & below_mask).count_ones();
                }
            }
            let permutation_sign: f64 = if swaps.is_multiple_of(2) { 1.0 } else { -1.0 };

            // Apply metric factor for the blade being dualized
            // |e_A|² = product of e_i² for each basis vector in A
            let mut metric_factor = 1.0;
            for j in 0..n {
                if (i >> j) & 1 == 1 {
                    if j >= P + Q {
                        // Null basis vector: |e_A|² = 0
                        metric_factor = 0.0;
                        break;
                    } else if j >= P {
                        // Negative basis vector: contributes -1
                        metric_factor *= -1.0;
                    }
                    // Positive basis vector: contributes +1
                }
            }

            // ⋆e_A = metric(A) * permutation_sign * e_complement
            let sign = metric_factor * permutation_sign;

            result.coefficients[dual_index] += sign * self.coefficients[i];
        }

        result
    }
}

// Operator implementations
impl<const P: usize, const Q: usize, const R: usize> Add for Multivector<P, Q, R> {
    type Output = Self;

    #[inline(always)]
    fn add(mut self, rhs: Self) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] += rhs.coefficients[i];
        }
        self
    }
}

impl<const P: usize, const Q: usize, const R: usize> Add for &Multivector<P, Q, R> {
    type Output = Multivector<P, Q, R>;

    #[inline(always)]
    fn add(self, rhs: Self) -> Multivector<P, Q, R> {
        let mut result = self.clone();
        for i in 0..Multivector::<P, Q, R>::BASIS_COUNT {
            result.coefficients[i] += rhs.coefficients[i];
        }
        result
    }
}

impl<const P: usize, const Q: usize, const R: usize> Sub for Multivector<P, Q, R> {
    type Output = Self;

    #[inline(always)]
    fn sub(mut self, rhs: Self) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] -= rhs.coefficients[i];
        }
        self
    }
}

impl<const P: usize, const Q: usize, const R: usize> Sub for &Multivector<P, Q, R> {
    type Output = Multivector<P, Q, R>;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Multivector<P, Q, R> {
        let mut result = self.clone();
        for i in 0..Multivector::<P, Q, R>::BASIS_COUNT {
            result.coefficients[i] -= rhs.coefficients[i];
        }
        result
    }
}

impl<const P: usize, const Q: usize, const R: usize> Mul<f64> for Multivector<P, Q, R> {
    type Output = Self;

    #[inline(always)]
    fn mul(mut self, scalar: f64) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] *= scalar;
        }
        self
    }
}

impl<const P: usize, const Q: usize, const R: usize> Mul<f64> for &Multivector<P, Q, R> {
    type Output = Multivector<P, Q, R>;

    #[inline(always)]
    fn mul(self, scalar: f64) -> Multivector<P, Q, R> {
        let mut result = self.clone();
        for i in 0..Multivector::<P, Q, R>::BASIS_COUNT {
            result.coefficients[i] *= scalar;
        }
        result
    }
}

impl<const P: usize, const Q: usize, const R: usize> Neg for Multivector<P, Q, R> {
    type Output = Self;

    #[inline(always)]
    fn neg(mut self) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] = -self.coefficients[i];
        }
        self
    }
}

impl<const P: usize, const Q: usize, const R: usize> Zero for Multivector<P, Q, R> {
    fn zero() -> Self {
        Self::zero()
    }

    fn is_zero(&self) -> bool {
        self.is_zero()
    }
}

/// Scalar type - wrapper around Multivector with only grade 0
#[derive(Debug, Clone, PartialEq)]
pub struct Scalar<const P: usize, const Q: usize, const R: usize> {
    pub mv: Multivector<P, Q, R>,
}

impl<const P: usize, const Q: usize, const R: usize> Scalar<P, Q, R> {
    pub fn from(value: f64) -> Self {
        Self {
            mv: Multivector::scalar(value),
        }
    }

    pub fn one() -> Self {
        Self::from(1.0)
    }

    pub fn geometric_product(&self, other: &Multivector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.geometric_product(other)
    }

    pub fn geometric_product_with_vector(&self, other: &Vector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.geometric_product(&other.mv)
    }
}

impl<const P: usize, const Q: usize, const R: usize> From<f64> for Scalar<P, Q, R> {
    fn from(value: f64) -> Self {
        Self::from(value)
    }
}

/// Vector type - wrapper around Multivector with only grade 1
#[derive(Debug, Clone, PartialEq)]
pub struct Vector<const P: usize, const Q: usize, const R: usize> {
    pub mv: Multivector<P, Q, R>,
}

impl<const P: usize, const Q: usize, const R: usize> Vector<P, Q, R> {
    /// Create zero vector
    pub fn zero() -> Self {
        Self {
            mv: Multivector::zero(),
        }
    }

    pub fn from_components(x: f64, y: f64, z: f64) -> Self {
        let mut mv = Multivector::zero();
        if P + Q + R >= 1 {
            mv.set_vector_component(0, x);
        }
        if P + Q + R >= 2 {
            mv.set_vector_component(1, y);
        }
        if P + Q + R >= 3 {
            mv.set_vector_component(2, z);
        }
        Self { mv }
    }

    pub fn e1() -> Self {
        Self {
            mv: Multivector::basis_vector(0),
        }
    }

    pub fn e2() -> Self {
        Self {
            mv: Multivector::basis_vector(1),
        }
    }

    pub fn e3() -> Self {
        Self {
            mv: Multivector::basis_vector(2),
        }
    }

    pub fn from_multivector(mv: &Multivector<P, Q, R>) -> Self {
        Self {
            mv: mv.grade_projection(1),
        }
    }

    pub fn geometric_product(&self, other: &Self) -> Multivector<P, Q, R> {
        self.mv.geometric_product(&other.mv)
    }

    pub fn geometric_product_with_multivector(
        &self,
        other: &Multivector<P, Q, R>,
    ) -> Multivector<P, Q, R> {
        self.mv.geometric_product(other)
    }

    pub fn geometric_product_with_bivector(
        &self,
        other: &Bivector<P, Q, R>,
    ) -> Multivector<P, Q, R> {
        self.mv.geometric_product(&other.mv)
    }

    pub fn geometric_product_with_scalar(&self, other: &Scalar<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.geometric_product(&other.mv)
    }

    pub fn add(&self, other: &Self) -> Self {
        Self {
            mv: &self.mv + &other.mv,
        }
    }

    pub fn magnitude(&self) -> f64 {
        self.mv.magnitude()
    }

    pub fn as_slice(&self) -> &[f64] {
        self.mv.as_slice()
    }

    /// Inner product with another vector
    pub fn inner_product(&self, other: &Self) -> Multivector<P, Q, R> {
        self.mv.inner_product(&other.mv)
    }

    /// Inner product with a multivector
    pub fn inner_product_with_mv(&self, other: &Multivector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.inner_product(other)
    }

    /// Inner product with a bivector
    pub fn inner_product_with_bivector(&self, other: &Bivector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.inner_product(&other.mv)
    }

    /// Outer product with another vector
    pub fn outer_product(&self, other: &Self) -> Multivector<P, Q, R> {
        self.mv.outer_product(&other.mv)
    }

    /// Outer product with a multivector
    pub fn outer_product_with_mv(&self, other: &Multivector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.outer_product(other)
    }

    /// Outer product with a bivector
    pub fn outer_product_with_bivector(&self, other: &Bivector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.outer_product(&other.mv)
    }

    /// Left contraction with bivector
    pub fn left_contraction(&self, other: &Bivector<P, Q, R>) -> Multivector<P, Q, R> {
        // Left contraction: a ⌊ b = grade_projection(a * b, |grade(b) - grade(a)|)
        let product = self.mv.geometric_product(&other.mv);
        let target_grade = if 2 >= 1 { 2 - 1 } else { 1 - 2 };
        product.grade_projection(target_grade)
    }

    /// Normalize the vector (return unit vector if possible)
    pub fn normalize(&self) -> Option<Self> {
        self.mv
            .normalize()
            .map(|normalized| Self { mv: normalized })
    }

    /// Compute the squared norm of the vector
    pub fn norm_squared(&self) -> f64 {
        self.mv.norm_squared()
    }

    /// Compute the reverse (for vectors, this is the same as the original)
    pub fn reverse(&self) -> Self {
        Self {
            mv: self.mv.reverse(),
        }
    }

    /// Compute the norm (magnitude) of the vector
    ///
    /// **Note**: This method is maintained for backward compatibility.
    /// New code should prefer [`magnitude()`](Self::magnitude) for clarity.
    pub fn norm(&self) -> f64 {
        self.magnitude()
    }

    /// Hodge dual of the vector
    /// Maps vectors to bivectors in 3D space
    pub fn hodge_dual(&self) -> Bivector<P, Q, R> {
        Bivector {
            mv: self.mv.hodge_dual(),
        }
    }
}

/// Bivector type - wrapper around Multivector with only grade 2
#[derive(Debug, Clone, PartialEq)]
pub struct Bivector<const P: usize, const Q: usize, const R: usize> {
    pub mv: Multivector<P, Q, R>,
}

impl<const P: usize, const Q: usize, const R: usize> Bivector<P, Q, R> {
    pub fn from_components(xy: f64, xz: f64, yz: f64) -> Self {
        let mut mv = Multivector::zero();
        if P + Q + R >= 2 {
            mv.set_bivector_component(0, xy);
        } // e12
        if P + Q + R >= 3 {
            mv.set_bivector_component(1, xz);
        } // e13
        if P + Q + R >= 3 {
            mv.set_bivector_component(2, yz);
        } // e23
        Self { mv }
    }

    pub fn e12() -> Self {
        let mut mv = Multivector::zero();
        mv.set_bivector_component(0, 1.0); // e12
        Self { mv }
    }

    pub fn e13() -> Self {
        let mut mv = Multivector::zero();
        mv.set_bivector_component(1, 1.0); // e13
        Self { mv }
    }

    pub fn e23() -> Self {
        let mut mv = Multivector::zero();
        mv.set_bivector_component(2, 1.0); // e23
        Self { mv }
    }

    pub fn from_multivector(mv: &Multivector<P, Q, R>) -> Self {
        Self {
            mv: mv.grade_projection(2),
        }
    }

    pub fn geometric_product(&self, other: &Vector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.geometric_product(&other.mv)
    }

    /// Geometric product with another bivector
    pub fn geometric_product_with_bivector(&self, other: &Self) -> Multivector<P, Q, R> {
        self.mv.geometric_product(&other.mv)
    }

    pub fn magnitude(&self) -> f64 {
        self.mv.magnitude()
    }

    /// Index access for bivector components
    pub fn get(&self, index: usize) -> f64 {
        match index {
            0 => self.mv.get(3), // e12
            1 => self.mv.get(5), // e13
            2 => self.mv.get(6), // e23
            _ => 0.0,
        }
    }

    /// Inner product with another bivector
    pub fn inner_product(&self, other: &Self) -> Multivector<P, Q, R> {
        self.mv.inner_product(&other.mv)
    }

    /// Inner product with a vector
    pub fn inner_product_with_vector(&self, other: &Vector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.inner_product(&other.mv)
    }

    /// Outer product with another bivector
    pub fn outer_product(&self, other: &Self) -> Multivector<P, Q, R> {
        self.mv.outer_product(&other.mv)
    }

    /// Outer product with a vector
    pub fn outer_product_with_vector(&self, other: &Vector<P, Q, R>) -> Multivector<P, Q, R> {
        self.mv.outer_product(&other.mv)
    }

    /// Right contraction with vector
    pub fn right_contraction(&self, other: &Vector<P, Q, R>) -> Multivector<P, Q, R> {
        // Right contraction: a ⌋ b = grade_projection(a * b, |grade(a) - grade(b)|)
        let product = self.mv.geometric_product(&other.mv);
        let target_grade = if 2 >= 1 { 2 - 1 } else { 1 - 2 };
        product.grade_projection(target_grade)
    }
}

impl<const P: usize, const Q: usize, const R: usize> core::ops::Index<usize> for Bivector<P, Q, R> {
    type Output = f64;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.mv.coefficients[3], // e12
            1 => &self.mv.coefficients[5], // e13
            2 => &self.mv.coefficients[6], // e23
            _ => panic!("Bivector index out of range: {}", index),
        }
    }
}

/// Convenience type alias for basis vector E
pub type E<const P: usize, const Q: usize, const R: usize> = Vector<P, Q, R>;

// Re-export the Rotor type from the rotor module
pub use rotor::Rotor;

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    type Cl3 = Multivector<3, 0, 0>; // 3D Euclidean space

    #[test]
    fn test_basis_vectors() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);

        // e1 * e1 = 1
        let e1_squared = e1.geometric_product(&e1);
        assert_eq!(e1_squared.scalar_part(), 1.0);

        // e1 * e2 = -e2 * e1 (anticommute)
        let e12 = e1.geometric_product(&e2);
        let e21 = e2.geometric_product(&e1);
        assert_eq!(e12.coefficients[3], -e21.coefficients[3]); // e1∧e2 component
    }

    #[test]
    fn test_wedge_product() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);

        let e12 = e1.outer_product(&e2);
        assert!(e12.get(3).abs() - 1.0 < 1e-10); // e1∧e2 has index 0b11 = 3

        // e1 ∧ e1 = 0
        let e11 = e1.outer_product(&e1);
        assert!(e11.is_zero());
    }

    #[test]
    fn test_rotor_from_bivector() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e12 = e1.outer_product(&e2);

        // Create 90 degree rotation in e1-e2 plane
        let angle = core::f64::consts::PI / 4.0; // Half angle for rotor
        let bivector = e12 * angle;
        let rotor = bivector.exp();

        // Check that rotor has unit norm
        assert!((rotor.norm() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_algebraic_identities() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e3 = Cl3::basis_vector(2);

        // Associativity: (ab)c = a(bc)
        let a = e1.clone() + e2.clone() * 2.0;
        let b = e2.clone() + e3.clone() * 3.0;
        let c = e3.clone() + e1.clone() * 0.5;

        let left = a.geometric_product(&b).geometric_product(&c);
        let right = a.geometric_product(&b.geometric_product(&c));
        assert_relative_eq!(left.norm(), right.norm(), epsilon = 1e-12);

        // Distributivity: a(b + c) = ab + ac
        let ab_plus_ac = a.geometric_product(&b) + a.geometric_product(&c);
        let a_times_b_plus_c = a.geometric_product(&(b.clone() + c.clone()));
        assert!((ab_plus_ac - a_times_b_plus_c).norm() < 1e-12);

        // Reverse property: (ab)† = b†a†
        let ab_reverse = a.geometric_product(&b).reverse();
        let b_reverse_a_reverse = b.reverse().geometric_product(&a.reverse());
        assert!((ab_reverse - b_reverse_a_reverse).norm() < 1e-12);
    }

    #[test]
    fn test_metric_signature() {
        // Test different signatures
        type Spacetime = Multivector<1, 3, 0>; // Minkowski signature

        let e0 = Spacetime::basis_vector(0); // timelike
        let e1 = Spacetime::basis_vector(1); // spacelike

        // e0^2 = +1, e1^2 = -1
        assert_eq!(e0.geometric_product(&e0).scalar_part(), 1.0);
        assert_eq!(e1.geometric_product(&e1).scalar_part(), -1.0);
    }

    #[test]
    fn test_grade_operations() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let scalar = Cl3::scalar(2.0);

        let mv = scalar + e1.clone() * 3.0 + e2.clone() * 4.0 + e1.outer_product(&e2) * 5.0;

        // Test grade projections
        let grade0 = mv.grade_projection(0);
        let grade1 = mv.grade_projection(1);
        let grade2 = mv.grade_projection(2);

        assert_eq!(grade0.scalar_part(), 2.0);
        assert_eq!(grade1.get(1), 3.0); // e1 component
        assert_eq!(grade1.get(2), 4.0); // e2 component
        assert_eq!(grade2.get(3), 5.0); // e12 component

        // Sum of grade projections should equal original
        let reconstructed = grade0 + grade1 + grade2;
        assert!((mv - reconstructed).norm() < 1e-12);
    }

    #[test]
    fn test_inner_and_outer_products() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e3 = Cl3::basis_vector(2);

        // Inner product of orthogonal vectors is zero
        assert!(e1.inner_product(&e2).norm() < 1e-12);

        // Inner product of parallel vectors
        let v1 = e1.clone() + e2.clone();
        let v2 = e1.clone() * 2.0 + e2.clone() * 2.0;
        let inner = v1.inner_product(&v2);
        assert_relative_eq!(inner.scalar_part(), 4.0, epsilon = 1e-12);

        // Outer product creates higher grade
        let bivector = e1.outer_product(&e2);
        let trivector = bivector.outer_product(&e3);
        assert_eq!(trivector.get(7), 1.0); // e123 component
    }
}
