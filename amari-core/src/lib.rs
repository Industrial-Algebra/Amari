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
use core::ops::{Add, Div, Mul, Neg, Sub};
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

/// Exact dyadic bivector square (CORE-W03 stage 1).
///
/// Consumed by the stage-2 `exp()` routing (`crate::Multivector::exp`).
mod exact_square;
pub(crate) use exact_square::exact_x2;

#[cfg(feature = "gf2")]
pub mod gf2;

#[allow(dead_code)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod simd;

// Re-export error types
pub use error::{CoreError, CoreResult, ExpError, NonFiniteStage};

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

/// Overflow-safe squaring cancellation test for the series route.
///
/// The round-1 form `new_norm < previous_norm * previous_norm * 2^-62` forms
/// `previous_norm²`, which overflows to `+inf` for `previous_norm > 2^512`
/// and then rejects every squaring at that scale. This ratio form computes the
/// identical predicate with both factors representable: for `previous_norm > 0`,
/// `new_norm < previous_norm² · 2^-62` iff
/// `new_norm / previous_norm < previous_norm · 2^-62`. The threshold
/// `previous_norm · 2^-62` is finite for every finite `previous_norm`, and the
/// ratio is `0` when `new_norm` underflows, which is still a genuine collapse.
fn squaring_is_collapse(previous_norm: f64, new_norm: f64) -> bool {
    previous_norm > 1073741824.0 // 2^30
        && new_norm / previous_norm < previous_norm * 2f64.powi(-62)
}

/// Symbolic decomposition of the scalar exponential: `exp(s) = m · 2^E`
/// with `m a rounded f64 and integer `E`, the base-2 exponent of the result.
///
/// This is the round-3 P1-1 fix for scalar factoring. Computing `f64::exp(s)`
/// first loses the answer when `exp(s)` underflows to `0` (large negative `s`)
/// or overflows to `+inf` (large positive `s`), even though multiplying by a
/// non-scalar coefficient can bring the product back into range. Carrying the
/// exponent separately keeps that information: the mantissa is the
/// accurately-reduced `exp(s - E·ln2)` (one `f64::exp`, ~1 ulp), and the
/// integer `E` is applied per coefficient with a single
/// round-to-nearest-even exponent step.
///
/// The reduction uses the split `ln2 = LN2_HI + LN2_LO` (fdlibm constants).
/// `LN2_HI` has enough trailing zero bits that `E · LN2_HI` is exact for
/// `|E| < 2^21`, and `s - E · LN2_HI` is exact by Sterbenz, so the reduced
/// argument loses only the `E · LN2_LO` rounding (~2^-70·E). A naive
/// `2^(s·log2 e)` decomposition with a single rounded `LOG2_E` instead carries
/// the constant's ~1e-16 relative error into the mantissa (~7 ulp here), which
/// is not acceptable.
///
/// For `|s|` beyond the reduced range (`|s/ln2| > 2^20`) the true exponential
/// is beyond any rescuing by a finite `f64` coefficient, so a saturated
/// exponent is returned and the per-coefficient scaling resolves it to `0` or
/// `+inf` (the honest representability outcome).
fn symbolic_exp_scale(s: f64) -> (f64, i64) {
    const LOG2_E: f64 = core::f64::consts::LOG2_E;
    const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
    const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);

    let estimate = s * LOG2_E;
    if !estimate.is_finite() || estimate.abs() > 1_048_576.0 {
        return if s > 0.0 {
            (1.0, i64::MAX / 2)
        } else {
            (1.0, i64::MIN / 2)
        };
    }
    // Nearest integer to `s / ln2`, and the accurately reduced argument
    // `r = s - E·ln2` (`|r| <= ln2/2` to within the estimate's rounding).
    let exponent = estimate.round();
    let reduced = (s - exponent * LN2_HI) - exponent * LN2_LO;
    (reduced.exp(), exponent as i64)
}

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

    /// Whether every stored coefficient is exactly zero.
    /// Internal structural predicate, not a tolerance-based numerical query.
    fn is_exact_zero(&self) -> bool {
        self.coefficients
            .iter()
            .all(|&coefficient| coefficient == 0.0)
    }

    /// Get the grade of a multivector (returns the highest non-zero grade)
    ///
    /// A grade counts as non-zero when any of its coefficients differs from
    /// exact zero; no tolerance is applied.
    pub fn grade(&self) -> usize {
        for grade in (0..=Self::DIM).rev() {
            let projection = self.grade_projection(grade);
            if !projection.is_exact_zero() {
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
    ///
    /// combining both the inner and outer products.
    pub fn geometric_product(&self, rhs: &Self) -> Self {
        self.geometric_product_scalar(rhs)
    }

    /// Scalar implementation of geometric product (fallback)
    pub(crate) fn geometric_product_scalar(&self, rhs: &Self) -> Self {
        let table = CayleyTable::<P, Q, R>::get();
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            if self.coefficients[i] == 0.0 {
                continue;
            }

            for j in 0..Self::BASIS_COUNT {
                if rhs.coefficients[j] == 0.0 {
                    continue;
                }

                let (sign, index) = table.get_product(i, j);
                result.coefficients[index] += sign * self.coefficients[i] * rhs.coefficients[j];
            }
        }

        result
    }

    /// Inner product (grade-lowering, dot product for vectors)
    ///
    /// Coefficients equal to exact zero are skipped; there is no implicit
    /// approximate-zero pruning. Ordinary floating-point rounding still
    /// applies to the accumulated values.
    pub fn inner_product(&self, rhs: &Self) -> Self {
        let self_grades = self.grade_decomposition();
        let rhs_grades = rhs.grade_decomposition();
        let mut result = Self::zero();

        // Inner product selects terms where grade(result) = |grade(a) - grade(b)|
        for (grade_a, mv_a) in self_grades.iter().enumerate() {
            for (grade_b, mv_b) in rhs_grades.iter().enumerate() {
                if !mv_a.is_exact_zero() && !mv_b.is_exact_zero() {
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
    ///
    /// Coefficients equal to exact zero are skipped; there is no implicit
    /// approximate-zero pruning.
    pub fn outer_product(&self, rhs: &Self) -> Self {
        let self_grades = self.grade_decomposition();
        let rhs_grades = rhs.grade_decomposition();
        let mut result = Self::zero();

        // Outer product selects terms where grade(result) = grade(a) + grade(b)
        for (grade_a, mv_a) in self_grades.iter().enumerate() {
            for (grade_b, mv_b) in rhs_grades.iter().enumerate() {
                if !mv_a.is_exact_zero() && !mv_b.is_exact_zero() {
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
        self.grade_projection(grade).coefficient_norm()
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

    /// Whether every coefficient is exactly zero.
    ///
    /// Exact structural test; there is no tolerance. For a tolerance-based
    /// query use [`is_approx_zero`](Self::is_approx_zero).
    pub fn is_zero(&self) -> bool {
        self.coefficients.iter().all(|&c| c == 0.0)
    }

    /// Whether every coefficient is zero within `epsilon` under the same
    /// coefficientwise relative+absolute rule as
    /// [`approx_eq`](Self::approx_eq) against the zero element.
    pub fn is_approx_zero(&self, epsilon: f64) -> bool {
        self.approx_eq(&Self::zero(), epsilon)
    }

    /// Signed metric quadratic form: the scalar product with the reverse.
    ///
    /// This is the honest quadratic form induced by the algebra's metric.
    /// In mixed or degenerate signatures it may be negative or zero for
    /// nonzero elements; it is not a magnitude. Use
    /// [`coefficient_norm`](Self::coefficient_norm) for the non-negative
    /// magnitude of the coefficient vector.
    pub fn norm_squared(&self) -> f64 {
        self.scalar_product(&self.reverse())
    }

    /// Coefficient magnitude: sqrt of the sum of squared coefficients.
    ///
    /// Signature-independent l2 norm of the coefficient vector; always >= 0
    /// and zero only for the exact zero element. This is not the metric
    /// quantity — use [`norm_squared`](Self::norm_squared) for the signed
    /// quadratic form.
    pub fn coefficient_norm(&self) -> f64 {
        // Scaled sum-of-squares: squaring raw coefficients would underflow
        // to 0 for magnitudes below ~1e-154 and overflow to inf above
        // ~1e154; scaling by the largest magnitude keeps every
        // representable magnitude representable. NaN propagates
        // explicitly (f64::max would silently ignore it).
        if self.coefficients.iter().any(|c| c.is_nan()) {
            return f64::NAN;
        }
        let max = self
            .coefficients
            .iter()
            .map(|c| c.abs())
            .fold(0.0f64, f64::max);
        if max == 0.0 || max.is_infinite() {
            return max;
        }
        let scaled = self
            .coefficients
            .iter()
            .map(|&c| {
                let r = c / max;
                r * r
            })
            .sum::<f64>();
        max * scaled.sqrt()
    }

    /// Approximate equality: coefficientwise relative+absolute comparison.
    ///
    /// Two multivectors are approximately equal when, for every coefficient
    /// pair (a, b): a == b, or both are finite and
    /// |a - b| <= epsilon * max(1.0, |a|, |b|).
    /// Non-finite coefficients compare approximately-equal only when exactly
    /// equal (so NaN never matches, +inf matches +inf).
    pub fn approx_eq(&self, other: &Self, epsilon: f64) -> bool {
        self.coefficients
            .iter()
            .zip(other.coefficients.iter())
            .all(|(&a, &b)| {
                a == b
                    || (a.is_finite()
                        && b.is_finite()
                        && (a - b).abs() <= epsilon * a.abs().max(b.abs()).max(1.0))
            })
    }

    /// Crate-private versor normalization: exact positive-domain scaling.
    ///
    /// Returns Some(self * 1/sqrt(norm_squared())) only when the signed
    /// metric quadratic form is strictly positive; no epsilon, no absolute
    /// value. Public rotor/versor domain policy remains undecided (W03/W07);
    /// internal call sites use this until then.
    pub(crate) fn normalize_versor(&self) -> Option<Self> {
        let q = self.norm_squared();
        (q > 0.0).then(|| self / q.sqrt())
    }

    /// Compute the multiplicative inverse, if the computed-pivot route certifies one.
    ///
    /// # Domain rule (computed-pivot policy)
    ///
    /// `inverse()` returns `Some(X)` when the fast path below certifies a
    /// computed scalar, or when LU with partial pivoting on the
    /// left-multiplication matrix completes without a zero pivot or a
    /// non-finite value. It returns `None` iff any post-pivot pivot is exactly
    /// `0.0` or any elimination/solution step produces a non-finite value.
    ///
    /// This is a **numerical accept/reject rule, not an exact-arithmetic
    /// invertibility certificate.** Rounding can erase a mathematically
    /// nonzero pivot — the near-singular input `1 + e1 + 1e-9·e2` in
    /// `Cl(2,0,0)` lies close to the zero divisor `(1+e1)(1-e1) = 0`, and a
    /// small enough perturbation rounds the last pivot to exactly zero — or it
    /// can admit an ill-conditioned solve whose result carries a large forward
    /// error even though the backward residual is small. There is **no
    /// `Some`-guarantee for every mathematically invertible `f64` input**;
    /// correctness is bounded by conditioning and by the representability of
    /// the coefficients.
    ///
    /// # Fast path
    ///
    /// The fast path computes `P = M·rev(M)` and, iff every non-scalar
    /// coefficient of `P` is exactly `0.0` **and** the scalar part is finite
    /// and nonzero, returns `rev(M) / λ` (a division, never a reciprocal
    /// multiplication). The membership test runs on the **computed** product:
    /// overflow, underflow, or cancellation can make a mathematically scalar
    /// product look non-scalar, or vice versa, so the fast path is not a
    /// certificate. In particular the scalar `1e200` has `λ = 1e400 = inf`, so
    /// it does **not** take the fast path; the LU route still returns the
    /// representable inverse `1e-200`. The quotient is accepted only when every
    /// coefficient is finite: a non-finite component means the true inverse is
    /// unrepresentable, so the fast path returns `None` instead of a `Some` with
    /// an infinite coefficient (round-1 P1-2: `1e-150 + 1e200·e3` in
    /// `Cl(1,0,1)`).
    ///
    /// # General path: LU with partial pivoting
    ///
    /// The general path forms the left-multiplication matrix `A` of `M`,
    /// whose column `j` holds the coefficients of `M·e_j` (`e_j` the `j`-th
    /// basis blade, `j = 0..N`, `N = 2^(P+Q+R)`), and solves `A·x = b` with
    /// `b` the coefficients of the scalar `1`. The result is `X = Σ_j x_j e_j`.
    /// Because the algebra is finite-dimensional and unital, a right inverse
    /// from `M·X = 1` is automatically the two-sided inverse.
    ///
    /// # Cost and storage
    ///
    /// The matrix is stored in a heap-allocated `Vec<f64>` of length `N²`
    /// (`N ≤ 64` for the workspace signatures), so the route costs `O(N²)`
    /// storage and `O(N³)` solve time. A `Vec` is used rather than a
    /// fixed-size array because this is not the exact-square hot path and `N`
    /// is a compile-time constant per instantiation.
    pub fn inverse(&self) -> Option<Self> {
        let rev = self.reverse();
        let product = self.geometric_product(&rev);
        let scalar = product.as_slice()[0];
        let non_scalar_is_zero = product.as_slice()[1..].iter().all(|&c| c == 0.0);

        if non_scalar_is_zero && scalar.is_finite() && scalar != 0.0 {
            let candidate = rev / scalar;
            // Finite-quotient guard (round-1 P1-2): a finite nonzero scalar
            // product does not guarantee a representable inverse. For
            // `1e-150 + 1e200·e3` in Cl(1,0,1) the e3 quotient is
            // `1e200 / 1e-300 = -inf`; the true inverse is unrepresentable, so
            // reject the candidate rather than return a `Some` with an infinite
            // component.
            if candidate.as_slice().iter().all(|c| c.is_finite()) {
                return Some(candidate);
            }
            return None;
        }

        self.inverse_via_lu()
    }

    /// General inverse route: LU with partial pivoting (computed-pivot policy).
    ///
    /// See [`Self::inverse`] for the domain rule. Returns `None` on a zero
    /// post-pivot pivot or a non-finite elimination/solution value.
    #[allow(clippy::needless_range_loop)] // matrix is addressed by computed row*N+col
    fn inverse_via_lu(&self) -> Option<Self> {
        let n = Self::BASIS_COUNT;

        // Column j of the left-multiplication matrix is the coefficient vector
        // of `self · e_j`.
        let mut matrix = vec![0.0f64; n * n];
        for j in 0..n {
            let mut basis_blade = Self::zero();
            basis_blade.set(j, 1.0);
            let column = self.geometric_product(&basis_blade);
            for (i, &value) in column.as_slice().iter().enumerate() {
                if !value.is_finite() {
                    return None;
                }
                matrix[i * n + j] = value;
            }
        }

        // Right-hand side: coefficients of the scalar `1`.
        let mut rhs = vec![0.0f64; n];
        rhs[0] = 1.0;

        // Forward elimination with partial pivoting.
        for k in 0..n {
            let mut pivot_row = k;
            let mut pivot_magnitude = matrix[k * n + k].abs();
            if !pivot_magnitude.is_finite() {
                return None;
            }
            for r in (k + 1)..n {
                let magnitude = matrix[r * n + k].abs();
                if !magnitude.is_finite() {
                    return None;
                }
                if magnitude > pivot_magnitude {
                    pivot_magnitude = magnitude;
                    pivot_row = r;
                }
            }
            if pivot_magnitude == 0.0 {
                return None;
            }
            if pivot_row != k {
                for c in 0..n {
                    matrix.swap(k * n + c, pivot_row * n + c);
                }
                rhs.swap(k, pivot_row);
            }

            let pivot = matrix[k * n + k];
            for r in (k + 1)..n {
                let factor = matrix[r * n + k] / pivot;
                if !factor.is_finite() {
                    return None;
                }
                matrix[r * n + k] = 0.0;
                rhs[r] -= factor * rhs[k];
                if !rhs[r].is_finite() {
                    return None;
                }
                for c in (k + 1)..n {
                    matrix[r * n + c] -= factor * matrix[k * n + c];
                    if !matrix[r * n + c].is_finite() {
                        return None;
                    }
                }
            }
        }

        // Back substitution.
        let mut solution = vec![0.0f64; n];
        for i in (0..n).rev() {
            let mut value = rhs[i];
            for j in (i + 1)..n {
                value -= matrix[i * n + j] * solution[j];
            }
            if !value.is_finite() {
                return None;
            }
            let quotient = value / matrix[i * n + i];
            if !quotient.is_finite() {
                return None;
            }
            solution[i] = quotient;
        }

        Some(Self::from_coefficients(solution))
    }

    /// Exponential map `exp(x)`.
    ///
    /// Returns `Result<Self, ExpError>` under the frozen CORE-W03 routing
    /// precedence (`docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md`,
    /// D02.3), with the round-2 scalar-factoring and exact-nilpotent
    /// amendments. In order:
    ///
    /// 1. exact zero — `1` exactly;
    /// 2. exact pure scalar `s` — `f64::exp(s)`, that value exactly;
    /// 3. **scalar factoring** (round-2): every scalar is central, so
    ///    `exp(s + y) = exp(s) · exp(y)`. The route writes `s` for the scalar
    ///    coefficient and `y` for the remaining non-scalar part and computes
    ///    `exp(y)` on the routes below, then multiplies the final result by the
    ///    `f64` coefficient `exp(s)`. A non-finite `exp(s)` is
    ///    `Err(NonFiniteResult { stage: ScalarRoute })`. Factoring makes the
    ///    scalar-plus-square-zero class exact (`y` nilpotent -> `1 + y`, times
    ///    `exp(s)`) and keeps every other route from spending squarings on the
    ///    (non-oscillatory) scalar direction.
    ///
    ///    On `y` (which has an exactly zero scalar part):
    ///    1. `y = 0` — result `1` (normally subsumed by precedence 2);
    ///    2. exact scalar square — if the exact dyadic square `y·y` is exactly
    ///       the scalar `lambda`: `lambda = 0` (with `y != 0`) is square-zero
    ///       and returns `1 + y` exactly; `lambda > 0` returns
    ///       `cosh(a) + (sinh(a)/a)·y`; `lambda < 0` returns
    ///       `cos(a) + (sin(a)/a)·y`. `a` comes from the exact magnitude
    ///       extraction (single-blade inputs short-circuit to `|c|`
    ///       bit-for-bit). When `cosh`/`sinh` overflow (large `a`) the
    ///       hyperbolic branch evaluates the symbolic exponential terms
    ///       `(e^{s+a} ± e^{s-a})/2` instead, so a finite `e^s·cosh(a)` is not
    ///       lost to an intermediate overflow. Non-finite trig or output ->
    ///       `ClosedForm`; a finite closed form pushed out of range only by the
    ///       symbolic scalar factor -> `ScalarRoute`;
    ///    3. **exact nilpotent of any index** (round-2, round-3 amendment):
    ///       iterate `y^(j+1) = y^j · y` with exact dyadic bignum powers until
    ///       the all-zero certificate (index `d <= 16`). The certification
    ///       search never converts to floating point; on a certificate the
    ///       polynomial `sum_{j<d} y^j / j!` is accumulated **coefficientwise
    ///       in exact integer arithmetic** as `N = sum_j y^j[blade]·(16!/j!)`
    ///       and `N / 16!` is rounded to `f64` **once** per coefficient. This
    ///       is exact where the exact rational value is representable and
    ///       preserves near-cancelling coefficients through the single final
    ///       rounding (the crafted `nextafter(1e10)` case). A missing
    ///       certificate (power or accumulator overflow, or the cap reached
    ///       with `y^d != 0`) is `NotNilpotent`; only a *certified* polynomial
    ///       with a non-finite post-division coefficient is `ClosedForm`;
    ///    4. bounded scaling-and-squaring series `exp(y)`.
    ///
    /// # Accuracy
    ///
    /// The scalar-square closed form is exact in its routing (the dyadic square
    /// is evaluated exactly) and loses only the final `sin`/`cos`/`sinh`/`cosh`
    /// evaluation and the single scaling of `y` by `sin(angle)/angle`: the
    /// angle is within 1 ulp of the exact value. The exact-nilpotent route is
    /// exact up to the single per-coefficient round-to-nearest-even conversion
    /// of `N/16!`; the certificate and the whole polynomial accumulation are
    /// exact, so it is exact wherever the exact rational coefficient is
    /// representable as `f64` and correctly rounded otherwise.
    ///
    /// The scalar-factoring route (precedence 3) carries `exp(s)` as a
    /// symbolic `mantissa · 2^E` (fdlibm `ln2` reduction), so a representable
    /// product survives even when `f64::exp(s)` itself underflows to `0` or
    /// overflows to `+inf`. The scale is composed **into** the downstream
    /// routes — the exact-polynomial finalization folds it into the single
    /// division rounding, and the other closed forms apply it through the
    /// single correctly-rounded `apply_symbolic_scale` — so no intermediate
    /// `c · mantissa` is ever formed in `f64` and no normalized-then-subnormal
    /// double rounding exists. If the result's scalar coefficient (`exp(s)`
    /// itself) is not representable the route reports
    /// `NonFiniteResult { stage: ScalarRoute }`.
    ///
    /// The series route is condition-aware: for an input whose dominant
    /// coefficient scale is `S`, the final relative error is at most
    /// `max(1e-6, C·S·u)`, where `u` is the f64 unit roundoff and `C = 2e4` is
    /// calibrated empirically (stage-2 evidence,
    /// `docs/plans/core-matrix-evidence/w03/stage2-*.log`). Repeated squaring
    /// amplifies absolute angle error by about `2^k`, so long-scale oscillatory
    /// inputs are inherently conditioned on their own f64 representation — an
    /// angle near `1e12` is only specified to `ulp(1e12) ≈ 1.2e-4` — and this
    /// API promises nothing tighter for them. **Scalar-square and nilpotent
    /// inputs never reach the series route** (precedences 3.2 and 3.3 carry
    /// exact certificates); the long-angle trio `0.1 + t·e12` is now routed
    /// through the closed form (its `y` has an exact scalar square) rather than
    /// the series, so the former series-route calibration numbers no longer
    /// describe it.
    ///
    /// Repeated squaring still evaluates the scalar channel through products
    /// whose cancellation can erase it at extreme scales (the documented
    /// squaring-channel hazard). With scalar factoring and the exact nilpotent
    /// route in place, the residual class that reaches the series is
    /// non-nilpotent with non-scalar square; the cancellation detector converts
    /// a silent collapse into an honest
    /// `Err(NonFiniteResult { stage: Squaring })`. The detector is conservative
    /// — it can also reject a legitimate sub-quadratic-growth input — and is a
    /// heuristic, not a proof: an honest error is preferred over a silent wrong
    /// result.
    pub fn exp(&self) -> Result<Self, ExpError> {
        // 1. Exact zero -> identity exactly.
        if self.is_zero() {
            return Ok(Self::scalar(1.0));
        }

        // 2. Exact pure scalar: every non-scalar coefficient is exactly zero.
        if self.as_slice()[1..].iter().all(|&c| c == 0.0) {
            let value = self.as_slice()[0].exp();
            if value.is_finite() {
                return Ok(Self::scalar(value));
            }
            return Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::ScalarRoute,
            });
        }

        // 3. Scalar factoring (round-2): scalars are central, so
        //    `exp(s + y) = exp(s) · exp(y)`. Route all of the downstream
        //    machinery on the non-scalar part `y`, then scale by `exp(s)`.
        let scalar = self.as_slice()[0];
        let mut y = self.clone();
        y.set(0, 0.0);
        // Symbolic scalar exponential (round-3 P1-1, unified round-4): exp(s)
        // is carried as `scale_mantissa * 2^scale_exponent` and composed into
        // the downstream routes, so a representable final result is reached
        // even when exp(s) itself underflows to 0 or overflows to +inf in f64.
        // Every route applies the scale with a single correctly-rounded
        // composition (`apply_symbolic_scale`, or the scaled exact-polynomial
        // finalization); no intermediate `c * scale_mantissa` is ever formed in
        // `f64`, which is what previously overflowed before the exponent could
        // rescue the product and double-rounded across the subnormal boundary.
        let (scale_mantissa, scale_exponent) = symbolic_exp_scale(scalar);
        let result = y.exp_nonscalar(scalar, scale_mantissa, scale_exponent)?;
        if result.as_slice().iter().all(|c| c.is_finite()) {
            Ok(result)
        } else {
            Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::ScalarRoute,
            })
        }
    }

    /// `exp` of a multivector with an exactly zero scalar part (the `y` of the
    /// scalar-factoring route in [`Self::exp`]).
    ///
    /// Precedence on `y`: zero -> identity; exact scalar square -> closed form;
    /// exact nilpotent of any index `<= 16` -> exact polynomial; otherwise the
    /// bounded scaling-and-squaring series.
    fn exp_nonscalar(
        &self,
        scalar: f64,
        scale_mantissa: f64,
        scale_exponent: i64,
    ) -> Result<Self, ExpError> {
        // 3.0 Defensive: `exp(0) = 1` (precedence 2 normally consumes this).
        if self.is_zero() {
            return Ok(Self::scale_by_symbolic(
                Self::scalar(1.0),
                scale_mantissa,
                scale_exponent,
            ));
        }

        // 3.1 Exact scalar square: the whole square-zero / circular /
        //     hyperbolic class has a closed form valid at ANY scale. The exact
        //     dyadic square is the only certificate used; no floating-point
        //     test guards the decision.
        let square = exact_x2(self);
        if !square.is_unsupported() && !square.has_non_finite_source() && square.is_exactly_scalar()
        {
            return self.exp_scalar_square(&square, scalar, scale_mantissa, scale_exponent);
        }

        // 3.2 Exact nilpotent of any index <= 16: `exp(y) = sum y^j / j!`,
        //     exact for the whole nilpotent class at any scale. The symbolic
        //     scalar scale is composed into the single final rounding of each
        //     coefficient. A missing certificate falls through to the series.
        match exact_square::exact_nilpotent_exp_scaled(self, scale_mantissa, scale_exponent) {
            exact_square::NilpotentOutcome::Exp(value) => return Ok(value),
            exact_square::NilpotentOutcome::NonFiniteClosedForm => {
                return Err(ExpError::NonFiniteResult {
                    stage: NonFiniteStage::ClosedForm,
                });
            }
            exact_square::NilpotentOutcome::NonFiniteScalarRoute => {
                return Err(ExpError::NonFiniteResult {
                    stage: NonFiniteStage::ScalarRoute,
                });
            }
            exact_square::NilpotentOutcome::NotNilpotent => {}
        }

        // 3.3 General non-nilpotent multivector -> bounded series on `y`, then
        //     the symbolic scale.
        let series = self.exp_series()?;
        Ok(Self::scale_by_symbolic(
            series,
            scale_mantissa,
            scale_exponent,
        ))
    }

    /// Precedence 3.1: exact scalar-square closed form for `y·y = lambda`.
    ///
    /// `self` is the non-scalar part `y` of the scalar-factoring route
    /// (exactly zero scalar coefficient); the caller multiplies the returned
    /// `exp(y)` by `exp(s)`.
    ///
    /// - `lambda == 0` (and `y != 0`): `y` is square-zero, so `exp(y) = 1 + y`
    ///   exactly, with every coefficient of `y` left untouched.
    /// - `lambda < 0`: `cos(a) + (sin(a)/a)·y` with `a = sqrt(|lambda|)`.
    /// - `lambda > 0`: `cosh(a) + (sinh(a)/a)·y`.
    ///
    /// `a` comes from the exact extraction in `exact_square`; the only
    /// floating-point operations are the final trig/hyperbolic evaluations and
    /// the single correctly-rounded symbolic scaling of `x`. A non-finite
    /// unscaled closed form is reported as `ExpError::NonFiniteResult { stage:
    /// ClosedForm }`; a finite closed form pushed out of range only by the
    /// symbolic scalar factor is `ScalarRoute`.
    fn exp_scalar_square(
        &self,
        square: &exact_square::ExactSquare,
        scalar: f64,
        scale_mantissa: f64,
        scale_exponent: i64,
    ) -> Result<Self, ExpError> {
        let (sign, angle) = square.scalar_sign_and_angle();

        // Scalar coefficient and the (vector) closed-form factor:
        //   lambda == 0 -> (1, 1);  circular -> (cos a, sin a / a);
        //   hyperbolic -> (cosh a, sinh a / a). `a` is positive here.
        let (scalar_coefficient, factor) = if sign == 0 {
            debug_assert!(
                square.is_zero(),
                "zero scalar square must certify square-zero on the scalar-square route"
            );
            (1.0, 1.0)
        } else if sign < 0 {
            // Circular: cos(angle) + (sin(angle)/angle)·x; cos/sin are bounded
            // for a finite angle (a non-finite angle is unrepresentable).
            (angle.cos(), angle.sin() / angle)
        } else {
            // Hyperbolic: cosh(angle) + (sinh(angle)/angle)·x. cosh/sinh may
            // overflow even when `e^s · cosh(a)` is finite, so in that regime
            // the scale is composed symbolically with the two exponential terms
            // `e^{s+a}` and `e^{s-a}` (a closed-form failure if the dominant
            // term is itself non-representable).
            let cosh = angle.cosh();
            let sinhc = angle.sinh() / angle;
            if !cosh.is_finite() || !sinhc.is_finite() {
                let result = self.exp_hyperbolic_symbolic(scalar, angle);
                return if result.as_slice().iter().all(|c| c.is_finite()) {
                    Ok(result)
                } else {
                    Err(ExpError::NonFiniteResult {
                        stage: NonFiniteStage::ClosedForm,
                    })
                };
            }
            (cosh, sinhc)
        };

        if !scalar_coefficient.is_finite() || !factor.is_finite() {
            // A non-finite angle made the unscaled closed form unrepresentable.
            return Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::ClosedForm,
            });
        }

        // Round-5: the vector coefficients are finalized with the three-way
        // symbolic multiply `c · factor · scale` (one rounding at destination
        // precision), never the `f64` `c · factor` that overflowed before the
        // scalar exponent rescued the finite result.
        let mut result = Self::zero();
        result.set(
            0,
            exact_square::apply_symbolic_scale(scalar_coefficient, scale_mantissa, scale_exponent),
        );

        let mut unscaled_finite = true;
        for (index, &coefficient) in self.as_slice().iter().enumerate().skip(1) {
            if coefficient == 0.0 {
                continue;
            }
            if !exact_square::apply_symbolic_scale3(coefficient, factor, 1.0, 0).is_finite() {
                unscaled_finite = false;
            }
            result.set(
                index,
                exact_square::apply_symbolic_scale3(
                    coefficient,
                    factor,
                    scale_mantissa,
                    scale_exponent,
                ),
            );
        }

        if result.as_slice().iter().all(|c| c.is_finite()) {
            Ok(result)
        } else if unscaled_finite {
            // The closed form was finite; the symbolic scalar scale pushed the
            // product out of range (the honest `ScalarRoute` outcome).
            Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::ScalarRoute,
            })
        } else {
            Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::ClosedForm,
            })
        }
    }

    /// Apply the symbolic scalar scale `scale_mantissa * 2^scale_exponent` to
    /// every coefficient of `value` with one correctly-rounded composition per
    /// coefficient. This is the unified round-4 replacement for the former
    /// `scale_by_pow2_rne(c * scale_mantissa, scale_exponent)`, which formed the
    /// mantissa product in `f64` first and could overflow before the exponent
    /// rescued it.
    fn scale_by_symbolic(mut value: Self, scale_mantissa: f64, scale_exponent: i64) -> Self {
        for coefficient in value.coefficients.iter_mut() {
            *coefficient =
                exact_square::apply_symbolic_scale(*coefficient, scale_mantissa, scale_exponent);
        }
        value
    }

    /// Hyperbolic scalar-square closed form in the regime where `cosh`/`sinh`
    /// overflow: `exp(s) · exp(a)` with `a` a positive hyperbolic angle.
    ///
    /// `exp(s)·cosh(a) = (e^{s+a} + e^{s-a})/2` and
    /// `exp(s)·sinh(a)/a = (e^{s+a} - e^{s-a})/(2a)`. Both exponentials are
    /// carried symbolically, so `e^{s+a}/2` may be representable even though
    /// `cosh(a)` is not. `cosh`/`sinh` overflow only for `a >~ 710`, where
    /// `e^{2a} > 2^2048`; the smaller term is then dominated by more than 53
    /// bits and is absorbed (its relative weight is below `2^-984`, far below
    /// the final rounding). If the dominant term `e^{s+a}/2` is itself not
    /// representable the caller sees a non-finite coefficient and reports the
    /// honest `ClosedForm` error.
    fn exp_hyperbolic_symbolic(&self, scalar: f64, angle: f64) -> Self {
        let (plus_mantissa, plus_exponent) = symbolic_exp_scale(scalar + angle);
        let (minus_mantissa, minus_exponent) = symbolic_exp_scale(scalar - angle);
        let (dominant_mantissa, dominant_exponent) = if plus_exponent >= minus_exponent {
            (plus_mantissa, plus_exponent)
        } else {
            (minus_mantissa, minus_exponent)
        };
        let scalar_part =
            exact_square::scale_by_pow2_rne(0.5 * dominant_mantissa, dominant_exponent);
        let mut result = Self::scalar(scalar_part);
        for (index, &coefficient) in self.as_slice().iter().enumerate().skip(1) {
            if coefficient != 0.0 {
                // Round-5: `e^{s+a}/(2a)·c` is finalized symbolically — the
                // exact rational `1/(2a) = 1/(2·m_a)·2^{-e_a}` is folded into
                // the single destination rounding, so `c = a` reproduces the
                // scalar `e^{s+a}/2` bit-for-bit. `2.0 * angle` is never formed.
                result.set(
                    index,
                    exact_square::apply_symbolic_scale_over_2a(
                        coefficient,
                        angle,
                        dominant_mantissa,
                        dominant_exponent,
                    ),
                );
            }
        }
        result
    }

    /// Smallest `k` with `||self / 2^k||_coeff <= 0.5`, together with the
    /// scaled element.
    ///
    /// The `log2` estimate is corrected against the actual scaled norm, so the
    /// frozen target and minimality hold even when `log2` rounds the
    /// next-above-power-of-two boundary the wrong way (round-1 P2: the norm
    /// `f64::from_bits((1123<<52)+1)` estimates `k = 101` while the frozen
    /// smallest `k` is 102).
    fn series_scaling(&self) -> Result<(usize, Self), ExpError> {
        let norm = self.coefficient_norm();
        if !norm.is_finite() {
            return Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::SeriesScaling,
            });
        }

        let mut k = if norm <= 0.5 {
            0usize
        } else {
            // `log2(2·norm)` without forming `2·norm`, which may overflow.
            let l = norm.log2() + 1.0;
            l.ceil().max(0.0) as usize
        };

        let mut scaled = self.scale_down(k);
        let mut scaled_norm = scaled.coefficient_norm();
        while scaled_norm > 0.5 {
            k += 1;
            scaled = self.scale_down(k);
            scaled_norm = scaled.coefficient_norm();
        }
        while k > 0 {
            let finer = self.scale_down(k - 1);
            if finer.coefficient_norm() <= 0.5 {
                k -= 1;
                scaled = finer;
                scaled_norm = scaled.coefficient_norm();
            } else {
                break;
            }
        }
        if !scaled_norm.is_finite() {
            return Err(ExpError::NonFiniteResult {
                stage: NonFiniteStage::SeriesScaling,
            });
        }
        Ok((k, scaled))
    }

    /// Bounded scaling-and-squaring series (D02.4).
    ///
    /// Scaling divides by a staged sequence of representable powers of two
    /// (never constructing `2^k`, which may be unrepresentable) until the
    /// scaled coefficient norm is at most `0.5`, accumulates at most 64 terms
    /// with `term_n = term_{n-1}·x/n` (division, never a reciprocal multiply),
    /// then applies the `k` squarings. Non-finite scaling, accumulation, and
    /// squaring values are reported as `ExpError::NonFiniteResult`; exhaustion
    /// of the 64-term cap is `ExpError::NoConvergence`.
    ///
    /// The `log2` scaling estimate is corrected against the actual scaled norm
    /// (round-1 P2), and each squaring is checked by the overflow-safe
    /// cancellation predicate [`squaring_is_collapse`] (round-1 detector,
    /// round-2 threshold fix): a norm collapse of more than ~62 bits while the
    /// previous norm is large is reported as
    /// `Err(NonFiniteResult { stage: Squaring })` rather than a silently wrong
    /// result. The detector is a heuristic, not a proof — it can miss
    /// cancellation (false negatives) and, being conservative, it can also
    /// reject a legitimate sub-quadratic-growth input (a residual false
    /// positive). Scalar-square and nilpotent inputs never reach this path
    /// (precedences 3.1 and 3.2 carry exact certificates), so the residual
    /// false-positive class is non-nilpotent, non-scalar-square growth; the
    /// outcome is an honest error, never a silently wrong value.
    fn exp_series(&self) -> Result<Self, ExpError> {
        // Degenerate zero input (precedence 1 normally handles this).
        if self.is_zero() {
            return Ok(Self::scalar(1.0));
        }

        // Smallest `k` with `||self / 2^k||_coeff <= 0.5`, with the `log2`
        // estimate corrected against the actual scaled norm (round-1 P2).
        let (k, scaled) = self.series_scaling()?;

        const MAX_TERMS: usize = 64;
        const EPS_REL: f64 = 1.0 / 68719476736.0; // 2^-36

        let mut result = Self::scalar(1.0);
        let mut term = Self::scalar(1.0);
        let mut term_peak_norm = term.coefficient_norm();
        let mut last_term_norm = term_peak_norm;
        let mut terms = 0usize;
        let mut converged = false;

        for n in 1..=MAX_TERMS {
            term = term.geometric_product(&scaled) / (n as f64);
            let term_norm = term.coefficient_norm();
            if !term_norm.is_finite() {
                return Err(ExpError::NonFiniteResult {
                    stage: NonFiniteStage::SeriesAccumulation,
                });
            }
            result = result + term.clone();
            let result_scale = result.coefficient_norm();
            if !result_scale.is_finite() {
                return Err(ExpError::NonFiniteResult {
                    stage: NonFiniteStage::SeriesAccumulation,
                });
            }

            terms = n;
            last_term_norm = term_norm;
            if term_norm > term_peak_norm {
                term_peak_norm = term_norm;
            }

            if term_norm <= EPS_REL * result_scale.max(term_peak_norm) {
                converged = true;
                break;
            }
        }

        if !converged {
            return Err(ExpError::NoConvergence {
                terms,
                last_term_norm,
                result_scale: result.coefficient_norm(),
            });
        }

        let mut previous_norm = result.coefficient_norm();
        for _ in 0..k {
            result = result.geometric_product(&result);
            let new_norm = result.coefficient_norm();
            if !new_norm.is_finite() {
                return Err(ExpError::NonFiniteResult {
                    stage: NonFiniteStage::Squaring,
                });
            }
            // Squaring cancellation detector (round-1 remediation, made
            // overflow-safe in round 2): a norm collapse of more than ~62 bits
            // while the previous norm is large means the scalar channel
            // cancelled catastrophically (the documented squaring-channel
            // hazard). Reported as an honest error rather than a silently
            // wrong representable result. This is a detector, not a proof; it
            // has false negatives, and it can conservatively reject a
            // legitimate sub-quadratic-growth input.
            if squaring_is_collapse(previous_norm, new_norm) {
                return Err(ExpError::NonFiniteResult {
                    stage: NonFiniteStage::Squaring,
                });
            }
            previous_norm = new_norm;
        }

        Ok(result)
    }

    /// Divide by `2^k` using staged divisions by representable powers of two.
    ///
    /// Never constructs `2^k` (unrepresentable for large `k`). Divisions by a
    /// power of two are exact exponent shifts while coefficients remain normal;
    /// subnormal coefficients round toward zero (documented in [`Self::exp`]).
    fn scale_down(&self, mut k: usize) -> Self {
        const POW2_64: f64 = 18446744073709551616.0; // 2^64, exactly representable
        let mut out = self.clone();
        while k >= 64 {
            out = out / POW2_64;
            k -= 64;
        }
        while k > 0 {
            out = out / 2.0;
            k -= 1;
        }
        out
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
            if mv_a.is_exact_zero() {
                continue;
            }

            for (b_grade, mv_b) in other_grades.iter().enumerate() {
                if mv_b.is_exact_zero() {
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
            if mv_a.is_exact_zero() {
                continue;
            }

            for (b_grade, mv_b) in other_grades.iter().enumerate() {
                if mv_b.is_exact_zero() {
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
            if self.coefficients[i] == 0.0 {
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

/// Scale every coefficient by division.
///
/// Division (rather than multiplication by a reciprocal) keeps extreme
/// magnitudes representable: `1.0 / n` overflows for subnormal `n`, which
/// would turn exact normalizations into infinities/NaNs.
impl<const P: usize, const Q: usize, const R: usize> Div<f64> for Multivector<P, Q, R> {
    type Output = Self;
    fn div(mut self, rhs: f64) -> Self {
        for c in self.coefficients.iter_mut() {
            *c /= rhs;
        }
        self
    }
}

impl<const P: usize, const Q: usize, const R: usize> Div<f64> for &Multivector<P, Q, R> {
    type Output = Multivector<P, Q, R>;
    fn div(self, rhs: f64) -> Multivector<P, Q, R> {
        let mut out = self.clone();
        for c in out.coefficients.iter_mut() {
            *c /= rhs;
        }
        out
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

    /// Coefficient magnitude: sqrt of the sum of squared coefficients.
    pub fn coefficient_norm(&self) -> f64 {
        self.mv.coefficient_norm()
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

    /// Normalize the vector; Some only for strictly positive metric square.
    ///
    /// Negative-square (e.g. timelike) and null vectors return None. No
    /// tolerance: a tiny positive-square vector normalizes exactly.
    pub fn normalize(&self) -> Option<Self> {
        let q = self.norm_squared();
        (q > 0.0).then(|| Self {
            mv: &self.mv / q.sqrt(),
        })
    }

    /// Signed metric quadratic form: the scalar product with the reverse.
    ///
    /// In mixed or degenerate signatures this may be negative or zero for
    /// nonzero vectors; it is not a magnitude. Use
    /// [`coefficient_norm`](Self::coefficient_norm) for the non-negative
    /// coefficient magnitude.
    pub fn norm_squared(&self) -> f64 {
        self.mv.norm_squared()
    }

    /// Compute the reverse (for vectors, this is the same as the original)
    pub fn reverse(&self) -> Self {
        Self {
            mv: self.mv.reverse(),
        }
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

    /// Coefficient magnitude: sqrt of the sum of squared coefficients.
    pub fn coefficient_norm(&self) -> f64 {
        self.mv.coefficient_norm()
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
        let rotor = bivector.exp().expect("bounded bivector exponential");

        // Check that rotor has unit norm
        assert!((rotor.coefficient_norm() - 1.0).abs() < 1e-10);
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
        assert_relative_eq!(
            left.coefficient_norm(),
            right.coefficient_norm(),
            epsilon = 1e-12
        );

        // Distributivity: a(b + c) = ab + ac
        let ab_plus_ac = a.geometric_product(&b) + a.geometric_product(&c);
        let a_times_b_plus_c = a.geometric_product(&(b.clone() + c.clone()));
        assert!((ab_plus_ac - a_times_b_plus_c).coefficient_norm() < 1e-12);

        // Reverse property: (ab)† = b†a†
        let ab_reverse = a.geometric_product(&b).reverse();
        let b_reverse_a_reverse = b.reverse().geometric_product(&a.reverse());
        assert!((ab_reverse - b_reverse_a_reverse).coefficient_norm() < 1e-12);
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
        assert!((mv - reconstructed).coefficient_norm() < 1e-12);
    }

    #[test]
    fn test_inner_and_outer_products() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e3 = Cl3::basis_vector(2);

        // Inner product of orthogonal vectors is zero
        assert!(e1.inner_product(&e2).coefficient_norm() < 1e-12);

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

    // ------------------------------------------------------------------
    // CORE-W03 round-1 remediation: scaling minimality / frozen target (P2)
    // ------------------------------------------------------------------

    #[test]
    fn series_scaling_meets_frozen_target_for_boundary_norm() {
        // Reviewer's next-above-power-of-two norm: the `log2` estimate rounds
        // to `k = 101`, whose scaled norm is 0.5000000000000001 > 0.5. The
        // corrected scaling must pick the frozen smallest `k = 102` (scaled
        // norm <= 0.5) and that `k` must be minimal.
        let norm = f64::from_bits((1123u64 << 52) + 1);
        let mut m = Multivector::<2, 0, 0>::zero();
        // Scalar + vector: the exact square carries a grade-1 part, so this
        // input routes to the series rather than the scalar-square class.
        m.set(0, 1.0);
        m.set(1, norm);

        let (k, scaled) = m.series_scaling().expect("finite scaling");
        assert_eq!(k, 102, "frozen smallest k for the boundary norm");
        assert!(
            scaled.coefficient_norm() <= 0.5,
            "scaled norm {} exceeds 0.5",
            scaled.coefficient_norm()
        );
        // Minimality: one less exponent exceeds the target.
        let finer = m.scale_down(k - 1);
        assert!(finer.coefficient_norm() > 0.5, "k is not minimal");
        // The estimate alone would have been 101 for the pure norm; the
        // scaled norm of the unrepaired estimate exceeds the target.
        let estimate = (norm.log2() + 1.0).ceil() as usize;
        assert_eq!(estimate, 101);
        assert!(m.scale_down(estimate).coefficient_norm() > 0.5);
    }

    // ------------------------------------------------------------------
    // CORE-W03 round-2 remediation: overflow-safe detector threshold
    // ------------------------------------------------------------------

    #[test]
    fn squaring_detector_threshold_is_overflow_safe() {
        // The old `prev * prev * 2^-62` form overflows to `+inf` at the
        // `1e155`/`f64::MAX` scale (well before `1e80` squared), which would
        // make every squaring look like a collapse. The ratio form keeps the
        // threshold `prev * 2^-62` finite for every finite `prev`.
        for previous in [1e80, 1e155, 1e200, f64::MAX] {
            let naive = previous * previous;
            let safe = previous * 2f64.powi(-62);
            assert!(safe.is_finite(), "threshold for {previous} is {safe}");
            if previous >= 1e155 {
                assert!(
                    naive.is_infinite(),
                    "precondition: naive square of {previous} overflows"
                );
            }
        }

        // Genuine collapse: the new norm is far below the previous square.
        assert!(squaring_is_collapse(1e200, 1.0));
        // Below the 2^30 gate the detector never fires.
        assert!(!squaring_is_collapse(1e8, 0.0));
        // A one-bit (factor-2) growth at a degenerate scale is conservative:
        // the detector may still reject it (documented false positive), but it
        // must not be driven by an infinitary threshold.
        let threshold = 1e80 * 2f64.powi(-62);
        assert!(threshold.is_finite() && threshold > 0.0);
    }

    // ------------------------------------------------------------------
    // CORE-W03 round-3 remediation: symbolic scalar scale composition
    // ------------------------------------------------------------------

    #[test]
    fn symbolic_scalar_scale_composition_matches_exp() {
        // The reduced-argument decomposition (fdlibm ln2 split) must reproduce
        // `f64::exp(s)` to ~1 ulp for in-range scalars.
        for s in [-20.0f64, -1.0, -0.5, 0.0, 0.3, 1.0, 5.0, 100.0] {
            let (mantissa, exponent) = symbolic_exp_scale(s);
            let composed = exact_square::scale_by_pow2_rne(mantissa, exponent);
            let reference = s.exp();
            let diff = (composed.to_bits() as i64 - reference.to_bits() as i64).unsigned_abs();
            assert!(diff <= 1, "s={s}: {composed} vs {reference} ({diff} ulp)");
        }

        // P1-1 underflow rescue: e^-750·1e300 must be bit-exact even though
        // `f64::exp(-750)` underflows to 0.
        let (mantissa, exponent) = symbolic_exp_scale(-750.0);
        let composed = exact_square::scale_by_pow2_rne(1e300 * mantissa, exponent);
        assert_eq!(
            composed.to_bits(),
            f64::from_bits(0x3A97_8AAE_2278_9158).to_bits()
        );

        // P1-1 overflow side: e^750·1e-300 is representable (the scalar
        // coefficient e^750 is not, so `exp` errors honestly, but the vector
        // coefficient must be right).
        let (mantissa, exponent) = symbolic_exp_scale(750.0);
        let composed = exact_square::scale_by_pow2_rne(1e-300 * mantissa, exponent);
        assert_eq!(
            composed.to_bits(),
            f64::from_bits(0x4545_BFA5_CD2C_9F0B).to_bits()
        );

        // `scale_by_pow2_rne` is a single correctly-rounded exponent step:
        // exact power-of-two rescales are bit-exact and subnormal results
        // round once (ties to even).
        assert_eq!(
            exact_square::scale_by_pow2_rne(1.5, 4).to_bits(),
            24f64.to_bits()
        );
        assert_eq!(
            exact_square::scale_by_pow2_rne(1.0, -1075).to_bits(),
            0.0f64.to_bits()
        );
        assert_eq!(
            exact_square::scale_by_pow2_rne(1.5, -1075).to_bits(),
            f64::from_bits(1).to_bits()
        );
    }
}
