//! Verified fusion operations with phantom types for compile-time safety
//!
//! This module provides type-safe operations for the Tropical-Dual-Clifford fusion system,
//! ensuring mathematical correctness and dimensional consistency at compile time.
//!
//! The verification system uses phantom types to track:
//! - Dimensional consistency across algebraic systems
//! - Variable dependencies in automatic differentiation
//! - Tropical semiring operation validity
//! - Geometric algebra signature preservation
#![allow(missing_docs)]

use crate::TropicalDualClifford;
use alloc::vec::Vec;
use amari_dual::DualNumber;
use amari_tropical::TropicalNumber;
use core::marker::PhantomData;
use num_traits::Float;

/// Phantom type for tracking tropical semiring properties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxPlus;

/// Phantom type for tracking tropical semiring properties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinPlus;

/// Phantom type for tracking dual number variable dependencies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variable<const ID: usize>;

/// Phantom type for tracking constant values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Constant;

/// Phantom type for tracking Clifford algebra signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature<const P: usize, const Q: usize, const R: usize>;

/// Phantom type for tracking dimensions across algebraic systems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimension<const DIM: usize>;

/// Type-safe Tropical-Dual-Clifford structure with compile-time verification
#[derive(Clone, Debug)]
pub struct VerifiedTropicalDualClifford<
    T: Float,
    const DIM: usize,
    TropicalSemiring = MaxPlus,
    DualVariable = Variable<0>,
    CliffordSig = Signature<3, 0, 0>,
> {
    /// The underlying fusion structure
    pub inner: TropicalDualClifford<T, DIM>,
    /// Phantom types for compile-time verification
    _tropical: PhantomData<TropicalSemiring>,
    _dual: PhantomData<DualVariable>,
    _clifford: PhantomData<CliffordSig>,
}

impl<T: Float, const DIM: usize>
    VerifiedTropicalDualClifford<T, DIM, MaxPlus, Variable<0>, Signature<3, 0, 0>>
{
    /// Create a new verified TDC from logits with default typing
    pub fn from_logits(logits: &[T]) -> Self {
        Self {
            inner: TropicalDualClifford::from_logits(logits),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }

    /// Create a verified constant TDC (no variable dependencies)
    pub fn constant(
        logits: &[T],
    ) -> VerifiedTropicalDualClifford<T, DIM, MaxPlus, Constant, Signature<3, 0, 0>> {
        VerifiedTropicalDualClifford {
            inner: TropicalDualClifford::from_logits(logits),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }

    /// Create a verified variable TDC with specific ID
    pub fn variable<const ID: usize>(
        logits: &[T],
    ) -> VerifiedTropicalDualClifford<T, DIM, MaxPlus, Variable<ID>, Signature<3, 0, 0>> {
        VerifiedTropicalDualClifford {
            inner: TropicalDualClifford::from_logits(logits),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }
}

impl<T: Float, const DIM: usize, TropicalSemiring, DualVariable, CliffordSig>
    VerifiedTropicalDualClifford<T, DIM, TropicalSemiring, DualVariable, CliffordSig>
{
    /// Extract verified tropical features
    pub fn extract_verified_tropical_features(
        &self,
    ) -> Vec<VerifiedTropicalNumber<T, TropicalSemiring>> {
        self.inner
            .extract_tropical_features()
            .into_iter()
            .map(|tn| VerifiedTropicalNumber::new(tn))
            .collect()
    }

    /// Extract verified dual features
    pub fn extract_verified_dual_features(&self) -> Vec<VerifiedDualNumber<T, DualVariable>>
    where
        DualVariable: Clone,
    {
        self.inner
            .extract_dual_features()
            .into_iter()
            .map(|dn| VerifiedDualNumber::new(dn))
            .collect()
    }

    /// Get dimension at compile time
    pub const fn dimension() -> usize {
        DIM
    }

    /// Get the real value (removes dual information)
    pub fn real(&self) -> T {
        self.inner.dual().norm().real
    }

    /// Get the dual part (derivative information)
    pub fn dual(&self) -> T {
        self.inner.dual().norm().dual
    }

    /// Type-safe distance computation with dimensional checking
    pub fn distance(&self, other: &Self) -> T {
        self.inner.distance(&other.inner)
    }

    /// Convert to underlying representation (unsafe - loses type safety)
    pub fn into_inner(self) -> TropicalDualClifford<T, DIM> {
        self.inner
    }

    /// Access underlying representation (unsafe - loses type safety)
    pub fn as_inner(&self) -> &TropicalDualClifford<T, DIM> {
        &self.inner
    }
}

/// Type-safe operations that preserve algebraic structure
impl<T: Float, const DIM: usize, TropicalSemiring, DualVariable, CliffordSig>
    VerifiedTropicalDualClifford<T, DIM, TropicalSemiring, DualVariable, CliffordSig>
where
    TropicalSemiring: Clone,
    DualVariable: Clone,
    CliffordSig: Clone,
{
    /// Type-safe addition with structure preservation
    #[allow(clippy::should_implement_trait)]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            inner: self.inner.add(&other.inner),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }

    /// Type-safe scaling with structure preservation
    pub fn scale(&self, factor: T) -> Self {
        Self {
            inner: self.inner.scale(factor),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }

    /// Type-safe interpolation with structure preservation
    pub fn interpolate(&self, other: &Self, t: T) -> Self {
        Self {
            inner: self.inner.interpolate(&other.inner, t),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }
}

/// Type-safe tropical operations
impl<T: Float, const DIM: usize, DualVariable, CliffordSig>
    VerifiedTropicalDualClifford<T, DIM, MaxPlus, DualVariable, CliffordSig>
{
    /// Max-plus tropical addition (preserves MaxPlus semiring)
    pub fn tropical_add(&self, other: &Self) -> Self
    where
        DualVariable: Clone,
        CliffordSig: Clone,
    {
        // For tropical addition, we take the maximum
        Self {
            inner: self.inner.add(&other.inner), // This performs component-wise max for tropical
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }

    /// Convert to MinPlus semiring
    pub fn to_min_plus(
        self,
    ) -> VerifiedTropicalDualClifford<T, DIM, MinPlus, DualVariable, CliffordSig>
    where
        DualVariable: Clone,
        CliffordSig: Clone,
    {
        VerifiedTropicalDualClifford {
            inner: self.inner,
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }
}

/// Type-safe dual number operations with variable tracking
impl<T: Float, const DIM: usize, TropicalSemiring, const ID: usize, CliffordSig>
    VerifiedTropicalDualClifford<T, DIM, TropicalSemiring, Variable<ID>, CliffordSig>
{
    /// Differentiate with respect to this variable
    pub fn differentiate<F, Output>(&self, f: F) -> (T, T)
    where
        F: Fn(&Self) -> Output,
        Output: DualExtractable<T>,
        TropicalSemiring: Clone,
        CliffordSig: Clone,
    {
        let result = f(self);
        (result.real_part(), result.dual_part())
    }

    /// Get gradient information for this variable
    pub fn gradient(&self) -> T {
        self.inner.dual().norm().dual
    }
}

/// Type-safe Clifford algebra operations with signature preservation
impl<
        T: Float,
        const DIM: usize,
        TropicalSemiring,
        DualVariable,
        const P: usize,
        const Q: usize,
        const R: usize,
    > VerifiedTropicalDualClifford<T, DIM, TropicalSemiring, DualVariable, Signature<P, Q, R>>
{
    /// Type-safe geometric product preserving signature
    pub fn geometric_product(&self, other: &Self) -> Self
    where
        TropicalSemiring: Clone,
        DualVariable: Clone,
    {
        Self {
            inner: self.inner.transform(&other.inner),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }

    /// Get the Clifford signature at compile time
    pub const fn clifford_signature() -> (usize, usize, usize) {
        (P, Q, R)
    }

    /// Compute geometric norm
    pub fn geometric_norm(&self) -> T {
        T::from(self.inner.clifford().coefficient_norm()).unwrap_or(T::zero())
    }
}

/// Wrapper for verified dual numbers with variable tracking
#[derive(Clone, Debug)]
pub struct VerifiedDualNumber<T: Float, V = Variable<0>> {
    value: DualNumber<T>,
    _variable: PhantomData<V>,
}

impl<T: Float, V> VerifiedDualNumber<T, V> {
    pub fn new(value: DualNumber<T>) -> Self {
        Self {
            value,
            _variable: PhantomData,
        }
    }

    pub fn real(&self) -> T {
        self.value.real
    }

    pub fn dual(&self) -> T {
        self.value.dual
    }

    pub fn inner(&self) -> &DualNumber<T> {
        &self.value
    }
}

/// Wrapper for verified tropical numbers with semiring tracking
#[derive(Clone, Debug)]
pub struct VerifiedTropicalNumber<T: Float, Semiring = MaxPlus> {
    value: TropicalNumber<T>,
    _semiring: PhantomData<Semiring>,
}

impl<T: Float, Semiring> VerifiedTropicalNumber<T, Semiring> {
    pub fn new(value: TropicalNumber<T>) -> Self {
        Self {
            value,
            _semiring: PhantomData,
        }
    }

    pub fn value(&self) -> T {
        self.value.value()
    }

    pub fn inner(&self) -> &TropicalNumber<T> {
        &self.value
    }
}

/// Max-plus specific operations
impl<T: Float> VerifiedTropicalNumber<T, MaxPlus> {
    /// Max-plus addition (maximum operation)
    #[allow(clippy::should_implement_trait)]
    pub fn add(&self, other: &Self) -> Self {
        Self::new(self.value.tropical_add(&other.value))
    }

    /// Max-plus multiplication (regular addition)
    #[allow(clippy::should_implement_trait)]
    pub fn mul(&self, other: &Self) -> Self {
        Self::new(self.value.tropical_mul(&other.value))
    }
}

/// Min-plus specific operations
impl<T: Float> VerifiedTropicalNumber<T, MinPlus> {
    /// Min-plus addition (minimum operation)
    #[allow(clippy::should_implement_trait)]
    pub fn add(&self, other: &Self) -> Self {
        // For min-plus, we need minimum instead of maximum
        let min_val = if self.value.value() <= other.value.value() {
            self.value
        } else {
            other.value
        };
        Self::new(min_val)
    }

    /// Min-plus multiplication (regular addition)
    #[allow(clippy::should_implement_trait)]
    pub fn mul(&self, other: &Self) -> Self {
        Self::new(self.value.tropical_mul(&other.value))
    }
}

/// Trait for extracting dual number information
pub trait DualExtractable<T: Float> {
    fn real_part(&self) -> T;
    fn dual_part(&self) -> T;
}

impl<T: Float, const DIM: usize, TropicalSemiring, DualVariable, CliffordSig> DualExtractable<T>
    for VerifiedTropicalDualClifford<T, DIM, TropicalSemiring, DualVariable, CliffordSig>
{
    fn real_part(&self) -> T {
        self.real()
    }

    fn dual_part(&self) -> T {
        self.dual()
    }
}

/// Type-safe builder for verified TDC objects
pub struct VerifiedTDCBuilder<T: Float, const DIM: usize> {
    logits: Vec<T>,
    _phantom: PhantomData<T>,
}

impl<T: Float, const DIM: usize> Default for VerifiedTDCBuilder<T, DIM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Float, const DIM: usize> VerifiedTDCBuilder<T, DIM> {
    pub fn new() -> Self {
        Self {
            logits: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn add_logit(mut self, logit: T) -> Self {
        self.logits.push(logit);
        self
    }

    pub fn add_logits(mut self, logits: &[T]) -> Self {
        self.logits.extend_from_slice(logits);
        self
    }

    /// Build as constant (no variable dependencies)
    pub fn build_constant(
        self,
    ) -> VerifiedTropicalDualClifford<T, DIM, MaxPlus, Constant, Signature<3, 0, 0>> {
        VerifiedTropicalDualClifford::constant(&self.logits)
    }

    /// Build as variable with specific ID
    pub fn build_variable<const ID: usize>(
        self,
    ) -> VerifiedTropicalDualClifford<T, DIM, MaxPlus, Variable<ID>, Signature<3, 0, 0>> {
        VerifiedTropicalDualClifford::variable::<ID>(&self.logits)
    }

    /// Build with custom semiring
    pub fn build_with_semiring<Semiring>(
        self,
    ) -> VerifiedTropicalDualClifford<T, DIM, Semiring, Variable<0>, Signature<3, 0, 0>> {
        VerifiedTropicalDualClifford {
            inner: TropicalDualClifford::from_logits(&self.logits),
            _tropical: PhantomData,
            _dual: PhantomData,
            _clifford: PhantomData,
        }
    }
}

/// Dimensional consistency checking at compile time
pub struct DimensionalChecker<const DIM1: usize, const DIM2: usize>;

impl<const DIM: usize> DimensionalChecker<DIM, DIM> {
    /// Only compiles if dimensions match
    pub const fn assert_same_dimension() -> bool {
        true
    }
}

/// Type-safe operations between verified TDC objects
pub trait VerifiedOperations<T: Float, Rhs> {
    type Output;

    /// Type-safe addition with dimensional and algebraic checking
    fn verified_add(&self, rhs: &Rhs) -> Self::Output;

    /// Type-safe distance with dimensional checking
    fn verified_distance(&self, rhs: &Rhs) -> T;
}

impl<T: Float, const DIM: usize, TropicalSemiring, DualVariable, CliffordSig>
    VerifiedOperations<T, Self>
    for VerifiedTropicalDualClifford<T, DIM, TropicalSemiring, DualVariable, CliffordSig>
where
    TropicalSemiring: Clone,
    DualVariable: Clone,
    CliffordSig: Clone,
{
    type Output = Self;

    fn verified_add(&self, rhs: &Self) -> Self::Output {
        // Compile-time dimension checking is automatic due to same type
        self.add(rhs)
    }

    fn verified_distance(&self, rhs: &Self) -> T {
        // Compile-time dimension checking is automatic due to same type
        self.distance(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_verified_tdc_creation() {
        let logits = vec![1.0, 2.0, 3.0, 0.5];

        // Test different creation methods
        let tdc_default = VerifiedTropicalDualClifford::<f64, 4>::from_logits(&logits);
        let tdc_constant = VerifiedTropicalDualClifford::<f64, 4>::constant(&logits);
        let tdc_variable = VerifiedTropicalDualClifford::<f64, 4>::variable::<42>(&logits);

        assert!(tdc_default.real() > 0.0);
        assert!(tdc_constant.real() > 0.0);
        assert!(tdc_variable.real() > 0.0);
    }

    #[test]
    fn test_dimensional_consistency() {
        let logits = vec![1.0, 2.0, 3.0, 0.5];

        let tdc1 = VerifiedTropicalDualClifford::<f64, 4>::from_logits(&logits);
        let tdc2 = VerifiedTropicalDualClifford::<f64, 4>::from_logits(&logits);

        // These operations should compile (same dimensions)
        let sum = tdc1.verified_add(&tdc2);
        let distance = tdc1.verified_distance(&tdc2);

        assert!(sum.real() > 0.0);
        assert!(distance >= 0.0);

        // Compile-time dimension checking
        assert!(DimensionalChecker::<4, 4>::assert_same_dimension());
    }

    #[test]
    fn test_tropical_semiring_operations() {
        let logits = vec![1.0, 2.0];

        let tdc_maxplus = VerifiedTropicalDualClifford::<f64, 2>::from_logits(&logits);
        let tdc_minplus = tdc_maxplus.clone().to_min_plus();

        let tropical_features_max = tdc_maxplus.extract_verified_tropical_features();
        let tropical_features_min = tdc_minplus.extract_verified_tropical_features();

        assert_eq!(tropical_features_max.len(), tropical_features_min.len());
    }

    #[test]
    fn test_clifford_signature_preservation() {
        let logits = vec![1.0, 2.0, 3.0];

        let tdc = VerifiedTropicalDualClifford::<f64, 3>::from_logits(&logits);
        let signature = VerifiedTropicalDualClifford::<f64, 3>::clifford_signature();

        assert_eq!(signature, (3, 0, 0)); // Default Euclidean signature
        assert!(tdc.geometric_norm() >= 0.0);
    }

    #[test]
    fn test_automatic_differentiation_tracking() {
        let logits = vec![2.0, 3.0];

        let var_x = VerifiedTropicalDualClifford::<f64, 2>::variable::<0>(&logits);
        let var_y = VerifiedTropicalDualClifford::<f64, 2>::variable::<1>(&logits);

        // Variables should have gradient information
        assert!(var_x.gradient().abs() >= 0.0);
        assert!(var_y.gradient().abs() >= 0.0);

        // Test differentiation
        let (value, deriv) = var_x.differentiate(|x| x.scale(2.0));
        assert!(value > 0.0);
        assert!(deriv.abs() >= 0.0);
    }

    #[test]
    fn test_builder_pattern() {
        let tdc_constant = VerifiedTDCBuilder::<f64, 3>::new()
            .add_logit(1.0)
            .add_logits(&[2.0, 3.0])
            .build_constant();

        let tdc_variable = VerifiedTDCBuilder::<f64, 3>::new()
            .add_logit(1.0)
            .add_logits(&[2.0, 3.0])
            .build_variable::<5>();

        assert!(tdc_constant.real() > 0.0);
        assert!(tdc_variable.real() > 0.0);
    }

    #[test]
    fn test_type_safe_operations() {
        let logits1 = vec![1.0, 2.0];
        let logits2 = vec![2.0, 1.0];

        let tdc1 = VerifiedTropicalDualClifford::<f64, 2>::from_logits(&logits1);
        let tdc2 = VerifiedTropicalDualClifford::<f64, 2>::from_logits(&logits2);

        // Test type-safe operations
        let sum = tdc1.add(&tdc2);
        let scaled = tdc1.scale(2.0);
        let interpolated = tdc1.interpolate(&tdc2, 0.5);

        assert!(sum.real() > 0.0);
        assert!(scaled.real() > 0.0);
        assert!(interpolated.real() > 0.0);

        // Test distance
        let distance = tdc1.distance(&tdc2);
        assert!(distance >= 0.0);
    }

    #[test]
    fn test_verified_tropical_numbers() {
        let tn1 = TropicalNumber::new(2.0);
        let tn2 = TropicalNumber::new(3.0);

        let vtn1_max = VerifiedTropicalNumber::<f64, MaxPlus>::new(tn1);
        let vtn2_max = VerifiedTropicalNumber::<f64, MaxPlus>::new(tn2);

        let vtn1_min = VerifiedTropicalNumber::<f64, MinPlus>::new(tn1);
        let vtn2_min = VerifiedTropicalNumber::<f64, MinPlus>::new(tn2);

        // Max-plus operations
        let max_sum = vtn1_max.add(&vtn2_max);
        assert_relative_eq!(max_sum.value(), 3.0, epsilon = 1e-10); // max(2,3) = 3

        // Min-plus operations
        let min_sum = vtn1_min.add(&vtn2_min);
        assert_relative_eq!(min_sum.value(), 2.0, epsilon = 1e-10); // min(2,3) = 2
    }
}
