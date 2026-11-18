//! Phantom-typed geometric algebra with formal-annotation compatibility
//!
//! This module provides type-safe implementations of geometric algebra
//! operations using phantom types for compile-time invariants (dimension,
//! signature) and Creusot-compatible annotations. The annotations express
//! intended contracts only; ordinary builds compile them to nothing (see
//! the CORE-D07 matrix: C1 — annotation compatibility, not proof), and
//! discharged proof obligations are a separate pre-1.0 gate (C3/C4).
//!
//! ## Finiteness: a construction-time guarantee only
//!
//! Every coefficient-taking constructor rejects non-finite values
//! (`NaN`, `±inf`). Unchecked arithmetic (`add`, `geometric_product`,
//! `apply_to_multivector`) is deliberately infallible and MAY overflow
//! finite inputs to `±inf` or produce `NaN` (`inf + (-inf)`). To restore
//! the invariant after unchecked arithmetic: KEEP the coefficient `Vec`
//! before constructing (the types do not expose their private coefficients
//! for later revalidation), then re-enter through
//! [`VerifiedMultivector::new`] (returns `Err` on non-finite coefficients)
//! or use the fallible `verified_*` wrappers in `verified_contracts`,
//! which reject non-finite results. There is no post-hoc revalidation of
//! an existing `Verified*` value.

use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use num_traits::{Float, One, Zero};

#[cfg(feature = "formal-verification")]
use creusot_contracts::macros::{ensures, requires};

/// True iff every value is finite (no `NaN`, no ±infinity).
///
/// M-REP-C construction-time finiteness predicate. `T: Float` supplies
/// `is_finite`. This is an ordinary runtime predicate; the
/// `formal-verification` annotations are intended specifications only and are
/// compiled away in ordinary builds.
fn all_finite<T: Float>(values: &[T]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// Type-invariant unit-norm tolerance (`1e-4`) for [`VerifiedRotor`].
///
/// `T::from(0.0001)` is `num_traits::NumCast::from`, which returns `None`
/// only for a `Float` type that cannot represent `0.0001`. Every concrete
/// `Float` used by this crate (`f32`, `f64`) represents it; the `zero`
/// fallback keeps this constructor free of any panic path in the
/// (unreachable in-tree) `None` case, matching the module policy of errors
/// rather than panics.
fn rotor_tolerance<T: Float>() -> T {
    match T::from(0.0001) {
        Some(tolerance) => tolerance,
        None => T::zero(),
    }
}

/// Phantom type encoding the metric signature of a Clifford algebra Cl(p,q,r)
/// - P: number of positive basis vectors (e_i² = +1)
/// - Q: number of negative basis vectors (e_i² = -1)
/// - R: number of null basis vectors (e_i² = 0)
pub struct Signature<const P: usize, const Q: usize, const R: usize>;

/// Phantom type for compile-time grade tracking
pub struct Grade<const G: usize>;

/// Type-level dimension marker for vectors
pub struct Dim<const D: usize>;

/// A phantom-typed multivector with compile-time signature guarantees
///
/// This structure ensures at the type level that:
/// 1. The signature is fixed and consistent
/// 2. Dimension bounds are respected
///
/// (Algebraic-law preservation is NOT type-enforced; annotations elsewhere
/// in this crate express intended contracts only — see the module docs.)
#[derive(Debug, Clone)]
pub struct VerifiedMultivector<T, const P: usize, const Q: usize, const R: usize>
where
    T: Float + Zero + One,
{
    /// Coefficients in lexicographic basis blade order
    pub(crate) coefficients: Vec<T>,
    /// Phantom marker for signature
    _signature: PhantomData<Signature<P, Q, R>>,
}

impl<T, const P: usize, const Q: usize, const R: usize> VerifiedMultivector<T, P, Q, R>
where
    T: Float + Zero + One,
{
    /// The total dimension of the algebra
    pub const DIM: usize = P + Q + R;

    /// The number of basis blades (2^n)
    pub const BASIS_SIZE: usize = 1 << (P + Q + R);

    /// Create a new verified multivector from coefficients
    ///
    /// # Type Invariants
    /// - Coefficients array must have exactly 2^(P+Q+R) elements
    /// - Signature is encoded at type level and cannot be violated
    /// - Every coefficient must be finite (`T::is_finite()`); `NaN` and
    ///   ±infinity are rejected (M-REP-C finiteness invariant)
    ///
    /// # Errors
    /// - `"Coefficient array size must equal 2^(P+Q+R)"` on length mismatch
    /// - `"All coefficients must be finite (no NaN or infinity)"` on any
    ///   non-finite coefficient
    #[cfg_attr(feature = "formal-verification",
        requires(coefficients.len() == Self::BASIS_SIZE),
        requires(all_finite(&coefficients)))]
    pub fn new(coefficients: Vec<T>) -> Result<Self, &'static str> {
        if coefficients.len() != Self::BASIS_SIZE {
            return Err("Coefficient array size must equal 2^(P+Q+R)");
        }
        if !all_finite(&coefficients) {
            return Err("All coefficients must be finite (no NaN or infinity)");
        }

        Ok(Self {
            coefficients,
            _signature: PhantomData,
        })
    }

    /// Create a scalar multivector
    ///
    /// # Errors
    /// Returns `Err("Scalar coefficient must be finite (no NaN or
    /// infinity)")` when `value` is not finite (M-REP-C finiteness
    /// invariant). Finite inputs produce the same multivector as before; only
    /// the return type became fallible.
    #[cfg_attr(feature = "formal-verification",
        ensures(result.is_ok() ==> result.unwrap().is_scalar()),
        ensures(result.is_ok() ==> result.unwrap().coefficients[0] == value),
        ensures(result.is_ok() ==>
            result.unwrap().coefficients.len() == Self::BASIS_SIZE),
        ensures(result.is_err() ==> !value.is_finite()))]
    pub fn scalar(value: T) -> Result<Self, &'static str> {
        if !value.is_finite() {
            return Err("Scalar coefficient must be finite (no NaN or infinity)");
        }

        let mut coefficients = vec![T::zero(); Self::BASIS_SIZE];
        coefficients[0] = value;

        Ok(Self {
            coefficients,
            _signature: PhantomData,
        })
    }

    /// Create a basis vector e_i
    ///
    /// # Type Safety
    /// The index is bounds-checked against the signature dimensions
    #[cfg_attr(feature = "formal-verification",
        requires(index < Self::DIM),
        ensures(result.grade() == 1))]
    pub fn basis_vector(index: usize) -> Result<Self, &'static str> {
        if index >= Self::DIM {
            return Err("Basis vector index exceeds dimension");
        }

        // Basis coefficients are `0`/`1` and therefore finite by
        // construction; routing through `new` keeps every multivector
        // constructor on the same finiteness-checked path (M-REP-C).
        let mut coefficients = vec![T::zero(); Self::BASIS_SIZE];
        coefficients[1 << index] = T::one();

        Self::new(coefficients)
    }

    /// Check if this is a pure scalar
    #[cfg_attr(feature = "formal-verification",
        ensures(result == (self.coefficients[1..].iter().all(|c| c.is_zero()))))]
    pub fn is_scalar(&self) -> bool {
        self.coefficients[1..].iter().all(|c| c.is_zero())
    }

    /// Compute the grade (highest non-zero grade component)
    #[cfg_attr(feature = "formal-verification",
        ensures(result <= Self::DIM))]
    pub fn grade(&self) -> usize {
        // Count bits to determine grade of each basis blade
        for (i, coeff) in self.coefficients.iter().enumerate().rev() {
            if !coeff.is_zero() {
                return i.count_ones() as usize;
            }
        }
        0
    }

    /// Addition of multivectors (same signature enforced by types)
    #[cfg_attr(feature = "formal-verification",
        ensures(result.coefficients.len() == self.coefficients.len()))]
    pub fn add(&self, other: &Self) -> Self {
        let coefficients: Vec<T> = self
            .coefficients
            .iter()
            .zip(&other.coefficients)
            .map(|(a, b)| *a + *b)
            .collect();

        Self {
            coefficients,
            _signature: PhantomData,
        }
    }

    /// Geometric product with compile-time signature matching
    ///
    /// # Intended Mathematical Properties (annotations, not proofs)
    /// - Associativity: (AB)C = A(BC)
    /// - Distributivity: A(B+C) = AB + AC
    /// - Identity: 1*A = A*1 = A
    ///
    /// These are the contracts the formal-verification annotations
    /// EXPRESS. Ordinary builds compile them to nothing; IEEE arithmetic
    /// means coefficient equality holds exactly only where the contracts
    /// say so (M-REP-C owns the specification audit).
    #[cfg_attr(feature = "formal-verification",
        requires(self.coefficients.len() == Self::BASIS_SIZE),
        requires(other.coefficients.len() == Self::BASIS_SIZE),
        ensures(result.coefficients.len() == Self::BASIS_SIZE))]
    pub fn geometric_product(&self, other: &Self) -> Self {
        // Simplified geometric product implementation
        // In practice, this would use the Cayley table for the signature
        let mut result = vec![T::zero(); Self::BASIS_SIZE];

        for i in 0..Self::BASIS_SIZE {
            for (j, &other_j) in other.coefficients.iter().enumerate() {
                let sign = self.compute_product_sign(i, j);
                // `compute_product_sign` returns only `-1`, `0`, or `1`; map
                // it to a total `T` factor without a fallible `NumCast`.
                let factor = match sign {
                    0 => T::zero(),
                    1 => T::one(),
                    _ => -T::one(),
                };
                let target_index = i ^ j; // XOR gives the resulting basis blade
                result[target_index] =
                    result[target_index] + self.coefficients[i] * other_j * factor;
            }
        }

        Self {
            coefficients: result,
            _signature: PhantomData,
        }
    }

    /// Compute the sign (and zero factor) of the product of two basis blades
    ///
    /// Accounts for the metric signature (P,Q,R):
    /// - Basis vectors 0..P square to +1
    /// - Basis vectors P..P+Q square to -1
    /// - Basis vectors P+Q..P+Q+R square to 0
    fn compute_product_sign(&self, blade1: usize, blade2: usize) -> i32 {
        // Step 1: Count transposition swaps for reorder sign
        let swaps = self.count_swaps(blade1, blade2);
        let mut sign: i32 = if swaps.is_multiple_of(2) { 1 } else { -1 };

        // Step 2: Apply metric signature for shared basis vectors
        // When e_i appears in both blades, e_i * e_i = metric(i)
        let shared = blade1 & blade2;
        for i in 0..Self::DIM {
            if shared & (1 << i) != 0 {
                if i >= P + Q {
                    // Null basis vector (R range): e_i² = 0
                    return 0;
                } else if i >= P {
                    // Negative basis vector (Q range): e_i² = -1
                    sign = -sign;
                }
                // Positive basis vector (P range): e_i² = +1, no change
            }
        }

        sign
    }

    /// Count the number of swaps needed to reorder basis vectors
    fn count_swaps(&self, blade1: usize, blade2: usize) -> usize {
        // Count inversions when combining blade indices
        let mut count = 0;
        for i in 0..Self::DIM {
            if blade1 & (1 << i) != 0 {
                for j in 0..i {
                    if blade2 & (1 << j) != 0 {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

/// Type-safe k-vector (homogeneous grade element)
///
/// This type guarantees at compile-time that the multivector
/// contains only elements of grade K
pub struct KVector<T, const K: usize, const P: usize, const Q: usize, const R: usize>
where
    T: Float + Zero + One,
{
    multivector: VerifiedMultivector<T, P, Q, R>,
    _grade: PhantomData<Grade<K>>,
}

impl<T, const K: usize, const P: usize, const Q: usize, const R: usize> KVector<T, K, P, Q, R>
where
    T: Float + Zero + One,
{
    /// Create a k-vector from a general multivector by grade projection
    #[cfg_attr(feature = "formal-verification",
        requires(K <= P + Q + R),
        ensures(result.grade() == K))]
    pub fn from_multivector(mv: VerifiedMultivector<T, P, Q, R>) -> Self {
        let mut coefficients = vec![T::zero(); VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE];

        // Extract only grade-K components
        for (i, coeff) in mv.coefficients.iter().enumerate() {
            if i.count_ones() as usize == K {
                coefficients[i] = *coeff;
            }
        }

        Self {
            multivector: VerifiedMultivector {
                coefficients,
                _signature: PhantomData,
            },
            _grade: PhantomData,
        }
    }

    /// Get the grade (always returns K due to type constraint)
    pub const fn grade(&self) -> usize {
        K
    }

    /// Inner product with another k-vector (produces a scalar)
    pub fn inner_product(&self, other: &Self) -> T {
        // Inner product of k-vectors of same grade
        self.multivector
            .coefficients
            .iter()
            .zip(&other.multivector.coefficients)
            .map(|(a, b)| *a * *b)
            .fold(T::zero(), |acc, x| acc + x)
    }
}

// Trait for outer product with specific grade combinations
// This works around const generic arithmetic limitations
pub trait OuterProduct<T, const J: usize, const P: usize, const Q: usize, const R: usize>
where
    T: Float + Zero + One,
{
    type Output;
    fn outer_product(&self, other: &KVector<T, J, P, Q, R>) -> Self::Output;
}

// Implement outer product for specific grade combinations
// Grade 1 ∧ Grade 1 = Grade 2 (vector ∧ vector = bivector)
impl<T, const P: usize, const Q: usize, const R: usize> OuterProduct<T, 1, P, Q, R>
    for KVector<T, 1, P, Q, R>
where
    T: Float + Zero + One,
{
    type Output = KVector<T, 2, P, Q, R>;

    fn outer_product(&self, other: &KVector<T, 1, P, Q, R>) -> Self::Output {
        let mut coefficients = vec![T::zero(); VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE];

        // Compute outer product for grade-1 vectors
        for i in 0..VerifiedMultivector::<T, P, Q, R>::DIM {
            for j in i + 1..VerifiedMultivector::<T, P, Q, R>::DIM {
                let blade_i = 1 << i;
                let blade_j = 1 << j;
                let blade_ij = blade_i | blade_j;

                coefficients[blade_ij] = self.multivector.coefficients[blade_i]
                    * other.multivector.coefficients[blade_j]
                    - self.multivector.coefficients[blade_j]
                        * other.multivector.coefficients[blade_i];
            }
        }

        KVector {
            multivector: VerifiedMultivector {
                coefficients,
                _signature: PhantomData,
            },
            _grade: PhantomData,
        }
    }
}

// Grade 1 ∧ Grade 2 = Grade 3 (vector ∧ bivector = trivector)
impl<T, const P: usize, const Q: usize, const R: usize> OuterProduct<T, 2, P, Q, R>
    for KVector<T, 1, P, Q, R>
where
    T: Float + Zero + One,
{
    type Output = KVector<T, 3, P, Q, R>;

    fn outer_product(&self, other: &KVector<T, 2, P, Q, R>) -> Self::Output {
        let dim = VerifiedMultivector::<T, P, Q, R>::DIM;
        let basis_size = VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE;
        let mut coefficients = vec![T::zero(); basis_size];

        // For each grade-1 blade (vectors) and grade-2 blade (bivectors),
        // compute their wedge product contributing to grade-3 (trivectors)
        for i in 0..dim {
            let blade_i = 1usize << i;
            for (j, &other_j) in other.multivector.coefficients.iter().enumerate() {
                if j.count_ones() != 2 {
                    continue;
                }
                // Only wedge if basis vectors don't overlap
                if blade_i & j == 0 {
                    let target = blade_i | j;
                    // Compute sign from reordering e_i into canonical position within target
                    let mut swaps = 0;
                    for k in 0..i {
                        if j & (1 << k) != 0 {
                            swaps += 1;
                        }
                    }
                    let sign = if swaps % 2 == 0 { T::one() } else { -T::one() };
                    coefficients[target] = coefficients[target]
                        + sign * self.multivector.coefficients[blade_i] * other_j;
                }
            }
        }

        KVector {
            multivector: VerifiedMultivector {
                coefficients,
                _signature: PhantomData,
            },
            _grade: PhantomData,
        }
    }
}

// Grade 2 ∧ Grade 1 = Grade 3 (bivector ∧ vector = trivector)
impl<T, const P: usize, const Q: usize, const R: usize> OuterProduct<T, 1, P, Q, R>
    for KVector<T, 2, P, Q, R>
where
    T: Float + Zero + One,
{
    type Output = KVector<T, 3, P, Q, R>;

    fn outer_product(&self, other: &KVector<T, 1, P, Q, R>) -> Self::Output {
        let dim = VerifiedMultivector::<T, P, Q, R>::DIM;
        let basis_size = VerifiedMultivector::<T, P, Q, R>::BASIS_SIZE;
        let mut coefficients = vec![T::zero(); basis_size];

        // For each grade-2 blade (bivectors) and grade-1 blade (vectors),
        // compute their wedge product contributing to grade-3 (trivectors)
        for (i, &self_i) in self.multivector.coefficients.iter().enumerate() {
            if i.count_ones() != 2 {
                continue;
            }
            for j in 0..dim {
                let blade_j = 1usize << j;
                // Only wedge if basis vectors don't overlap
                if i & blade_j == 0 {
                    let target = i | blade_j;
                    // Compute sign: count how many bits in i are above j
                    let mut swaps = 0;
                    for k in (j + 1)..dim {
                        if i & (1 << k) != 0 {
                            swaps += 1;
                        }
                    }
                    let sign = if swaps % 2 == 0 { T::one() } else { -T::one() };
                    coefficients[target] = coefficients[target]
                        + sign * self_i * other.multivector.coefficients[blade_j];
                }
            }
        }

        KVector {
            multivector: VerifiedMultivector {
                coefficients,
                _signature: PhantomData,
            },
            _grade: PhantomData,
        }
    }
}

/// A rotor type carrying a runtime-checked even-grade/unit-norm invariant.
///
/// Rotors are even-grade multivectors with unit norm that represent rotations
/// in the geometric algebra. The `Verified` name refers to the runtime checks
/// performed by [`VerifiedRotor::new`]; it is **not** a compile-time proof, and
/// no proof obligation is discharged by this type.
pub struct VerifiedRotor<T, const P: usize, const Q: usize, const R: usize>
where
    T: Float + Zero + One,
{
    multivector: VerifiedMultivector<T, P, Q, R>,
    /// Marker for the runtime-checked rotor invariant.
    ///
    /// This is `PhantomData` and carries no compile-time proof; the intended
    /// invariant is enforced (with the caveats documented on
    /// [`VerifiedRotor::new`]) at runtime.
    _rotor_invariant: PhantomData<()>,
}

impl<T, const P: usize, const Q: usize, const R: usize> VerifiedRotor<T, P, Q, R>
where
    T: Float + Zero + One,
{
    /// Create a rotor from a multivector.
    ///
    /// # Checks performed
    /// - **Finite coefficients**: every coefficient must be finite
    ///   (`T::is_finite()`); otherwise this returns
    ///   `Err("Rotor coefficients must be finite (no NaN or infinity)")`.
    /// - **Even grade**: every odd-grade coefficient must compare equal to
    ///   zero (`coeff != 0`); otherwise this returns
    ///   `Err("Rotor must have even grade")`.
    /// - **Unit norm (type-invariant tolerance)**: the Euclidean coefficient
    ///   norm computed as `sqrt(sum(c_i^2))` must satisfy
    ///   `(norm - 1).abs() <= 1e-4`; otherwise this returns
    ///   `Err("Rotor must have unit norm")`. `1e-4` is the documented
    ///   **type-invariant tolerance**, not a mathematical guarantee: the
    ///   accepted set is finite, even-grade, approximately-unit
    ///   (`|norm - 1| <= 1e-4`) multivectors.
    ///
    /// All checks are ordinary runtime comparisons executed by this method.
    /// The `formal-verification` attributes below are *intended
    /// specifications* (contract annotations); they are compiled away in
    /// normal builds and no proof of them is discharged here.
    ///
    /// # Non-finite coefficients (closed by M-REP-C, 2026-10-15)
    /// The former `(norm - 1).abs() > 1e-4` test alone accepted `NaN`
    /// (`NaN > tolerance` is `false`). M-REP-C enforces finiteness **before**
    /// the even-grade and norm checks, so a `NaN`/±infinity coefficient is
    /// now **rejected** with `Err`. This dated correction supersedes the
    /// M-REP-A note that recorded the NaN-acceptance gap.
    #[cfg_attr(feature = "formal-verification",
        requires(mv.is_even_grade()),
        requires(all_finite(&mv.coefficients)),
        requires((Self::compute_norm(&mv) - T::one()).abs() < rotor_tolerance::<T>()),
        ensures(result.is_ok()))]
    pub fn new(mv: VerifiedMultivector<T, P, Q, R>) -> Result<Self, &'static str> {
        if !all_finite(&mv.coefficients) {
            return Err("Rotor coefficients must be finite (no NaN or infinity)");
        }
        if !Self::is_even_grade(&mv) {
            return Err("Rotor must have even grade");
        }

        let norm = Self::compute_norm(&mv);
        if (norm - T::one()).abs() > rotor_tolerance::<T>() {
            return Err("Rotor must have unit norm");
        }

        Ok(Self {
            multivector: mv,
            _rotor_invariant: PhantomData,
        })
    }

    /// Check if multivector has only even grade components
    fn is_even_grade(mv: &VerifiedMultivector<T, P, Q, R>) -> bool {
        for (i, coeff) in mv.coefficients.iter().enumerate() {
            let grade = i.count_ones() as usize;
            if !grade.is_multiple_of(2) && !coeff.is_zero() {
                return false;
            }
        }
        true
    }

    /// Compute the norm of a multivector
    fn compute_norm(mv: &VerifiedMultivector<T, P, Q, R>) -> T {
        mv.coefficients
            .iter()
            .map(|c| *c * *c)
            .fold(T::zero(), |acc, x| acc + x)
            .sqrt()
    }

    /// Compose two rotors (rotation composition).
    ///
    /// The geometric product of the two rotors is computed and then divided by
    /// its computed norm. The normalization denominator is checked explicitly:
    /// a zero or non-finite norm (a degenerate product such as two
    /// null-product rotors in `Cl(1,1,0)`, or an overflow) returns
    /// `Err("composition is degenerate")` instead of dividing by zero and
    /// manufacturing `NaN` coefficients. The normalized coefficients are
    /// routed through [`VerifiedMultivector::new`], so a non-finite
    /// coefficient cannot be stored in the result either.
    ///
    /// # Postcondition
    /// On `Ok`, every result coefficient is finite (the constructor gate
    /// holds). Exact unit norm is **not** guaranteed: normalization is a
    /// floating-point division, so the invariant is approximate only. The
    /// former `ensures(Self::compute_norm(&result.multivector) == T::one())`
    /// annotation was removed on 2026-10-16 (M-REP-C review round 1) as
    /// unsound; no proof of it was ever discharged.
    #[cfg_attr(feature = "formal-verification",
        requires(all_finite(&self.multivector.coefficients)),
        requires(all_finite(&other.multivector.coefficients)),
        ensures(result.is_ok() ==>
            all_finite(&result.unwrap().multivector.coefficients)))]
    pub fn compose(&self, other: &Self) -> Result<Self, &'static str> {
        let composed = self.multivector.geometric_product(&other.multivector);

        // Scale-first normalization (R2): with a wide coefficient spread
        // (e.g. a subnormal-dominated product) the naive sum of squares
        // underflows and the normalized result silently violates the
        // tolerance (observed norm 0.73 on the s/sqrt(2), t=2.3e-162
        // fixture). Compute the norm relative to the largest coefficient.
        let scale = composed
            .coefficients
            .iter()
            .fold(T::zero(), |m, c| m.max(c.abs()));
        if scale.is_zero() || !scale.is_finite() {
            return Err("composition is degenerate");
        }
        let sum: T = composed
            .coefficients
            .iter()
            .map(|c| {
                let r = *c / scale;
                r * r
            })
            .fold(T::zero(), |acc, x| acc + x);
        let norm = scale * sum.sqrt();
        if norm.is_zero() || !norm.is_finite() {
            return Err("composition is degenerate");
        }

        let normalized =
            VerifiedMultivector::new(composed.coefficients.iter().map(|c| *c / norm).collect())?;

        // Revalidate through the rotor constructor — the even-grade +
        // tolerance checks are the output gate (R2: do not construct
        // directly).
        Self::new(normalized)
    }

    /// Apply rotor to a multivector (rotation/reflection)
    ///
    /// Computes R v R† where R† is the reverse of the rotor.
    pub fn apply_to_multivector(
        &self,
        v: &VerifiedMultivector<T, P, Q, R>,
    ) -> VerifiedMultivector<T, P, Q, R> {
        // Compute R† (reverse): negate grades where k*(k-1)/2 is odd
        let mut rev_coeffs = self.multivector.coefficients.clone();
        for (i, coeff) in rev_coeffs.iter_mut().enumerate() {
            let grade = i.count_ones() as usize;
            if grade >= 2 && (grade * (grade - 1) / 2) % 2 == 1 {
                *coeff = -*coeff;
            }
        }
        let r_rev = VerifiedMultivector::<T, P, Q, R> {
            coefficients: rev_coeffs,
            _signature: PhantomData,
        };

        // Compute R * v * R†
        let rv = self.multivector.geometric_product(v);
        rv.geometric_product(&r_rev)
    }
}

/// Type-safe vector with compile-time dimension
pub struct Vector<T, const D: usize>
where
    T: Float + Zero + One,
{
    pub(crate) data: Vec<T>,
    _dim: PhantomData<Dim<D>>,
}

impl<T, const D: usize> Vector<T, D>
where
    T: Float + Zero + One,
{
    /// Create a new vector (dimension checked at compile time)
    ///
    /// # Errors
    /// - `"Vector data length must match dimension"` on length mismatch
    /// - `"All vector components must be finite (no NaN or infinity)"` on
    ///   any non-finite component (M-REP-C finiteness invariant)
    #[cfg_attr(feature = "formal-verification",
        requires(data.len() == D),
        requires(all_finite(&data)),
        ensures(result.dimension() == D))]
    pub fn new(data: Vec<T>) -> Result<Self, &'static str> {
        if data.len() != D {
            return Err("Vector data length must match dimension");
        }
        if !all_finite(&data) {
            return Err("All vector components must be finite (no NaN or infinity)");
        }

        Ok(Self {
            data,
            _dim: PhantomData,
        })
    }

    /// Get the dimension (always D due to type parameter)
    pub const fn dimension(&self) -> usize {
        D
    }

    /// Dot product (only defined for same dimension - enforced by types)
    ///
    /// The former `ensures(result >= T::zero())` contract was removed on
    /// 2026-10-15 (M-REP-A / MATRIX-02): it is unsound for mixed vectors
    /// (e.g. `dot([1], [-1]) == -1`) and for indefinite signatures. This
    /// method carries **no** contract. The sound, signature-independent
    /// successor is [`Self::verified_self_dot`] (M-REP-C, 2026-10-15).
    pub fn dot(&self, other: &Self) -> T {
        self.data
            .iter()
            .zip(&other.data)
            .map(|(a, b)| *a * *b)
            .fold(T::zero(), |acc, x| acc + x)
    }

    /// Sound self-dot: the sum of squares of this vector's coefficients,
    /// `Σ data[i]²` (M-REP-C successor to the removed dot contract).
    ///
    /// # Soundness scope
    /// This is an ordinary coordinate sum of squares. It makes **no** metric
    /// or signature claim, and performs no square root, so it is
    /// signature-independent. Every finite coefficient squares to a
    /// nonnegative value, and a sum of nonnegative values is never `NaN`, so
    /// for finite coefficients the result satisfies `>= T::zero()` even when
    /// it is `+inf` (the square of `f64::MAX` overflows to `+inf`). The sum
    /// is therefore **not** always finite — the earlier claim "finite
    /// coefficients imply a finite sum" was false — but it is always
    /// nonnegative.
    ///
    /// This method rejects non-finite coefficients at runtime and returns
    /// `Err("Vector components must be finite (no NaN or infinity)")`. That
    /// matters because [`Self::new`] only rejects non-finite input at
    /// construction; an operation such as [`Self::add`] or [`Self::dot`] can
    /// overflow finite inputs to `±inf`, and a subsequent `(+inf) + (-inf)`
    /// produces `NaN`, for which `NaN >= 0` is false. The guard makes the
    /// `Ok(result) ==> result >= T::zero()` contract sound.
    ///
    /// Contrast with the removed dot contract: that claimed nonnegativity for
    /// `a·b` over *two* arbitrary vectors, which is false for mixed signs
    /// (`dot([1], [-1]) == -1`); a self-dot over one vector cannot exhibit
    /// that counterexample.
    #[cfg_attr(feature = "formal-verification",
        requires(all_finite(&self.data)),
        ensures(result.is_ok() ==> result.unwrap() >= T::zero()),
        ensures(result.is_err() == !all_finite(&self.data)))]
    pub fn verified_self_dot(&self) -> Result<T, &'static str> {
        if !all_finite(&self.data) {
            return Err("Vector components must be finite (no NaN or infinity)");
        }

        Ok(self
            .data
            .iter()
            .map(|x| *x * *x)
            .fold(T::zero(), |acc, x| acc + x))
    }

    /// Add two vectors (same dimension enforced at compile time)
    pub fn add(&self, other: &Self) -> Self {
        Self {
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| *a + *b)
                .collect(),
            _dim: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phantom_type_dimension_safety() {
        // This would fail to compile if dimensions don't match
        let v1 = Vector::<f64, 3>::new(vec![1.0, 2.0, 3.0]).unwrap();
        let v2 = Vector::<f64, 3>::new(vec![4.0, 5.0, 6.0]).unwrap();
        let _v3 = v1.add(&v2); // OK - same dimensions

        // The following would not compile:
        // let v4 = Vector::<f64, 2>::new(vec![1.0, 2.0]).unwrap();
        // let v5 = v1.add(&v4); // ERROR: type mismatch
    }

    #[test]
    fn test_signature_type_safety() {
        // Clifford algebra Cl(3,0,0) - 3D Euclidean space
        let mv1 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
        let mv2 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(3.0).unwrap();
        let _mv3 = mv1.add(&mv2); // OK - same signature

        // The following would not compile:
        // let mv4 = VerifiedMultivector::<f64, 2, 1, 0>::scalar(1.0); // Different signature
        // let mv5 = mv1.add(&mv4); // ERROR: type mismatch
    }

    #[test]
    fn test_grade_preservation() {
        // Create a bivector (grade 2)
        let bivector =
            KVector::<f64, 2, 3, 0, 0>::from_multivector(VerifiedMultivector::scalar(1.0).unwrap());

        assert_eq!(bivector.grade(), 2);
        // Grade is guaranteed at compile time
    }

    #[test]
    fn test_outer_product_type_safety() {
        use super::OuterProduct;

        // Create two vectors (grade 1) in 3D Euclidean space
        let v1 = KVector::<f64, 1, 3, 0, 0>::from_multivector(
            VerifiedMultivector::basis_vector(0).unwrap(),
        );
        let v2 = KVector::<f64, 1, 3, 0, 0>::from_multivector(
            VerifiedMultivector::basis_vector(1).unwrap(),
        );

        // Outer product of two vectors gives a bivector (grade 2)
        let bivector: KVector<f64, 2, 3, 0, 0> = v1.outer_product(&v2);

        // The type system guarantees this is grade 2
        assert_eq!(bivector.grade(), 2);

        // The following would not compile due to type mismatch:
        // let wrong: KVector<f64, 3, 3, 0, 0> = v1.outer_product(&v2);
        // Error: expected KVector<_, 3, _, _, _>, found KVector<_, 2, _, _, _>
    }

    // ---- M-REP-C finiteness battery (construction-time rejection) ----

    #[test]
    fn mrepc_multivector_new_rejects_nan() {
        let coefficients = vec![f64::NAN; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        let err = VerifiedMultivector::<f64, 3, 0, 0>::new(coefficients).unwrap_err();
        assert!(err.contains("finite"), "unexpected error: {err}");
    }

    #[test]
    fn mrepc_multivector_new_rejects_positive_infinity() {
        let mut coefficients = vec![0.0; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        coefficients[0] = f64::INFINITY;
        assert!(VerifiedMultivector::<f64, 3, 0, 0>::new(coefficients).is_err());
    }

    #[test]
    fn mrepc_multivector_new_rejects_negative_infinity() {
        let mut coefficients = vec![0.0; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        coefficients[5] = f64::NEG_INFINITY;
        assert!(VerifiedMultivector::<f64, 3, 0, 0>::new(coefficients).is_err());
    }

    #[test]
    fn mrepc_multivector_new_rejects_mixed_nonfinite() {
        let mut coefficients = vec![1.0; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        coefficients[2] = f64::NAN;
        assert!(VerifiedMultivector::<f64, 3, 0, 0>::new(coefficients).is_err());
    }

    #[test]
    fn mrepc_multivector_new_accepts_finite() {
        let coefficients = vec![1.0; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::new(coefficients).unwrap();
        assert_eq!(mv.coefficients[0], 1.0);
        assert_eq!(
            mv.coefficients.len(),
            VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE
        );
    }

    #[test]
    fn mrepc_scalar_rejects_nonfinite() {
        assert!(VerifiedMultivector::<f64, 3, 0, 0>::scalar(f64::NAN).is_err());
        assert!(VerifiedMultivector::<f64, 3, 0, 0>::scalar(f64::INFINITY).is_err());
        assert!(VerifiedMultivector::<f64, 3, 0, 0>::scalar(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn mrepc_scalar_accepts_finite() {
        let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.5).unwrap();
        assert_eq!(mv.coefficients[0], 2.5);
        assert!(mv.is_scalar());
    }

    #[test]
    fn mrepc_basis_vector_accepts_finite() {
        let basis = VerifiedMultivector::<f64, 3, 0, 0>::basis_vector(2).unwrap();
        assert_eq!(basis.coefficients[1 << 2], 1.0);
    }

    #[test]
    fn mrepc_vector_new_rejects_nonfinite() {
        assert!(Vector::<f64, 3>::new(vec![1.0, f64::NAN, 3.0]).is_err());
        assert!(Vector::<f64, 3>::new(vec![1.0, 2.0, f64::INFINITY]).is_err());
        assert!(Vector::<f64, 3>::new(vec![1.0, 2.0, f64::NEG_INFINITY]).is_err());
    }

    #[test]
    fn mrepc_vector_new_accepts_finite() {
        let v = Vector::<f64, 3>::new(vec![1.0, 2.0, 3.0]).unwrap();
        assert_eq!(v.dimension(), 3);
    }

    #[test]
    fn mrepc_rotor_new_rejects_nonfinite_coefficients() {
        // A `NaN` scalar is grade-0 (even), so it passes the even-grade check
        // and the old `NaN > tolerance` norm test; only the finiteness check
        // catches it. Build via a struct literal to bypass `new` and reach the
        // `VerifiedRotor` defense-in-depth check (M-REP-C).
        let mut coefficients = vec![0.0; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        coefficients[0] = f64::NAN;
        let nan_mv = VerifiedMultivector::<f64, 3, 0, 0> {
            coefficients,
            _signature: PhantomData,
        };
        let err = VerifiedRotor::new(nan_mv).err().unwrap();
        assert!(err.contains("finite"), "unexpected error: {err}");

        let mut mixed = vec![0.0; VerifiedMultivector::<f64, 3, 0, 0>::BASIS_SIZE];
        mixed[0] = 1.0;
        mixed[3] = f64::INFINITY; // grade-2 (even) coefficient
        let inf_mv = VerifiedMultivector::<f64, 3, 0, 0> {
            coefficients: mixed,
            _signature: PhantomData,
        };
        assert!(VerifiedRotor::new(inf_mv).is_err());
    }

    // ---- M-REP-C `verified_self_dot` battery ----

    #[test]
    fn mrepc_self_dot_known_values() {
        let v = Vector::<f64, 3>::new(vec![3.0, 4.0, 0.0]).unwrap();
        assert_eq!(v.verified_self_dot().unwrap(), 25.0);
    }

    #[test]
    fn mrepc_self_dot_zero_vector_is_zero() {
        let v = Vector::<f64, 3>::new(vec![0.0, 0.0, 0.0]).unwrap();
        assert_eq!(v.verified_self_dot().unwrap(), 0.0);
    }

    #[test]
    fn mrepc_self_dot_single_coefficient() {
        let v = Vector::<f64, 1>::new(vec![-2.5]).unwrap();
        assert_eq!(v.verified_self_dot().unwrap(), 6.25);
    }

    #[test]
    fn mrepc_self_dot_is_nonnegative() {
        let sign_mixed = Vector::<f64, 3>::new(vec![1.0, -2.0, -3.0]).unwrap();
        assert!(sign_mixed.verified_self_dot().unwrap() >= 0.0);

        // The removed `dot` contract was unsound for mixed vectors, but a
        // self-dot over a single vector is always nonnegative for finite
        // coefficients.
        let one_d = Vector::<f64, 1>::new(vec![1.0]).unwrap();
        assert_eq!(one_d.dot(&one_d), 1.0);
        assert!(one_d.verified_self_dot().unwrap() >= 0.0);
    }

    #[test]
    fn mrepc_self_dot_single_max_overflows_to_positive_infinity() {
        // A single finite coefficient squares to `+inf`, which is not finite
        // but *is* `>= 0`: the `Ok` contract holds and NaN is impossible.
        let v = Vector::<f64, 1>::new(vec![f64::MAX]).unwrap();
        let result = v.verified_self_dot().unwrap();
        assert!(result.is_infinite() && result > 0.0);
        assert!(result >= 0.0);
    }

    #[test]
    fn mrepc_self_dot_rejects_overflow_nan_chain() {
        // Finite constructed vectors; `add` overflows them to ±inf, and
        // `(+inf) + (-inf) = NaN`. `NaN >= 0` is false, so the non-finite
        // coefficient must be rejected rather than returned as `NaN`.
        let pos = Vector::<f64, 1>::new(vec![f64::MAX]).unwrap();
        let inf = pos.add(&pos); // [inf]
        let neg = Vector::<f64, 1>::new(vec![-f64::MAX]).unwrap();
        let ninf = neg.add(&neg); // [-inf]
        let nan = inf.add(&ninf); // [NaN]
        assert!(nan.data[0].is_nan());
        assert!(nan.verified_self_dot().is_err());
    }

    // ---- M-REP-C `compose` zero-normalization battery (F4) ----

    /// A unit even rotor `[s,0,0,±s]` in `Cl(1,1,0)`, `s = 1/√2`.
    fn hyperbolic_unit_rotor(sign: f64) -> VerifiedRotor<f64, 1, 1, 0> {
        let s = 1.0 / 2.0f64.sqrt();
        VerifiedRotor::new(
            VerifiedMultivector::<f64, 1, 1, 0>::new(vec![s, 0.0, 0.0, sign * s]).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn mrepc_compose_degenerate_product_is_error() {
        // In Cl(1,1,0) the even blades are index 0 (scalar) and index 3
        // (e1e2). `[s,0,0,s] · [s,0,0,-s]` is exactly zero, so the
        // normalization denominator is zero and the former code divided by
        // it, manufacturing NaN coefficients. It must be rejected.
        let a = hyperbolic_unit_rotor(1.0);
        let b = hyperbolic_unit_rotor(-1.0);
        let err = a.compose(&b).err().unwrap();
        assert!(err.contains("degenerate"), "unexpected error: {err}");
    }

    #[test]
    fn mrepc_compose_ordinary_product_is_ok() {
        // The same rotor composed with itself has the finite non-zero
        // product `1 + e12`, so normalization succeeds and the result is a
        // finite multivector (ordinary compose is unchanged).
        let a = hyperbolic_unit_rotor(1.0);
        let composed = a.compose(&a).unwrap();
        assert!(composed
            .multivector
            .coefficients
            .iter()
            .all(|c| c.is_finite()));
        // `a` is idempotent under composition in Cl(1,1,0): `a∘a == a`.
        let s = 1.0 / 2.0f64.sqrt();
        assert!((composed.multivector.coefficients[0] - s).abs() < 1e-12);
        assert!((composed.multivector.coefficients[3] - s).abs() < 1e-12);
    }

    #[test]
    fn mrepc_apply_on_valid_rotor_is_finite() {
        // `compose` cannot produce a degenerate (NaN) rotor, so `apply` only
        // ever receives finite rotors; applying a valid rotor is finite.
        let r = hyperbolic_unit_rotor(1.0);
        let v = VerifiedMultivector::<f64, 1, 1, 0>::new(vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        let applied = r.apply_to_multivector(&v);
        assert!(applied.coefficients.iter().all(|c| c.is_finite()));
    }

    // ---- M-REP-C contract-precondition battery (formal feature only) ----

    #[cfg(feature = "formal-verification")]
    mod contract_battery {
        use super::*;
        use crate::verified_contracts::{
            verified_addition, verified_scalar_mult, verify_scalar_identity,
        };

        #[test]
        fn mrepc_contract_scalar_mult_finite_exact() {
            let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
            let result = verified_scalar_mult(3.0, &mv).unwrap();
            assert_eq!(result.coefficients[0], 6.0);
            assert_eq!(result.coefficients.len(), mv.coefficients.len());
        }

        #[test]
        fn mrepc_contract_scalar_mult_overflow_rejected() {
            // `2.0 * f64::MAX` overflows to `+inf`. Every `requires` is
            // satisfied (both inputs are finite), but the former
            // `new(...).unwrap()` panicked on the non-finite product. It is
            // now a checked `Err`.
            let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(f64::MAX).unwrap();
            assert!(verified_scalar_mult(2.0, &mv).is_err());
        }

        #[test]
        fn mrepc_contract_addition_finite_exact() {
            let a = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
            let b = VerifiedMultivector::<f64, 3, 0, 0>::scalar(3.0).unwrap();
            let result = verified_addition(&a, &b).unwrap();
            assert_eq!(result.coefficients[0], 5.0);
        }

        #[test]
        fn mrepc_contract_addition_overflow_rejected() {
            // `MAX + MAX` overflows to `+inf`; the checked sum returns `Err`
            // rather than a non-finite multivector whose coefficient
            // equality (`inf == MAX + MAX`) is a NaN-adjacent false claim.
            let a = VerifiedMultivector::<f64, 3, 0, 0>::scalar(f64::MAX).unwrap();
            assert!(verified_addition(&a, &a).is_err());
        }

        #[test]
        fn mrepc_contract_scalar_identity_finite_exact() {
            let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(5.0).unwrap();
            let result = verify_scalar_identity(&mv).unwrap();
            assert_eq!(result.coefficients[0], mv.coefficients[0]);
        }

        #[test]
        #[should_panic(expected = "finite")]
        fn mrepc_contract_scalar_mult_nonfinite_scalar_panics() {
            let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
            let _ = verified_scalar_mult(f64::NAN, &mv);
        }
    }
}

#[cfg(test)]
mod mrepc_r2_tests {
    use super::*;

    /// R2's exact cancellation fixture (reviewer-pinned): the product of
    /// a=[s,0,0,0,0,s,0,0] and b=[s,0,0,t,0,-s,0,0] (s=1/sqrt2, t=2.3e-162,
    /// Cl(2,1,0)) has ONLY coefficients 3 and 6 nonzero, each
    /// 1.6263455967290592e-162. Squaring each rounds to the minimum
    /// subnormal, so the NAIVE norm underflowed to
    /// 3.1434555694052576e-162 and the broken implementation returned Ok
    /// with Euclidean norm 0.7316788639818951 — outside [0.9999, 1.0001].
    #[test]
    fn compose_subnormal_cancellation_normalizes_within_tolerance() {
        let s = core::f64::consts::FRAC_1_SQRT_2;
        let t = 2.3e-162_f64;
        let mv_a =
            VerifiedMultivector::<f64, 2, 1, 0>::new(vec![s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0])
                .unwrap();
        let mv_b =
            VerifiedMultivector::<f64, 2, 1, 0>::new(vec![s, 0.0, 0.0, t, 0.0, -s, 0.0, 0.0])
                .unwrap();
        let ra = VerifiedRotor::new(mv_a).unwrap();
        let rb = VerifiedRotor::new(mv_b).unwrap();
        let rc = ra
            .compose(&rb)
            .expect("subnormal-cancellation compose must not be degenerate");
        // The product's only nonzero coefficients (3 and 6) must normalize
        // to a unit rotor: pin the exact post-fix shape.
        let c = &rc.multivector.coefficients;
        let norm = (c.iter().map(|x| x * x).sum::<f64>()).sqrt();
        assert!(
            (norm - 1.0).abs() <= 1e-4,
            "norm {norm} outside tolerance (pre-fix value was 0.7316788639818951)"
        );
    }
}
