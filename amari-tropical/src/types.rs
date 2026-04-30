//! Core tropical algebra types
//!
//! This module defines the fundamental types for tropical (max-plus) algebra:
//! - TropicalNumber: Scalar values in the tropical semiring
//! - TropicalMatrix: Matrices over the tropical semiring
//! - TropicalMultivector: Geometric algebra multivectors in tropical space
//!
//! ## Tropical Semiring
//!
//! The tropical semiring replaces standard arithmetic operations with:
//! - Tropical addition: max(a, b)
//! - Tropical multiplication: a + b
//! - Tropical zero: -∞
//! - Tropical one: 0
//!
//! This structure is isomorphic to (ℝ ∪ {-∞}, max, +) and has applications in:
//! - Optimization and shortest path algorithms
//! - Machine learning (max-plus neural networks)
//! - Discrete event systems
//! - Phylogenetics and computational biology

use core::fmt;
use num_traits::Float;

#[cfg(feature = "std")]
use std::ops::{Add, Mul};

#[cfg(not(feature = "std"))]
use core::ops::{Add, Mul};

use crate::TropicalError;

/// A tropical number in the max-plus semiring
///
/// Represents a value in tropical algebra where:
/// - Addition is max(a, b)
/// - Multiplication is a + b
/// - Zero element is -∞
/// - Unit element is 0
///
/// Generic over any floating-point type supporting the `Float` trait.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TropicalNumber<T: Float> {
    value: T,
}

impl<T: Float> TropicalNumber<T> {
    /// Create a new tropical number
    ///
    /// # Example
    /// ```
    /// use amari_tropical::TropicalNumber;
    ///
    /// let t = TropicalNumber::new(3.5);
    /// assert_eq!(t.value(), 3.5);
    /// ```
    #[inline]
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Get the underlying value
    #[inline]
    pub fn value(&self) -> T {
        self.value
    }

    /// Create tropical zero (-∞)
    ///
    /// The additive identity in tropical algebra
    #[inline]
    pub fn zero() -> Self {
        Self {
            value: T::neg_infinity(),
        }
    }

    /// Alias for `zero()` - tropical additive identity
    ///
    /// Returns negative infinity, the additive identity in tropical algebra
    #[inline]
    pub fn neg_infinity() -> Self {
        Self::zero()
    }

    /// Alias for `zero()` - tropical additive identity
    #[inline]
    pub fn tropical_zero() -> Self {
        Self::zero()
    }

    /// Create tropical one (0)
    ///
    /// The multiplicative identity in tropical algebra
    #[inline]
    pub fn one() -> Self {
        Self { value: T::zero() }
    }

    /// Alias for `one()` - tropical multiplicative identity
    #[inline]
    pub fn tropical_one() -> Self {
        Self::one()
    }

    /// Check if this is tropical zero (-∞)
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.value == T::neg_infinity()
    }

    /// Check if this is tropical one (0)
    #[inline]
    pub fn is_one(&self) -> bool {
        self.value == T::zero()
    }

    /// Check if this value represents infinity (tropical zero)
    ///
    /// Alias for `is_zero()` - checks if value is negative infinity
    #[inline]
    pub fn is_infinity(&self) -> bool {
        self.is_zero()
    }

    /// Tropical addition: max(self, other)
    #[inline]
    pub fn tropical_add(&self, other: &Self) -> Self {
        Self {
            value: self.value.max(other.value),
        }
    }

    /// Tropical multiplication: self + other
    #[inline]
    pub fn tropical_mul(&self, other: &Self) -> Self {
        Self {
            value: self.value + other.value,
        }
    }

    /// Tropical power: self * k (standard multiplication)
    #[inline]
    pub fn tropical_pow(&self, k: T) -> Self {
        Self {
            value: self.value * k,
        }
    }
}

impl<T: Float + fmt::Display> fmt::Display for TropicalNumber<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tropical({})", self.value)
    }
}

// Standard arithmetic operators using tropical operations
impl<T: Float> Add for TropicalNumber<T> {
    type Output = Self;

    /// Implements tropical addition as max
    #[inline]
    fn add(self, other: Self) -> Self {
        self.tropical_add(&other)
    }
}

impl<T: Float> Mul for TropicalNumber<T> {
    type Output = Self;

    /// Implements tropical multiplication as addition
    #[inline]
    fn mul(self, other: Self) -> Self {
        self.tropical_mul(&other)
    }
}

/// A matrix in tropical algebra
///
/// Stores elements as a 2D vector with dimensions (rows × cols).
/// Matrix operations follow tropical semiring rules:
/// - Matrix addition: element-wise max
/// - Matrix multiplication: tropical matrix product
#[derive(Debug, Clone, PartialEq)]
pub struct TropicalMatrix<T: Float> {
    /// Number of rows in the matrix
    pub rows: usize,
    /// Number of columns in the matrix
    pub cols: usize,
    /// Matrix data stored as row-major nested vectors
    pub data: Vec<Vec<TropicalNumber<T>>>,
}

impl<T: Float> TropicalMatrix<T> {
    /// Create a new tropical matrix with given dimensions
    ///
    /// Initializes all elements to tropical zero (-∞)
    pub fn new(rows: usize, cols: usize) -> Self {
        let data = vec![vec![TropicalNumber::zero(); cols]; rows];
        Self { rows, cols, data }
    }

    /// Create a tropical matrix from raw 2D data
    ///
    /// Data must be a Vec of rows, where each row is a Vec of values
    pub fn from_vec(data: Vec<Vec<T>>) -> Result<Self, TropicalError> {
        if data.is_empty() {
            return Ok(Self::new(0, 0));
        }

        let rows = data.len();
        let cols = data[0].len();

        // Verify all rows have the same length
        for (i, row) in data.iter().enumerate() {
            if row.len() != cols {
                return Err(TropicalError::DimensionMismatch(format!(
                    "Row {} has {} elements, expected {}",
                    i,
                    row.len(),
                    cols
                )));
            }
        }

        let tropical_data: Vec<Vec<TropicalNumber<T>>> = data
            .into_iter()
            .map(|row| row.into_iter().map(TropicalNumber::new).collect())
            .collect();

        Ok(Self {
            rows,
            cols,
            data: tropical_data,
        })
    }

    /// Get matrix dimensions (rows, cols)
    #[inline]
    pub fn dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Get number of rows
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Get number of columns
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Get element at (row, col)
    pub fn get(&self, row: usize, col: usize) -> Result<TropicalNumber<T>, TropicalError> {
        if row >= self.rows || col >= self.cols {
            return Err(TropicalError::IndexOutOfBounds(format!(
                "Index ({}, {}) out of bounds for {}×{} matrix",
                row, col, self.rows, self.cols
            )));
        }
        Ok(self.data[row][col])
    }

    /// Set element at (row, col)
    pub fn set(
        &mut self,
        row: usize,
        col: usize,
        value: TropicalNumber<T>,
    ) -> Result<(), TropicalError> {
        if row >= self.rows || col >= self.cols {
            return Err(TropicalError::IndexOutOfBounds(format!(
                "Index ({}, {}) out of bounds for {}×{} matrix",
                row, col, self.rows, self.cols
            )));
        }
        self.data[row][col] = value;
        Ok(())
    }

    /// Create tropical identity matrix
    ///
    /// Diagonal elements are tropical one (0), off-diagonal are tropical zero (-∞)
    pub fn identity(size: usize) -> Self {
        let mut matrix = Self::new(size, size);
        for i in 0..size {
            matrix.data[i][i] = TropicalNumber::one();
        }
        matrix
    }

    /// Create tropical matrix from log probabilities
    ///
    /// In tropical algebra, log probabilities are natural since:
    /// - log(p1 * p2) = log(p1) + log(p2) (tropical multiplication)
    /// - log(max(p1, p2)) ≈ max(log(p1), log(p2)) (tropical addition)
    ///
    /// This method converts a 2D array of log probabilities into a tropical matrix.
    pub fn from_log_probs(log_probs: &[Vec<T>]) -> Self {
        if log_probs.is_empty() {
            return Self::new(0, 0);
        }

        let rows = log_probs.len();
        let cols = log_probs.first().map(|r| r.len()).unwrap_or(0);

        let data: Vec<Vec<TropicalNumber<T>>> = log_probs
            .iter()
            .map(|row| row.iter().map(|&val| TropicalNumber::new(val)).collect())
            .collect();

        Self { rows, cols, data }
    }

    /// Tropical matrix multiplication
    ///
    /// `(A ⊗ B)[i,j] = max_k(A[i,k] + B[k,j])`
    pub fn tropical_matmul(&self, other: &Self) -> Result<Self, TropicalError> {
        if self.cols != other.rows {
            return Err(TropicalError::DimensionMismatch(format!(
                "Cannot multiply {}×{} matrix with {}×{} matrix",
                self.rows, self.cols, other.rows, other.cols
            )));
        }

        let mut result = Self::new(self.rows, other.cols);

        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = TropicalNumber::zero();
                for k in 0..self.cols {
                    let a = self.data[i][k];
                    let b = other.data[k][j];
                    sum = sum.tropical_add(&a.tropical_mul(&b));
                }
                result.data[i][j] = sum;
            }
        }

        Ok(result)
    }

    /// Get the underlying data as a 2D vector
    pub fn data(&self) -> &Vec<Vec<TropicalNumber<T>>> {
        &self.data
    }

    /// Get mutable access to the underlying data
    pub fn data_mut(&mut self) -> &mut Vec<Vec<TropicalNumber<T>>> {
        &mut self.data
    }
}

impl<T: Float + fmt::Display> fmt::Display for TropicalMatrix<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "TropicalMatrix [{}×{}]:", self.rows, self.cols)?;
        for row in &self.data {
            write!(f, "  [")?;
            for (j, val) in row.iter().enumerate() {
                if j > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", val.value)?;
            }
            writeln!(f, "]")?;
        }
        Ok(())
    }
}

/// A geometric algebra multivector in tropical space
///
/// Combines geometric algebra with tropical semiring operations.
/// Generic over:
/// - T: Scalar type (must implement Float)
/// - P, Q, R: Metric signature (p positive, q negative, r null dimensions)
#[derive(Debug, Clone, PartialEq)]
pub struct TropicalMultivector<T: Float, const P: usize, const Q: usize, const R: usize> {
    /// Multivector components in tropical algebra
    ///
    /// Total dimension is 2^(P+Q+R) for the full geometric algebra
    components: Vec<TropicalNumber<T>>,
}

impl<T: Float, const P: usize, const Q: usize, const R: usize> TropicalMultivector<T, P, Q, R> {
    /// Create a new tropical multivector
    ///
    /// Initializes all components to tropical zero (-∞)
    pub fn new() -> Self {
        let dim = 1 << (P + Q + R); // 2^(P+Q+R)
        Self {
            components: vec![TropicalNumber::zero(); dim],
        }
    }

    /// Create from component values
    pub fn from_components(components: Vec<T>) -> Result<Self, TropicalError> {
        let dim = 1 << (P + Q + R);
        if components.len() != dim {
            return Err(TropicalError::DimensionMismatch(format!(
                "Expected {} components for Cl({},{},{}), got {}",
                dim,
                P,
                Q,
                R,
                components.len()
            )));
        }

        Ok(Self {
            components: components.into_iter().map(TropicalNumber::new).collect(),
        })
    }

    /// Get total number of components
    #[inline]
    pub fn dim(&self) -> usize {
        self.components.len()
    }

    /// Get component at index
    pub fn get(&self, index: usize) -> Result<TropicalNumber<T>, TropicalError> {
        self.components.get(index).copied().ok_or_else(|| {
            TropicalError::IndexOutOfBounds(format!(
                "Index {} out of bounds for multivector with {} components",
                index,
                self.components.len()
            ))
        })
    }

    /// Set component at index
    pub fn set(&mut self, index: usize, value: TropicalNumber<T>) -> Result<(), TropicalError> {
        if index >= self.components.len() {
            return Err(TropicalError::IndexOutOfBounds(format!(
                "Index {} out of bounds for multivector with {} components",
                index,
                self.components.len()
            )));
        }
        self.components[index] = value;
        Ok(())
    }

    /// Get slice of all components
    pub fn components(&self) -> &[TropicalNumber<T>] {
        &self.components
    }

    /// Tropical multivector addition (grade-wise max)
    pub fn tropical_add(&self, other: &Self) -> Self {
        let components: Vec<_> = self
            .components
            .iter()
            .zip(&other.components)
            .map(|(a, b)| a.tropical_add(b))
            .collect();

        Self { components }
    }

    /// Geometric product in tropical algebra
    ///
    /// This is a simplified implementation for tropical geometric algebra.
    /// In tropical algebra, the geometric product combines grade-wise operations.
    pub fn geometric_product(&self, other: &Self) -> Self {
        // Simplified tropical geometric product
        // For now, use component-wise tropical multiplication as approximation
        let components: Vec<_> = self
            .components
            .iter()
            .zip(&other.components)
            .map(|(a, b)| a.tropical_mul(b))
            .collect();

        Self { components }
    }

    /// Compute tropical norm
    ///
    /// In tropical algebra, the norm is the maximum absolute value
    /// of all components (tropical addition of all components)
    pub fn tropical_norm(&self) -> TropicalNumber<T> {
        let mut max_val = TropicalNumber::zero();
        for &comp in &self.components {
            max_val = max_val.tropical_add(&comp);
        }
        max_val
    }

    /// Get scalar part (grade-0 component)
    #[inline]
    pub fn scalar(&self) -> TropicalNumber<T> {
        self.components[0]
    }
}

impl<T: Float, const P: usize, const Q: usize, const R: usize> Default
    for TropicalMultivector<T, P, Q, R>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Float + fmt::Display, const P: usize, const Q: usize, const R: usize> fmt::Display
    for TropicalMultivector<T, P, Q, R>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TropicalMultivector<{},{},{}>([", P, Q, R)?;
        for (i, comp) in self.components.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", comp.value)?;
        }
        write!(f, "])")
    }
}

// Precision type aliases for tropical numbers
/// Standard-precision tropical number (f64)
pub type StandardTropical = TropicalNumber<f64>;

/// Extended-precision tropical number (uses extended precision float from amari-core)
#[cfg(feature = "high-precision")]
pub type ExtendedTropical = TropicalNumber<crate::ExtendedFloat>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tropical_number_basics() {
        let a = TropicalNumber::new(3.0f64);
        let b = TropicalNumber::new(5.0f64);

        // Tropical addition is max
        let sum = a.tropical_add(&b);
        assert_eq!(sum.value(), 5.0);

        // Tropical multiplication is addition
        let product = a.tropical_mul(&b);
        assert_eq!(product.value(), 8.0);
    }

    #[test]
    fn test_tropical_identities() {
        let zero = TropicalNumber::<f64>::zero();
        let one = TropicalNumber::<f64>::one();
        let x = TropicalNumber::new(3.0);

        // Tropical zero is -∞, is additive identity
        assert!(zero.is_zero());
        assert_eq!(x.tropical_add(&zero), x);

        // Tropical one is 0, is multiplicative identity
        assert!(one.is_one());
        assert_eq!(x.tropical_mul(&one), x);
    }

    #[test]
    fn test_tropical_matrix_creation() {
        let mat = TropicalMatrix::<f64>::new(2, 3);
        assert_eq!(mat.dims(), (2, 3));
        assert_eq!(mat.rows(), 2);
        assert_eq!(mat.cols(), 3);
    }

    #[test]
    fn test_tropical_matrix_identity() {
        let id = TropicalMatrix::<f64>::identity(3);
        assert_eq!(id.dims(), (3, 3));

        // Diagonal should be tropical one (0)
        for i in 0..3 {
            assert!(id.get(i, i).unwrap().is_one());
        }

        // Off-diagonal should be tropical zero (-∞)
        assert!(id.get(0, 1).unwrap().is_zero());
        assert!(id.get(1, 2).unwrap().is_zero());
    }

    #[test]
    fn test_tropical_matrix_multiplication() {
        let mut a = TropicalMatrix::<f64>::new(2, 2);
        a.set(0, 0, TropicalNumber::new(1.0)).unwrap();
        a.set(0, 1, TropicalNumber::new(2.0)).unwrap();
        a.set(1, 0, TropicalNumber::new(3.0)).unwrap();
        a.set(1, 1, TropicalNumber::new(4.0)).unwrap();

        let id = TropicalMatrix::<f64>::identity(2);

        // Multiplying by identity should preserve matrix
        let result = a.tropical_matmul(&id).unwrap();
        assert_eq!(result.get(0, 0).unwrap().value(), 1.0);
        assert_eq!(result.get(0, 1).unwrap().value(), 2.0);
        assert_eq!(result.get(1, 0).unwrap().value(), 3.0);
        assert_eq!(result.get(1, 1).unwrap().value(), 4.0);
    }

    #[test]
    fn test_tropical_multivector_creation() {
        let mv = TropicalMultivector::<f64, 3, 0, 0>::new();
        assert_eq!(mv.dim(), 8); // 2^3 = 8 components for Cl(3,0,0)
    }

    #[test]
    fn test_tropical_multivector_addition() {
        let mut a = TropicalMultivector::<f64, 2, 0, 0>::new();
        let mut b = TropicalMultivector::<f64, 2, 0, 0>::new();

        a.set(0, TropicalNumber::new(1.0)).unwrap();
        a.set(1, TropicalNumber::new(2.0)).unwrap();

        b.set(0, TropicalNumber::new(3.0)).unwrap();
        b.set(1, TropicalNumber::new(1.5)).unwrap();

        let sum = a.tropical_add(&b);

        // Tropical addition is max
        assert_eq!(sum.get(0).unwrap().value(), 3.0);
        assert_eq!(sum.get(1).unwrap().value(), 2.0);
    }
}
