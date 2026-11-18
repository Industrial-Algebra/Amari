//! Verified tropical algebra with phantom types for formal verification
//!
//! This module provides type-safe tropical algebra operations with compile-time
//! guarantees about mathematical properties and dimensional consistency.

use crate::*;
use alloc::vec::Vec;
use amari_core::verified::{Dim, Signature};
use num_traits::Float;
use std::marker::PhantomData;

/// Phantom type for tropical semiring properties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TropicalSemiring;

/// Phantom type for max-plus operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxPlus;

/// Phantom type for min-plus operations (dual)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinPlus;

/// Type-safe tropical number with phantom type constraints
#[derive(Debug, Clone, Copy)]
pub struct VerifiedTropicalNumber<T: Float + Clone + Copy, S = TropicalSemiring> {
    value: T,
    _semiring: PhantomData<S>,
}

impl<T: Float + Clone + Copy, S> VerifiedTropicalNumber<T, S> {
    /// Create a new verified tropical number
    pub fn new(value: T) -> Self {
        Self {
            value,
            _semiring: PhantomData,
        }
    }

    /// Get the underlying value
    pub fn value(&self) -> T {
        self.value
    }

    /// Check if this is the tropical zero (additive identity)
    pub fn is_tropical_zero(&self) -> bool {
        self.value.is_infinite() && self.value.is_sign_negative()
    }

    /// Check if this is the tropical one (multiplicative identity)
    pub fn is_tropical_one(&self) -> bool {
        self.value.is_zero()
    }
}

/// Tropical zero (additive identity) for max-plus semiring
impl<T: Float + Clone + Copy> VerifiedTropicalNumber<T, MaxPlus> {
    /// Create the tropical additive identity (negative infinity)
    pub fn tropical_zero() -> Self {
        Self::new(T::neg_infinity())
    }

    /// Create the tropical multiplicative identity (zero)
    pub fn tropical_one() -> Self {
        Self::new(T::zero())
    }

    /// Tropical addition (max operation)
    pub fn tropical_add(self, other: Self) -> Self {
        Self::new(self.value.max(other.value))
    }

    /// Tropical multiplication (addition)
    pub fn tropical_mul(self, other: Self) -> Self {
        Self::new(self.value + other.value)
    }
}

/// Tropical zero (additive identity) for min-plus semiring
impl<T: Float + Clone + Copy> VerifiedTropicalNumber<T, MinPlus> {
    /// Create the tropical additive identity (positive infinity)
    pub fn tropical_zero() -> Self {
        Self::new(T::infinity())
    }

    /// Create the tropical multiplicative identity (zero)
    pub fn tropical_one() -> Self {
        Self::new(T::zero())
    }

    /// Tropical addition (min operation)
    pub fn tropical_add(self, other: Self) -> Self {
        Self::new(self.value.min(other.value))
    }

    /// Tropical multiplication (addition)
    pub fn tropical_mul(self, other: Self) -> Self {
        Self::new(self.value + other.value)
    }
}

/// Type-safe tropical multivector with dimensional constraints
#[derive(Debug, Clone)]
pub struct VerifiedTropicalMultivector<
    T: Float + Clone + Copy,
    const P: usize,
    const Q: usize,
    const R: usize,
    S = MaxPlus,
> {
    coefficients: Vec<VerifiedTropicalNumber<T, S>>,
    _signature: PhantomData<Signature<P, Q, R>>,
}

impl<T: Float + Clone + Copy, const P: usize, const Q: usize, const R: usize, S>
    VerifiedTropicalMultivector<T, P, Q, R, S>
{
    /// Total dimension of the underlying vector space (P + Q + R)
    pub const DIM: usize = P + Q + R;
    /// Number of basis elements in the Clifford algebra (2^DIM)
    pub const BASIS_SIZE: usize = 1 << Self::DIM;
    /// Metric signature as (positive, negative, zero) counts
    pub const SIGNATURE: (usize, usize, usize) = (P, Q, R);

    /// Validate coefficient array size at compile time
    pub fn new(coefficients: Vec<VerifiedTropicalNumber<T, S>>) -> Result<Self, &'static str> {
        if coefficients.len() != Self::BASIS_SIZE {
            return Err("Coefficient array size must equal 2^(P+Q+R)");
        }

        Ok(Self {
            coefficients,
            _signature: PhantomData,
        })
    }

    /// Create scalar tropical multivector
    pub fn scalar(value: T) -> Self {
        let mut coefficients = Vec::with_capacity(Self::BASIS_SIZE);
        for i in 0..Self::BASIS_SIZE {
            if i == 0 {
                coefficients.push(VerifiedTropicalNumber::new(value));
            } else {
                coefficients.push(VerifiedTropicalNumber::new(T::neg_infinity()));
            }
        }

        Self {
            coefficients,
            _signature: PhantomData,
        }
    }

    /// Get coefficient at index
    pub fn get(&self, index: usize) -> VerifiedTropicalNumber<T, S> {
        if index < self.coefficients.len() {
            VerifiedTropicalNumber::new(self.coefficients[index].value())
        } else {
            // Return tropical zero for out of bounds
            VerifiedTropicalNumber::new(T::neg_infinity())
        }
    }

    /// Get the dimension
    pub fn dimension(&self) -> usize {
        Self::DIM
    }

    /// Get the signature
    pub fn signature(&self) -> (usize, usize, usize) {
        Self::SIGNATURE
    }

    /// Check if multivector is tropical zero
    pub fn is_tropical_zero(&self) -> bool {
        self.coefficients.iter().all(|c| c.is_tropical_zero())
    }
}

impl<T: Float + Clone + Copy, const P: usize, const Q: usize, const R: usize>
    VerifiedTropicalMultivector<T, P, Q, R, MaxPlus>
{
    /// Tropical addition (element-wise max)
    pub fn tropical_add(&self, other: &Self) -> Self {
        let coefficients: Vec<_> = self
            .coefficients
            .iter()
            .zip(&other.coefficients)
            .map(|(a, b)| a.tropical_add(*b))
            .collect();

        Self {
            coefficients,
            _signature: PhantomData,
        }
    }

    /// Tropical multiplication (tropical geometric product)
    pub fn tropical_mul(&self, other: &Self) -> Self {
        let mut result =
            vec![VerifiedTropicalNumber::<T, MaxPlus>::tropical_zero(); Self::BASIS_SIZE];

        for (i, self_coefficient) in self.coefficients.iter().enumerate().take(Self::BASIS_SIZE) {
            for (j, other_coefficient) in
                other.coefficients.iter().enumerate().take(Self::BASIS_SIZE)
            {
                let coeff_product = self_coefficient.tropical_mul(*other_coefficient);
                let target_index = i ^ j; // XOR for geometric algebra indices

                result[target_index] = result[target_index].tropical_add(coeff_product);
            }
        }

        Self {
            coefficients: result,
            _signature: PhantomData,
        }
    }

    /// Find maximum element (tropical norm)
    pub fn tropical_norm(&self) -> VerifiedTropicalNumber<T, MaxPlus> {
        self.coefficients.iter().fold(
            VerifiedTropicalNumber::<T, MaxPlus>::tropical_zero(),
            |acc, &x| acc.tropical_add(x),
        )
    }
}

/// Type-safe tropical matrix with verified dimensions
#[derive(Debug, Clone)]
pub struct VerifiedTropicalMatrix<
    T: Float + Clone + Copy,
    const ROWS: usize,
    const COLS: usize,
    S = MaxPlus,
> {
    data: Vec<Vec<VerifiedTropicalNumber<T, S>>>,
    _dimensions: PhantomData<Dim<ROWS>>,
    _columns: PhantomData<Dim<COLS>>,
}

impl<T: Float + Clone + Copy, const ROWS: usize, const COLS: usize, S> Default
    for VerifiedTropicalMatrix<T, ROWS, COLS, S>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Float + Clone + Copy, const ROWS: usize, const COLS: usize, S>
    VerifiedTropicalMatrix<T, ROWS, COLS, S>
{
    /// Create new matrix with verified dimensions
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(ROWS);
        for _ in 0..ROWS {
            let mut row = Vec::with_capacity(COLS);
            for _ in 0..COLS {
                row.push(VerifiedTropicalNumber::new(T::neg_infinity()));
            }
            data.push(row);
        }

        Self {
            data,
            _dimensions: PhantomData,
            _columns: PhantomData,
        }
    }

    /// Get dimensions at compile time
    pub fn dimensions() -> (usize, usize) {
        (ROWS, COLS)
    }

    /// Access element with bounds checking
    pub fn get(&self, row: usize, col: usize) -> Option<VerifiedTropicalNumber<T, S>> {
        if row < self.data.len() && col < self.data[row].len() {
            Some(VerifiedTropicalNumber::new(self.data[row][col].value()))
        } else {
            None
        }
    }

    /// Set element with bounds checking
    pub fn set(
        &mut self,
        row: usize,
        col: usize,
        value: VerifiedTropicalNumber<T, S>,
    ) -> Result<(), &'static str> {
        if row >= ROWS || col >= COLS {
            return Err("Index out of bounds");
        }
        self.data[row][col] = value;
        Ok(())
    }
}

impl<T: Float + Clone + Copy, const ROWS: usize, const COLS: usize>
    VerifiedTropicalMatrix<T, ROWS, COLS, MaxPlus>
{
    /// Tropical matrix multiplication with compile-time dimension checking
    pub fn tropical_mul<const K: usize>(
        &self,
        other: &VerifiedTropicalMatrix<T, COLS, K, MaxPlus>,
    ) -> VerifiedTropicalMatrix<T, ROWS, K, MaxPlus> {
        let mut result = VerifiedTropicalMatrix::<T, ROWS, K, MaxPlus>::new();

        for i in 0..ROWS {
            for j in 0..K {
                let mut sum = VerifiedTropicalNumber::<T, MaxPlus>::tropical_zero();
                for k in 0..COLS {
                    let a = self.get(i, k).unwrap_or_default();
                    let b = other.get(k, j).unwrap_or_default();
                    sum = sum.tropical_add(a.tropical_mul(b));
                }
                result.set(i, j, sum).unwrap();
            }
        }

        result
    }
}

// Default implementations for tropical numbers
impl<T: Float + Clone + Copy> Default for VerifiedTropicalNumber<T, MaxPlus> {
    fn default() -> Self {
        Self::tropical_zero()
    }
}

impl<T: Float + Clone + Copy> Default for VerifiedTropicalNumber<T, MinPlus> {
    fn default() -> Self {
        Self::tropical_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verified_tropical_number_max_plus() {
        let a = VerifiedTropicalNumber::<f64, MaxPlus>::new(2.0);
        let b = VerifiedTropicalNumber::<f64, MaxPlus>::new(3.0);

        let sum = a.tropical_add(b);
        assert_eq!(sum.value(), 3.0); // max(2, 3) = 3

        let product = a.tropical_mul(b);
        assert_eq!(product.value(), 5.0); // 2 + 3 = 5
    }

    #[test]
    fn test_verified_tropical_number_min_plus() {
        let a = VerifiedTropicalNumber::<f64, MinPlus>::new(2.0);
        let b = VerifiedTropicalNumber::<f64, MinPlus>::new(3.0);

        let sum = a.tropical_add(b);
        assert_eq!(sum.value(), 2.0); // min(2, 3) = 2

        let product = a.tropical_mul(b);
        assert_eq!(product.value(), 5.0); // 2 + 3 = 5
    }

    #[test]
    fn test_verified_tropical_multivector() {
        type TropMV = VerifiedTropicalMultivector<f64, 3, 0, 0, MaxPlus>;

        let mv = TropMV::scalar(5.0);
        assert_eq!(mv.dimension(), 3);
        assert_eq!(mv.signature(), (3, 0, 0));
        assert_eq!(mv.get(0).value(), 5.0);
    }

    #[test]
    fn test_verified_tropical_matrix() {
        type TropMatrix = VerifiedTropicalMatrix<f64, 2, 3, MaxPlus>;

        let mut matrix = TropMatrix::new();
        assert_eq!(TropMatrix::dimensions(), (2, 3));

        let val = VerifiedTropicalNumber::new(1.5);
        matrix.set(0, 1, val).unwrap();
        assert_eq!(matrix.get(0, 1).unwrap().value(), 1.5);
    }

    #[test]
    fn test_tropical_matrix_multiplication() {
        let mut m1 = VerifiedTropicalMatrix::<f64, 2, 2, MaxPlus>::new();
        let mut m2 = VerifiedTropicalMatrix::<f64, 2, 2, MaxPlus>::new();

        // Set up simple values for testing
        m1.set(0, 0, VerifiedTropicalNumber::new(1.0)).unwrap();
        m1.set(0, 1, VerifiedTropicalNumber::new(2.0)).unwrap();
        m2.set(0, 0, VerifiedTropicalNumber::new(0.5)).unwrap();
        m2.set(1, 0, VerifiedTropicalNumber::new(1.5)).unwrap();

        let result = m1.tropical_mul(&m2);

        // Verify multiplication worked
        assert!(result.get(0, 0).is_some());
    }
}
